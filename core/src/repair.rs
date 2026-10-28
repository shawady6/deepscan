//! Repair tab backend: Windows health checks, bloatware removal, "slim down" tweaks,
//! a library of fix scripts, restore points and an undo history.
//!
//! Safety model: the UI only ever sends IDs. Every script and registry change lives here as static
//! data, so nothing the UI (or a web page inside it) sends can run arbitrary code.
use crate::store;
use crate::util::*;
use serde_json::{json, Value};
use std::process::Command;
use std::time::Duration;

// ===================================================================== registry helpers
#[derive(Clone, Copy, PartialEq)]
pub enum Hive {
    Cu,
    Lm,
}
impl Hive {
    fn tag(self) -> &'static str {
        if self == Hive::Cu { "cu" } else { "lm" }
    }
    fn from_tag(t: &str) -> Hive {
        if t == "cu" { Hive::Cu } else { Hive::Lm }
    }
}

#[cfg(windows)]
fn root(h: Hive) -> winreg::RegKey {
    use winreg::enums::*;
    winreg::RegKey::predef(if h == Hive::Cu { HKEY_CURRENT_USER } else { HKEY_LOCAL_MACHINE })
}
#[cfg(windows)]
fn reg_get(h: Hive, path: &str, name: &str) -> Option<u32> {
    root(h).open_subkey(path).ok()?.get_value::<u32, _>(name).ok()
}
#[cfg(windows)]
fn reg_set(h: Hive, path: &str, name: &str, value: Option<u32>) -> Result<(), String> {
    match value {
        Some(v) => {
            let (key, _) = root(h).create_subkey(path).map_err(|e| e.to_string())?;
            key.set_value(name, &v).map_err(|e| e.to_string())
        }
        None => match root(h).open_subkey_with_flags(path, winreg::enums::KEY_SET_VALUE) {
            Ok(key) => key.delete_value(name).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }).map_err(|e| e.to_string()),
            Err(_) => Ok(()), // key absent: nothing to remove
        },
    }
}
#[cfg(not(windows))]
fn reg_get(_: Hive, _: &str, _: &str) -> Option<u32> { None }
#[cfg(not(windows))]
fn reg_set(_: Hive, _: &str, _: &str, _: Option<u32>) -> Result<(), String> { Err("Windows only".into()) }

fn sh(program: &str, args: &[&str]) -> bool {
    run_cmd_status(Command::new(program).args(args), Duration::from_secs(60)).map(|(ok, _)| ok).unwrap_or(false)
}

// ===================================================================== slim-down tweaks
pub enum Op {
    /// Registry DWORD set to `value`.
    Dword(Hive, &'static str, &'static str, u32),
    /// `powercfg /h off` (also deletes hiberfil.sys).
    Hibernate,
}
use Op::*;

pub struct Tweak {
    pub id: &'static str,
    pub name: &'static str,
    pub note: &'static str,
    pub risk: &'static str,
    /// 1 = Light, 2 = Balanced, 3 = Aggressive (a preset includes every tweak at or below its level).
    pub level: u8,
    pub ops: &'static [Op],
    /// Services stopped right after applying (and started again on undo when they were enabled before).
    pub stop: &'static [&'static str],
}

const CDM: &str = r"Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const ADV: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const SVC: &str = r"SYSTEM\CurrentControlSet\Services";

