//! Small helpers shared by the collectors.
use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const IS_WIN: bool = cfg!(windows);

static NULL: Value = Value::Null;

/// `v[k]` that never panics: missing keys give `null`.
pub fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NULL)
}

/// Trimmed string; null gives "".
pub fn s(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(x) => x.trim().to_string(),
        other => other.to_string().trim().to_string(),
    }
}

/// PowerShell prints a single object instead of a one-item array; normalise to a list.
pub fn as_list(v: &Value) -> Vec<Value> {
    match v {
        Value::Null => vec![],
        Value::Array(a) => a.clone(),
        other => vec![other.clone()],
    }
}

pub fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

pub fn round(x: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (x * m).round() / m
}

/// Bytes to GiB, rounded. `null` when the input is not a number.
pub fn gb(v: &Value, digits: i32) -> Value {
    match v.as_f64() {
        Some(n) => serde_json::json!(round(n / 1_073_741_824.0, digits)),
        None => Value::Null,
    }
}

pub fn is_admin() -> bool {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0
    }
    #[cfg(not(windows))]
    false
}

/// Run a command, return stdout, kill it after `timeout`.
pub fn run_cmd(cmd: &mut Command, timeout: Duration) -> Option<Vec<u8>> {
    run_cmd_status(cmd, timeout).map(|(_, out)| out)
}

/// Like `run_cmd` but also returns whether the command exited with success (false when it had to be killed).
pub fn run_cmd_status(cmd: &mut Command, timeout: Duration) -> Option<(bool, Vec<u8>)> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000 | 0x0000_4000); // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS: scans must not slow the PC down
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let mut success = false;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                success = st.success();
                break;
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    reader.join().ok().map(|b| (success, b))
}

/// Command output as text: handles UTF-16 (sfc.exe) and drops NUL characters.
pub fn decode_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[1] == 0 {
        let u: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&u).replace('\0', "");
    }
    String::from_utf8_lossy(bytes).replace('\0', "")
}

fn encode_ps(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Run a PowerShell script for its effect. Returns (success, combined output text).
/// Errors stop the script (`$ErrorActionPreference='Stop'`); scripts mark best-effort steps themselves.
pub fn ps_run(script: &str, timeout_s: u64) -> (bool, String) {
    if !IS_WIN {
        return (false, "Windows only".into());
    }
    let full = format!(
        "$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;$ErrorActionPreference='Stop';\
         try {{ & {{ {script} }} 2>&1 | Out-String -Width 200 }} catch {{ ($_ | Out-String); exit 1 }}"
    );
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encode_ps(&full)]);
    match run_cmd_status(&mut cmd, Duration::from_secs(timeout_s)) {
        Some((ok, out)) => (ok, decode_output(&out).trim().to_string()),
        None => (false, "Could not start PowerShell.".into()),
    }
}

/// Run a PowerShell script that prints JSON and return the parsed value.
/// The script travels as -EncodedCommand (UTF-16LE base64) so quoting never breaks.
pub fn ps_json(script: &str, timeout_s: u64) -> Option<Value> {
    if !IS_WIN {
        return None;
    }
    let full = format!(
        "$ProgressPreference='SilentlyContinue';$ErrorActionPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;{script}"
    );
    let encoded = encode_ps(&full);
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded]);
    let out = run_cmd(&mut cmd, Duration::from_secs(timeout_s))?;
    let text = String::from_utf8_lossy(&out);
    let text = text.trim().trim_start_matches('\u{feff}');
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(text).ok()
}
