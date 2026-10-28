//! Settings and speed-test history, kept in the user's profile.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

fn dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    } else {
        std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config"))
    };
    let p = base.unwrap_or_else(std::env::temp_dir).join("DeepScan");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn read(name: &str, default: Value) -> Value {
    std::fs::read_to_string(dir().join(name)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(default)
}

fn write(name: &str, data: &Value) {
    let _g = LOCK.lock();
    if let Ok(t) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(dir().join(name), t);
    }
}

pub fn get_settings() -> Value {
    let mut s = json!({"theme": "dark", "hide_admin_notice": false, "auto_update_check": true, "skip_version": ""});
    if let Value::Object(saved) = read("settings.json", json!({})) {
        for (k, v) in saved {
            s[k] = v;
        }
    }
    s
}

pub fn set_settings(patch: &Value) -> Value {
    let mut s = get_settings();
    if let Some(p) = patch.as_object() {
        for (k, v) in p {
            if matches!(k.as_str(), "theme" | "hide_admin_notice" | "auto_update_check" | "skip_version") {
                s[k.as_str()] = v.clone();
            }
        }
    }
    write("settings.json", &s);
    s
}

fn last(v: Value, n: usize) -> Vec<Value> {
    let a = v.as_array().cloned().unwrap_or_default();
    a[a.len().saturating_sub(n)..].to_vec()
}

pub fn speed_history() -> Vec<Value> {
    last(read("speed_history.json", json!([])), 30)
}

pub fn add_speed_result(result: &Value) {
    let mut hist = read("speed_history.json", json!([])).as_array().cloned().unwrap_or_default();
    hist.push(result.clone());
    let keep = hist[hist.len().saturating_sub(100)..].to_vec();
    write("speed_history.json", &Value::Array(keep));
}

// ------------------------------------------------------------ repair history
pub fn repair_history() -> Vec<Value> {
    let a = read("repair_history.json", json!([])).as_array().cloned().unwrap_or_default();
    a.into_iter().rev().take(20).collect() // newest first
}

pub fn add_history(mut entry: Value) -> String {
    let id = chrono::Local::now().timestamp_millis().to_string();
    entry["id"] = json!(id);
    entry["ts"] = json!(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
    entry["undone"] = json!(false);
    let mut all = read("repair_history.json", json!([])).as_array().cloned().unwrap_or_default();
    all.push(entry);
    let keep = all[all.len().saturating_sub(100)..].to_vec();
    write("repair_history.json", &Value::Array(keep));
    id
}

pub fn history_entry(id: &str) -> Option<Value> {
    read("repair_history.json", json!([])).as_array()?.iter().find(|e| e["id"] == id).cloned()
}

pub fn mark_undone(id: &str) {
    let mut all = read("repair_history.json", json!([])).as_array().cloned().unwrap_or_default();
    for e in all.iter_mut() {
        if e["id"] == id {
            e["undone"] = json!(true);
        }
    }
    write("repair_history.json", &Value::Array(all));
}