pub static TWEAKS: &[Tweak] = &[
    Tweak { id: "ads", name: "Turn off ads and app suggestions", note: "Stops suggested apps and ads in Start and Settings.", risk: "low", level: 1, stop: &[], ops: &[
        Dword(Hive::Cu, CDM, "SubscribedContent-338388Enabled", 0), Dword(Hive::Cu, CDM, "SubscribedContent-338393Enabled", 0),
        Dword(Hive::Cu, CDM, "SubscribedContent-353694Enabled", 0), Dword(Hive::Cu, CDM, "SubscribedContent-353696Enabled", 0),
        Dword(Hive::Cu, CDM, "SystemPaneSuggestionsEnabled", 0), Dword(Hive::Cu, CDM, "SilentInstalledAppsEnabled", 0),
        Dword(Hive::Cu, CDM, "OemPreInstalledAppsEnabled", 0), Dword(Hive::Cu, CDM, "PreInstalledAppsEnabled", 0),
        Dword(Hive::Cu, r"Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", 0)] },
    Tweak { id: "tips", name: "Turn off tips and welcome notifications", note: "No more \"Get the most out of Windows\" prompts.", risk: "low", level: 1, stop: &[], ops: &[
        Dword(Hive::Cu, CDM, "SubscribedContent-338389Enabled", 0), Dword(Hive::Cu, CDM, "SoftLandingEnabled", 0),
        Dword(Hive::Cu, r"Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement", "ScoobeSystemSettingEnabled", 0)] },
    Tweak { id: "telemetry", name: "Send required diagnostic data only", note: "Sets diagnostic data to the minimum level.", risk: "low", level: 1, stop: &[], ops: &[
        Dword(Hive::Lm, r"SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", 1)] },
    Tweak { id: "start_recs", name: "Hide Start menu recommendations", note: "Removes the recommended files and apps section.", risk: "low", level: 1, stop: &[], ops: &[
        Dword(Hive::Cu, ADV, "Start_IrisRecommendations", 0)] },
    Tweak { id: "widgets", name: "Remove Widgets and news feed", note: "Turns the Widgets panel off.", risk: "medium", level: 2, stop: &[], ops: &[
        Dword(Hive::Lm, r"SOFTWARE\Policies\Microsoft\Dsh", "AllowNewsAndInterests", 0)] },
    Tweak { id: "copilot", name: "Turn off Copilot", note: "Disables the Copilot button and panel (policy).", risk: "medium", level: 2, stop: &[], ops: &[
        Dword(Hive::Cu, r"Software\Policies\Microsoft\Windows\WindowsCopilot", "TurnOffWindowsCopilot", 1)] },
    Tweak { id: "bgapps", name: "Stop apps running in the background", note: "Store apps only run when you open them.", risk: "medium", level: 2, stop: &[], ops: &[
        Dword(Hive::Cu, r"Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications", "GlobalUserDisabled", 1)] },
    Tweak { id: "websearch", name: "Turn off web results in Search", note: "Start search shows only your PC.", risk: "medium", level: 2, stop: &[], ops: &[
        Dword(Hive::Cu, r"Software\Policies\Microsoft\Windows\Explorer", "DisableSearchBoxSuggestions", 1)] },
    Tweak { id: "visual", name: "Reduce visual effects and animations", note: "Switches to the \"best performance\" effects profile.", risk: "medium", level: 3, stop: &[], ops: &[
        Dword(Hive::Cu, r"Software\Microsoft\Windows\CurrentVersion\Explorer\VisualEffects", "VisualFXSetting", 2),
        Dword(Hive::Cu, ADV, "TaskbarAnimations", 0)] },
    Tweak { id: "hibernate", name: "Turn off hibernation", note: "Frees the hibernation file (about 40% of RAM size). Fast Startup stops working.", risk: "high", level: 3, stop: &[], ops: &[Hibernate] },
    Tweak { id: "indexing", name: "Turn off search indexing", note: "Searching for files becomes slower.", risk: "high", level: 3, stop: &["WSearch"], ops: &[
        Dword(Hive::Lm, "SYSTEM\\CurrentControlSet\\Services\\WSearch", "Start", 4)] },
    Tweak { id: "sysmain", name: "Stop SysMain (Superfetch)", note: "Only useful on SSD drives. Not recommended for an HDD.", risk: "high", level: 3, stop: &["SysMain"], ops: &[
        Dword(Hive::Lm, "SYSTEM\\CurrentControlSet\\Services\\SysMain", "Start", 4)] },
];

fn hibernate_off() -> bool {
    reg_get(Hive::Lm, r"SYSTEM\CurrentControlSet\Control\Power", "HibernateEnabled") == Some(0)
}

fn op_applied(op: &Op) -> bool {
    match op {
        Dword(h, p, n, v) => reg_get(*h, p, n) == Some(*v),
        Hibernate => hibernate_off(),
    }
}

fn tweak_applied(t: &Tweak) -> bool {
    t.ops.iter().all(op_applied)
}

fn find_tweak(id: &str) -> Option<&'static Tweak> {
    TWEAKS.iter().find(|t| t.id == id)
}

pub fn tweaks() -> Value {
    Value::Array(TWEAKS.iter().map(|t| json!({"id": t.id, "name": t.name, "note": t.note, "risk": t.risk, "level": t.level, "applied": tweak_applied(t)})).collect())
}

