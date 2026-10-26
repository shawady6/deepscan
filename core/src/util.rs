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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(40)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    reader.join().ok()
}

/// Run a PowerShell script that prints JSON and return the parsed value.
/// The script travels as -EncodedCommand (UTF-16LE base64) so quoting never breaks.
pub fn ps_json(script: &str, timeout_s: u64) -> Option<Value> {
    if !IS_WIN {
        return None;
    }
    use base64::Engine;
    let full = format!(
        "$ProgressPreference='SilentlyContinue';$ErrorActionPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;{script}"
    );
    let bytes: Vec<u8> = full.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
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
