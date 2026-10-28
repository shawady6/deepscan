//! Pause Microsoft Defender real-time protection for a set time (or until turned back on), then switch it on again.
//!
//! The auto-resume does not depend on DeepScan staying open: a one-time Windows scheduled task (SYSTEM) turns
//! protection back on and deletes itself. It is created BEFORE protection is switched off, and it runs at next
//! start if the PC was off at that time. DeepScan also re-checks at startup as a second safety net.
//! Tamper Protection blocks this on purpose; Windows does not allow programs to turn it off, so we explain and stop.
use crate::store;
use crate::util::*;
use serde_json::{json, Value};
use std::sync::Mutex;

const TASK: &str = "DeepScan-DefenderResume";
const ALLOWED_MINUTES: &[u32] = &[0, 15, 30, 60, 120]; // 0 = until the user turns it back on

/// Demo state for non-Windows development.
static DEMO_OFF: Mutex<Option<i64>> = Mutex::new(None);

fn now() -> i64 {
    chrono::Local::now().timestamp()
}

fn fmt_time(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string()).unwrap_or_default()
}

/// Raw read: (service usable, real-time on, tamper protection on)
fn read() -> (bool, bool, bool) {
    let v = ps_json(
        "$m = Get-MpComputerStatus; if ($m) { [pscustomobject]@{ svc=[bool]$m.AMServiceEnabled; rtp=[bool]$m.RealTimeProtectionEnabled; tamper=[bool]$m.IsTamperProtected } | ConvertTo-Json -Compress } else { 'null' }",
        30,
    )
    .unwrap_or(Value::Null);
    if v.is_null() {
        return (false, false, false);
    }
    (g(&v, "svc") == true, g(&v, "rtp") == true, g(&v, "tamper") == true)
}

fn build(avail: bool, rtp: bool, tamper: bool) -> Value {
    let pause = store::defender_pause();
    let until = pause["until_ts"].as_i64();
    let indefinite = pause["indefinite"] == true;
    let paused = avail && !rtp && (indefinite || until.is_some());
    json!({
        "available": avail, "realtime": rtp, "tamper": tamper, "paused_by_app": paused,
        "indefinite": paused && indefinite,
        "until": if paused { until.map(fmt_time) } else { None },
        "remaining_min": if paused { until.map(|u| ((u - now()).max(0) + 59) / 60) } else { None },
    })
}

pub fn status() -> Value {
    if !IS_WIN {
        let off = *DEMO_OFF.lock().unwrap();
        return json!({"demo": true, "available": true, "realtime": off.is_none(), "tamper": false, "paused_by_app": off.is_some(),
            "indefinite": off == Some(0), "until": off.filter(|t| *t > 0).map(fmt_time), "remaining_min": off.filter(|t| *t > 0).map(|t| ((t - now()).max(0) + 59) / 60)});
    }
    let (avail, rtp, tamper) = read();
    // Back on already (user did it, or the task ran)? Forget the pause.
    if rtp && !store::defender_pause().is_null() {
        store::set_defender_pause(Value::Null);
    }
    build(avail, rtp, tamper)
}

fn wait_for(want_rtp: bool) -> bool {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if read().1 == want_rtp {
            return true;
        }
    }
    false
}

pub fn pause(minutes: u32) -> Value {
    if !ALLOWED_MINUTES.contains(&minutes) {
        return json!({"ok": false, "error": "Unsupported duration."});
    }
    if !IS_WIN {
        *DEMO_OFF.lock().unwrap() = Some(if minutes == 0 { 0 } else { now() + minutes as i64 * 60 });
        return json!({"ok": true, "demo": true});
    }
    let (avail, rtp, tamper) = read();
    if !avail {
        return json!({"ok": false, "error": "Microsoft Defender is not the active antivirus on this PC, so there is nothing to pause here."});
    }
    if tamper {
        return json!({"ok": false, "tamper": true, "error": "Tamper Protection is on. Windows does not let programs turn Defender off while it is on. Open Windows Security > Virus & threat protection > Manage settings, turn Tamper Protection off, then try again."});
    }
    if !rtp {
        return json!({"ok": false, "error": "Real-time protection is already off."});
    }
    // 1. schedule the automatic resume first, so protection can never stay off by accident
    if minutes > 0 {
        let sched = format!(
            "$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -WindowStyle Hidden -Command \"Set-MpPreference -DisableRealtimeMonitoring $false; Unregister-ScheduledTask -TaskName {TASK} -Confirm:$false\"'\n\
             $t = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes({minutes})\n\
             $s = New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries\n\
             Register-ScheduledTask -TaskName '{TASK}' -Action $a -Trigger $t -Settings $s -User 'SYSTEM' -RunLevel Highest -Force | Out-Null"
        );
        let (ok, out) = ps_run(&sched, 60);
        if !ok {
            return json!({"ok": false, "error": format!("Could not schedule the automatic turn-on, so protection was left on. {out}")});
        }
    }
    // 2. switch protection off
    let (ok, out) = ps_run("Set-MpPreference -DisableRealtimeMonitoring $true", 60);
    if !ok || !wait_for(false) {
        let _ = ps_run(&format!("Unregister-ScheduledTask -TaskName '{TASK}' -Confirm:$false -ErrorAction SilentlyContinue"), 30);
        return json!({"ok": false, "error": format!("Windows did not turn protection off (another policy or Tamper Protection may be blocking it). {out}")});
    }
    let until = if minutes > 0 { Some(now() + minutes as i64 * 60) } else { None };
    store::set_defender_pause(json!({"until_ts": until, "indefinite": minutes == 0}));
    store::add_history(json!({"title": "Paused Defender real-time protection",
        "detail": if minutes == 0 { "Until turned back on".to_string() } else { format!("For {minutes} minutes") }, "undoable": false}));
    json!({"ok": true})
}

pub fn resume() -> Value {
    if !IS_WIN {
        *DEMO_OFF.lock().unwrap() = None;
        return json!({"ok": true, "demo": true});
    }
    let (ok, out) = ps_run(&format!("Set-MpPreference -DisableRealtimeMonitoring $false\nUnregister-ScheduledTask -TaskName '{TASK}' -Confirm:$false -ErrorAction SilentlyContinue"), 60);
    if !ok || !wait_for(true) {
        return json!({"ok": false, "error": format!("Could not confirm that protection is back on. Open Windows Security to check. {out}")});
    }
    store::set_defender_pause(Value::Null);
    store::add_history(json!({"title": "Turned Defender real-time protection back on", "detail": "", "undoable": false}));
    json!({"ok": true})
}

/// Startup safety net: if a pause we started has run out (the scheduled task missed), turn protection back on.
pub fn reconcile() {
    if !IS_WIN {
        return;
    }
    let p = store::defender_pause();
    if let Some(u) = p["until_ts"].as_i64() {
        if u <= now() && !read().1 {
            let _ = resume();
        }
    }
}