/// Apply tweaks and record one history entry holding what was there before, so it can be undone.
pub fn tweaks_apply(ids: &[String]) -> Value {
    let (mut results, mut snap, mut done) = (vec![], vec![], vec![]);
    for id in ids {
        let Some(t) = find_tweak(id) else {
            results.push(json!({"id": id, "ok": false, "error": "Unknown option"}));
            continue;
        };
        let mut err: Option<String> = None;
        for op in t.ops {
            match op {
                Dword(h, p, n, v) => {
                    snap.push(json!({"t": "dword", "hive": h.tag(), "path": p, "name": n, "old": reg_get(*h, p, n)}));
                    if let Err(e) = reg_set(*h, p, n, Some(*v)) {
                        err = Some(e);
                        break;
                    }
                }
                Hibernate => {
                    snap.push(json!({"t": "hibernate", "old": !hibernate_off()}));
                    if !sh("powercfg", &["/h", "off"]) {
                        err = Some("powercfg failed".into());
                        break;
                    }
                }
            }
        }
        if err.is_none() {
            for svc in t.stop {
                let _ = sh("sc", &["stop", svc]);
            }
            done.push(t.name);
        }
        results.push(json!({"id": id, "ok": err.is_none(), "error": err}));
    }
    let hid = if done.is_empty() {
        Value::Null
    } else {
        json!(store::add_history(json!({
            "title": format!("Applied {} setting{}", done.len(), if done.len() == 1 { "" } else { "s" }),
            "detail": done.join(", "), "undoable": true, "undo": {"snap": snap, "ids": ids}
        })))
    };
    json!({"ok": results.iter().all(|r| r["ok"] == true), "results": results, "history_id": hid})
}

pub fn history_undo(id: &str) -> Value {
    let Some(e) = store::history_entry(id) else { return json!({"ok": false, "error": "Entry not found"}) };
    if e["undone"] == true || e["undoable"] != true {
        return json!({"ok": false, "error": "This entry cannot be undone"});
    }
    let mut failed = vec![];
    let snap = e["undo"]["snap"].as_array().cloned().unwrap_or_default();
    for item in snap.iter().rev() {
        match item["t"].as_str() {
            Some("dword") => {
                let old = item["old"].as_u64().map(|x| x as u32);
                if let Err(er) = reg_set(Hive::from_tag(&s(&item["hive"])), &s(&item["path"]), &s(&item["name"]), old) {
                    failed.push(er);
                }
            }
            Some("hibernate") => {
                if item["old"] == true && !sh("powercfg", &["/h", "on"]) {
                    failed.push("powercfg failed".into());
                }
            }
            _ => {}
        }
    }
    // Services that were stopped: start them again when their start type is no longer "disabled".
    for id in e["undo"]["ids"].as_array().cloned().unwrap_or_default() {
        if let Some(t) = id.as_str().and_then(find_tweak) {
            for svc in t.stop {
                if reg_get(Hive::Lm, &format!("{SVC}\\{svc}"), "Start") != Some(4) {
                    let _ = sh("sc", &["start", svc]);
                }
            }
        }
    }
    if failed.is_empty() {
        store::mark_undone(id);
    }
    json!({"ok": failed.is_empty(), "error": failed.first()})
}

// ===================================================================== bloatware
struct Bloat {
    id: &'static str,
    name: &'static str,
    maker: &'static str,
    safety: &'static str, // "safe" | "review"
    note: &'static str,
    patterns: &'static [&'static str],
}

/// Whitelist: only packages matching these patterns are ever offered or removed.
/// Windows core, Microsoft Store, Defender, Edge, Photos, Calculator, Terminal and drivers are not in this list.
static BLOAT: &[Bloat] = &[
    Bloat { id: "clipchamp", name: "Clipchamp", maker: "Microsoft", safety: "safe", note: "", patterns: &["Clipchamp.Clipchamp"] },
    Bloat { id: "candy", name: "Candy Crush and similar games", maker: "King", safety: "safe", note: "", patterns: &["*CandyCrush*"] },
    Bloat { id: "solitaire", name: "Microsoft Solitaire Collection", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.MicrosoftSolitaireCollection"] },
    Bloat { id: "news", name: "Microsoft News", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.BingNews"] },
    Bloat { id: "weather", name: "Weather", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.BingWeather"] },
    Bloat { id: "tips", name: "Tips and Get Help", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.Getstarted", "Microsoft.GetHelp"] },
    Bloat { id: "officehub", name: "Microsoft 365 (Office) hub", maker: "Microsoft", safety: "safe", note: "Only the launcher, not Office itself", patterns: &["Microsoft.MicrosoftOfficeHub"] },
    Bloat { id: "feedback", name: "Feedback Hub", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.WindowsFeedbackHub"] },
    Bloat { id: "skype", name: "Skype (consumer)", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.SkypeApp"] },
    Bloat { id: "mixed", name: "Mixed Reality Portal and 3D Viewer", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.MixedReality.Portal", "Microsoft.Microsoft3DViewer"] },
    Bloat { id: "people", name: "People", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.People"] },
    Bloat { id: "maps", name: "Maps", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.WindowsMaps"] },
    Bloat { id: "cortana", name: "Cortana", maker: "Microsoft", safety: "safe", note: "", patterns: &["Microsoft.549981C3F5F10"] },
    Bloat { id: "social", name: "Preinstalled social and video apps", maker: "Third party", safety: "safe", note: "TikTok, Facebook, Instagram, LinkedIn, Netflix, Prime Video, Spotify, Disney+",
        patterns: &["*TikTok*", "*Facebook*", "*Instagram*", "*LinkedIn*", "4DF9E0F8.Netflix", "AmazonVideo.PrimeVideo", "SpotifyAB.SpotifyMusic", "Disney.*"] },
    Bloat { id: "xbox", name: "Xbox app and Game Bar", maker: "Microsoft", safety: "review", note: "Needed by some games", patterns: &["Microsoft.GamingApp", "Microsoft.XboxGamingOverlay", "Microsoft.Xbox.TCUI"] },
    Bloat { id: "copilot", name: "Copilot app", maker: "Microsoft", safety: "review", note: "", patterns: &["Microsoft.Copilot", "Microsoft.Windows.Ai.Copilot.Provider"] },
    Bloat { id: "phone", name: "Phone Link", maker: "Microsoft", safety: "review", note: "Used to connect your phone", patterns: &["Microsoft.YourPhone"] },
    Bloat { id: "teams", name: "Microsoft Teams (personal)", maker: "Microsoft", safety: "review", note: "", patterns: &["MicrosoftTeams", "MSTeams"] },
    Bloat { id: "media", name: "Media Player and Films & TV", maker: "Microsoft", safety: "review", note: "You may use these to play music or video", patterns: &["Microsoft.ZuneMusic", "Microsoft.ZuneVideo"] },
    Bloat { id: "todo", name: "Microsoft To Do", maker: "Microsoft", safety: "review", note: "", patterns: &["Microsoft.Todos"] },
    Bloat { id: "outlook", name: "Outlook (new)", maker: "Microsoft", safety: "review", note: "", patterns: &["Microsoft.OutlookForWindows"] },
    Bloat { id: "automate", name: "Power Automate", maker: "Microsoft", safety: "review", note: "", patterns: &["Microsoft.PowerAutomateDesktop"] },
];

/// Case-insensitive match with `*` at the start and/or end ("*Foo*", "Foo.*", "Foo").
fn like(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.to_lowercase(), name.to_lowercase());
    let (starts, ends) = (p.starts_with('*'), p.ends_with('*'));
    let core = p.trim_matches('*');
    match (starts, ends) {
        (true, true) => n.contains(core),
        (true, false) => n.ends_with(core),
        (false, true) => n.starts_with(core),
        (false, false) => n == core,
    }
}

/// Installed package names are interpolated into a script, so only plain package-name characters pass.
fn safe_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn installed_packages() -> Option<Vec<(String, String)>> {
    let raw = ps_json(
        "ConvertTo-Json -Compress -InputObject @(Get-AppxPackage -AllUsers | Where-Object { -not $_.NonRemovable -and -not $_.IsFramework } | ForEach-Object { [pscustomobject]@{ name=$_.Name; version=[string]$_.Version } } | Sort-Object name -Unique)",
        60,
    )?;
    Some(as_list(&raw).iter().map(|p| (s(g(p, "name")), s(g(p, "version")))).filter(|(n, _)| safe_name(n)).collect())
}

fn installed_for(b: &Bloat, pkgs: &[(String, String)]) -> Vec<String> {
    pkgs.iter().filter(|(n, _)| b.patterns.iter().any(|p| like(p, n))).map(|(n, _)| n.clone()).collect()
}

pub fn bloat_scan() -> Value {
    if !IS_WIN {
        return json!({"demo": true, "items": [
            {"id": "clipchamp", "name": "Clipchamp", "maker": "Microsoft", "safety": "safe", "note": "", "packages": ["Clipchamp.Clipchamp"]},
            {"id": "solitaire", "name": "Microsoft Solitaire Collection", "maker": "Microsoft", "safety": "safe", "note": "", "packages": ["Microsoft.MicrosoftSolitaireCollection"]},
            {"id": "xbox", "name": "Xbox app and Game Bar", "maker": "Microsoft", "safety": "review", "note": "Needed by some games", "packages": ["Microsoft.GamingApp"]}]});
    }
    let Some(pkgs) = installed_packages() else { return json!({"error": "Could not list installed apps."}) };
    let items: Vec<Value> = BLOAT.iter()
        .filter_map(|b| {
            let found = installed_for(b, &pkgs);
            (!found.is_empty()).then(|| json!({"id": b.id, "name": b.name, "maker": b.maker, "safety": b.safety, "note": b.note, "packages": found}))
        })
        .collect();
    json!({"demo": false, "items": items})
}

pub fn bloat_remove(ids: &[String]) -> Value {
    if !IS_WIN {
        return json!({"ok": true, "demo": true, "removed": ids.len(), "failed": []});
    }
    let Some(pkgs) = installed_packages() else { return json!({"ok": false, "error": "Could not list installed apps."}) };
    let (mut targets, mut titles): (Vec<String>, Vec<&str>) = (vec![], vec![]);
    for id in ids {
        if let Some(b) = BLOAT.iter().find(|b| b.id == id) {
            let found = installed_for(b, &pkgs);
            if !found.is_empty() {
                titles.push(b.name);
                targets.extend(found);
            }
        }
    }
    if targets.is_empty() {
        return json!({"ok": true, "removed": 0, "failed": []});
    }
    let list = targets.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(",");
    let script = format!(
        "foreach ($n in @({list})) {{ \
           Get-AppxPackage -AllUsers -Name $n | Remove-AppxPackage -AllUsers -ErrorAction SilentlyContinue; \
           Get-AppxProvisionedPackage -Online | Where-Object {{ $_.DisplayName -eq $n }} | Remove-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue | Out-Null }}"
    );
    let (_ok, _out) = ps_run(&script, 600);
    // Verify by looking again instead of trusting the exit code.
    let after = installed_packages().unwrap_or_default();
    let failed: Vec<&String> = targets.iter().filter(|t| after.iter().any(|(n, _)| n == *t)).collect();
    let removed_titles: Vec<&str> = BLOAT.iter().filter(|b| titles.contains(&b.name) && installed_for(b, &after).is_empty()).map(|b| b.name).collect();
    if !removed_titles.is_empty() {
        store::add_history(json!({
            "title": format!("Removed {} app{}", removed_titles.len(), if removed_titles.len() == 1 { "" } else { "s" }),
            "detail": removed_titles.join(", "), "undoable": false,
            "note": "Cannot be undone here. Reinstall from the Microsoft Store, or use a restore point."
        }));
    }
    json!({"ok": failed.is_empty(), "removed": removed_titles.len(), "failed": failed})
}

// ===================================================================== fix scripts
pub struct Fix {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub risk: &'static str,
    pub runnable: bool,
    pub timeout: u64,
    pub script: &'static str,
}

pub static FIXES: &[Fix] = &[
    Fix { id: "wu_reset", name: "Windows Update stuck", desc: "Resets update services and clears the download cache.", risk: "low", runnable: true, timeout: 300, script: r#"
$svcs = 'wuauserv','bits','cryptsvc','msiserver'
Set-Service wuauserv -StartupType Manual
foreach ($s in $svcs) { Stop-Service $s -Force -ErrorAction SilentlyContinue }
$win = $env:SystemRoot
foreach ($p in "$win\SoftwareDistribution", "$win\System32\catroot2") {
  if (Test-Path "$p.bak") { Remove-Item "$p.bak" -Recurse -Force -ErrorAction SilentlyContinue }
  if (Test-Path $p) { Rename-Item $p ((Split-Path $p -Leaf) + '.bak') -ErrorAction SilentlyContinue }
}
foreach ($s in $svcs) { Start-Service $s -ErrorAction SilentlyContinue }
'Update services reset. Open Settings > Windows Update and check for updates again.'
"# },
    Fix { id: "net_reset", name: "No internet or limited access", desc: "Resets Winsock and the IP stack, then flushes DNS.", risk: "low", runnable: true, timeout: 120, script: r#"
netsh winsock reset
netsh int ip reset
ipconfig /flushdns
'Done. Restart the PC to finish the network reset.'
"# },
    Fix { id: "sfc_dism", name: "Damaged system files", desc: "Runs DISM and then SFC in the right order. Can take 15 to 40 minutes.", risk: "low", runnable: true, timeout: 3600, script: r#"
DISM /Online /Cleanup-Image /RestoreHealth
sfc /scannow
"# },
    Fix { id: "disk100", name: "Disk at 100% usage", desc: "Clears temp files, scans the system drive (read only) and shows the busiest processes.", risk: "low", runnable: true, timeout: 900, script: r#"
Remove-Item "$env:TEMP\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:SystemRoot\Temp\*" -Recurse -Force -ErrorAction SilentlyContinue
chkdsk $env:SystemDrive /scan
''
'Busiest processes right now:'
Get-Process | Sort-Object CPU -Descending | Select-Object -First 5 Name, @{n='CPU_s';e={[math]::Round($_.CPU)}}, @{n='RAM_MB';e={[math]::Round($_.WorkingSet64/1MB)}} | Format-Table -AutoSize
"# },
    Fix { id: "shell_reset", name: "Start menu or taskbar frozen", desc: "Re-registers the shell apps and restarts Explorer.", risk: "medium", runnable: true, timeout: 300, script: r#"
foreach ($n in 'Microsoft.Windows.StartMenuExperienceHost','MicrosoftWindows.Client.CBS','Microsoft.Windows.ShellExperienceHost') {
  Get-AppxPackage -AllUsers -Name $n -ErrorAction SilentlyContinue | ForEach-Object { Add-AppxPackage -DisableDevelopmentMode -Register "$($_.InstallLocation)\AppXManifest.xml" -ErrorAction SilentlyContinue }
}
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
'Shell apps registered and Explorer restarted.'
"# },
    Fix { id: "search_reset", name: "Windows Search not working", desc: "Restarts the search service and rebuilds the index.", risk: "low", runnable: true, timeout: 180, script: r#"
Stop-Service WSearch -Force -ErrorAction SilentlyContinue
Remove-Item "$env:ProgramData\Microsoft\Search\Data\Applications\Windows\Windows.edb" -Force -ErrorAction SilentlyContinue
Set-Service WSearch -StartupType Automatic
Start-Service WSearch
'Search service restarted. The index rebuilds in the background and can take a while.'
"# },
    Fix { id: "store_reset", name: "Microsoft Store will not open", desc: "Clears the Store cache and registers it again.", risk: "low", runnable: true, timeout: 180, script: r#"
Get-Process WinStore.App -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\Packages\Microsoft.WindowsStore_8wekyb3d8bbwe\LocalCache\*" -Recurse -Force -ErrorAction SilentlyContinue
Get-AppxPackage -AllUsers -Name Microsoft.WindowsStore | ForEach-Object { Add-AppxPackage -DisableDevelopmentMode -Register "$($_.InstallLocation)\AppXManifest.xml" }
'Store cache cleared and the app registered again.'
"# },
    Fix { id: "appx_fix", name: "Broken Store apps", desc: "Registers again every app that Windows reports as damaged.", risk: "low", runnable: true, timeout: 600, script: r#"
$bad = @(Get-AppxPackage -AllUsers | Where-Object { $_.Status -ne 'Ok' -and $_.InstallLocation })
foreach ($p in $bad) { Add-AppxPackage -DisableDevelopmentMode -Register "$($p.InstallLocation)\AppXManifest.xml" -ErrorAction SilentlyContinue; "Registered: $($p.Name)" }
if (-not $bad) { 'No damaged apps found.' }
"# },
    Fix { id: "wmi_fix", name: "WMI repository damaged", desc: "Salvages the WMI repository.", risk: "medium", runnable: true, timeout: 300, script: r#"
winmgmt /salvagerepository
winmgmt /verifyrepository
"# },
    Fix { id: "printer", name: "Printer stuck in queue", desc: "Stops the spooler, clears the queue and starts it again.", risk: "low", runnable: true, timeout: 120, script: r#"
Stop-Service Spooler -Force
Remove-Item "$env:SystemRoot\System32\spool\PRINTERS\*" -Force -ErrorAction SilentlyContinue
Start-Service Spooler
'Print queue cleared.'
"# },
    Fix { id: "audio", name: "No sound", desc: "Restarts the Windows audio services.", risk: "low", runnable: true, timeout: 120, script: r#"
Restart-Service AudioEndpointBuilder -Force
Start-Service Audiosrv -ErrorAction SilentlyContinue
'Audio services restarted. Check the output device in the volume menu.'
"# },
    Fix { id: "bluetooth", name: "Bluetooth missing or not working", desc: "Restarts the Bluetooth services.", risk: "low", runnable: true, timeout: 120, script: r#"
foreach ($s in 'bthserv','BthAvctpSvc') { Restart-Service $s -Force -ErrorAction SilentlyContinue }
'Bluetooth services restarted. Turn Bluetooth off and on again in Settings.'
"# },
    Fix { id: "icons", name: "Icons or thumbnails broken", desc: "Rebuilds the icon and thumbnail cache.", risk: "low", runnable: true, timeout: 120, script: r#"
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Start-Sleep 1
Remove-Item "$env:LOCALAPPDATA\IconCache.db" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache_*.db","$env:LOCALAPPDATA\Microsoft\Windows\Explorer\thumbcache_*.db" -Force -ErrorAction SilentlyContinue
Start-Process explorer.exe
'Icon and thumbnail cache rebuilt.'
"# },
    Fix { id: "boot", name: "Windows will not boot", desc: "Rebuilds the boot configuration. Only works from the WinPE or recovery environment, so it is shown here to read, not to run.", risk: "high", runnable: false, timeout: 0, script: r#"
bootrec /fixmbr
bootrec /fixboot
bootrec /scanos
bootrec /rebuildbcd
"# },
];

pub fn fixes() -> Value {
    Value::Array(FIXES.iter().map(|f| json!({"id": f.id, "name": f.name, "desc": f.desc, "risk": f.risk, "runnable": f.runnable})).collect())
}

pub fn fix_script(id: &str) -> Value {
    match FIXES.iter().find(|f| f.id == id) {
        Some(f) => json!({"ok": true, "script": f.script.trim()}),
        None => json!({"ok": false}),
    }
}

pub fn fix_run(id: &str) -> Value {
    let Some(f) = FIXES.iter().find(|f| f.id == id) else { return json!({"ok": false, "output": "Unknown fix"}) };
    if !f.runnable {
        return json!({"ok": false, "output": "This fix cannot run inside Windows."});
    }
    let (ok, out) = ps_run(f.script, f.timeout);
    store::add_history(json!({
        "title": f.name, "detail": if ok { "Completed" } else { "Finished with errors" }, "undoable": false,
        "note": if id == "net_reset" { "Cannot be undone. A restore point can bring back older settings." } else { "" }
    }));
    json!({"ok": ok, "output": out})
}

// ===================================================================== restore point
const RP_LAST: &str = r#"
$rp = Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object SequenceNumber -Descending | Select-Object -First 1
if ($rp) { [pscustomobject]@{ when=[Management.ManagementDateTimeConverter]::ToDateTime($rp.CreationTime).ToString('yyyy-MM-dd HH:mm'); name=$rp.Description } | ConvertTo-Json -Compress } else { '{}' }
"#;

pub fn restore_info() -> Value {
    if !IS_WIN {
        return json!({"demo": true, "last": "2026-10-26 10:11"});
    }
    let v = ps_json(RP_LAST, 30).unwrap_or(Value::Null);
    json!({"last": g(&v, "when")})
}

pub fn restore_create() -> Value {
    if !IS_WIN {
        return json!({"ok": true, "demo": true});
    }
    // Windows normally allows one restore point per 24 hours; the frequency value is lifted for this call and put back.
    let script = r#"
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$old = (Get-ItemProperty $k -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency
try {
  Enable-ComputerRestore -Drive "$env:SystemDrive\"
  Set-ItemProperty $k -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord
  Checkpoint-Computer -Description 'DeepScan restore point' -RestorePointType MODIFY_SETTINGS
  'Restore point created.'
} finally {
  if ($null -eq $old) { Remove-ItemProperty $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }
  else { Set-ItemProperty $k -Name SystemRestorePointCreationFrequency -Value $old -Type DWord }
}
"#;
    let (ok, out) = ps_run(script, 180);
    if ok {
        store::add_history(json!({"title": "Created a restore point", "detail": "DeepScan restore point", "undoable": false}));
    }
    json!({"ok": ok, "message": if ok { "Restore point created.".to_string() } else { out }})
}

pub fn restore_open() -> Value {
    json!({"ok": IS_WIN && Command::new("rstrui.exe").spawn().is_ok()})
}

pub fn restart_now() -> Value {
    json!({"ok": sh("shutdown", &["/r", "/t", "15", "/c", "DeepScan: restarting to finish pending changes"])})
}

// ===================================================================== health checks
fn check(id: &str, name: &str, status: &str, detail: &str, advice: &str, fix: Option<&str>) -> Value {
    json!({"id": id, "name": name, "status": status, "detail": detail, "advice": advice, "fix": fix})
}

pub const CHECK_IDS: &[(&str, &str, bool)] = &[
    ("reboot", "Pending restart", false),
    ("wu", "Windows Update service", false),
    ("appx", "Microsoft Store apps", true),
    ("wmi", "WMI repository", true),
    ("dism", "Windows image (component store)", true),
    ("disk", "Disk health", false),
    ("sfc", "System file integrity", true), // true = heavy: runs only when the user asks
];

pub fn health_list() -> Value {
    Value::Array(CHECK_IDS.iter().map(|(id, name, slow)| json!({"id": id, "name": name, "slow": slow})).collect())
}

pub fn health_check(id: &str) -> Value {
    let name = CHECK_IDS.iter().find(|c| c.0 == id).map(|c| c.1).unwrap_or("Check");
    if !IS_WIN {
        return match id {
            "reboot" => check(id, name, "warn", "A restart is waiting from the last update", "", Some("restart")),
            "appx" => check(id, name, "warn", "1 app is registered wrongly (Photos)", "", Some("appx_fix")),
            "dism" => check(id, name, "warn", "Repairable corruption found", "", Some("sfc_dism")),
            "sfc" => check(id, name, "ok", "No integrity violations found", "", None),
            _ => check(id, name, "ok", "Looks fine", "", None),
        };
    }
    let unknown = |why: &str| check(id, name, "unknown", why, "", None);
    match id {
        "reboot" => {
            let script = "[pscustomobject]@{ p = ((Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending') -or (Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired')) } | ConvertTo-Json -Compress";
            match ps_json(script, 20) {
                Some(v) if g(&v, "p") == true => check(id, name, "warn", "A restart is waiting from the last update", "Restart the PC to finish installing updates.", Some("restart")),
                Some(_) => check(id, name, "ok", "None", "", None),
                None => unknown("Could not check"),
            }
        }
        "wu" => match ps_json("$s = Get-Service wuauserv; [pscustomobject]@{ st=[string]$s.Status; start=[string]$s.StartType } | ConvertTo-Json -Compress", 20) {
            Some(v) if s(g(&v, "start")) == "Disabled" => check(id, name, "warn", "The service is disabled", "Updates cannot install while it is disabled.", Some("wu_reset")),
            Some(v) => check(id, name, "ok", &format!("{} ({})", s(g(&v, "st")), s(g(&v, "start"))), "", None),
            None => unknown("Could not check"),
        },
        "appx" => match ps_json("ConvertTo-Json -Compress -InputObject @(Get-AppxPackage -AllUsers | Where-Object { $_.Status -ne 'Ok' } | ForEach-Object { $_.Name })", 60) {
            Some(v) => {
                let bad = as_list(&v);
                if bad.is_empty() {
                    check(id, name, "ok", "All apps are registered correctly", "", None)
                } else {
                    let names: Vec<String> = bad.iter().take(3).map(s).collect();
                    check(id, name, "warn", &format!("{} app(s) registered wrongly ({})", bad.len(), names.join(", ")), "", Some("appx_fix"))
                }
            }
            None => unknown("Could not check"),
        },
        "wmi" => {
            let (_, out) = ps_run("winmgmt /verifyrepository", 60);
            let o = out.to_lowercase();
            if o.contains("inconsistent") {
                check(id, name, "warn", "The repository is inconsistent", "", Some("wmi_fix"))
            } else if o.contains("consistent") {
                check(id, name, "ok", "Consistent", "", None)
            } else {
                unknown("Could not read the result")
            }
        }
        "dism" => {
            let (_, out) = ps_run("DISM /Online /Cleanup-Image /CheckHealth", 180);
            let o = out.to_lowercase();
            if o.contains("no component store corruption") {
                check(id, name, "ok", "No corruption detected", "", None)
            } else if o.contains("repairable") {
                check(id, name, "warn", "Repairable corruption found", "", Some("sfc_dism"))
            } else if o.contains("not repairable") {
                check(id, name, "bad", "Corruption that cannot be repaired here", "Consider an in-place repair install.", None)
            } else {
                unknown("Needs administrator or the result could not be read")
            }
        }
        "disk" => match ps_json("ConvertTo-Json -Compress -InputObject @(Get-Volume | Where-Object { $_.DriveLetter } | ForEach-Object { [pscustomobject]@{ l=[string]$_.DriveLetter; h=[string]$_.HealthStatus } })", 30) {
            Some(v) => {
                let bad: Vec<String> = as_list(&v).iter().filter(|x| s(g(x, "h")) != "Healthy").map(|x| format!("{}:", s(g(x, "l")))).collect();
                if bad.is_empty() { check(id, name, "ok", "All volumes report Healthy", "", None) } else { check(id, name, "warn", &format!("Not healthy: {}", bad.join(", ")), "Back up your files, then run chkdsk on these drives.", None) }
            }
            None => unknown("Could not check"),
        },
        "sfc" => {
            let (_, out) = ps_run("sfc /verifyonly", 1500);
            let o = out.to_lowercase();
            if o.contains("did not find any integrity violations") {
                check(id, name, "ok", "No integrity violations found", "", None)
            } else if o.contains("found integrity violations") {
                check(id, name, "warn", "Some protected files are damaged", "", Some("sfc_dism"))
            } else {
                unknown("Could not read the result")
            }
        }
        _ => unknown("Unknown check"),
    }
}

pub fn repair_state() -> Value {
    json!({"tweaks": tweaks(), "fixes": fixes(), "checks": health_list(), "history": store::repair_history()})
}
