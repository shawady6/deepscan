#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deepscan_core as core;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::async_runtime::spawn_blocking;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

struct Live(Mutex<core::LiveState>);

/// Heavy work runs on a worker thread so the window never freezes. A panic becomes an error value, not a crash.
async fn blocking<F: FnOnce() -> Value + Send + 'static>(f: F) -> Value {
    spawn_blocking(f).await.unwrap_or_else(|e| json!({"error": e.to_string()}))
}

#[tauri::command]
async fn scan(step: String) -> Value {
    blocking(move || core::scan(&step)).await
}

#[tauri::command]
async fn updates() -> Value {
    blocking(|| core::collect::updates()).await
}

#[tauri::command]
async fn public_ip() -> Value {
    blocking(|| core::collect::public_ip()).await
}

#[tauri::command]
async fn run_speedtest() -> Value {
    blocking(|| core::run_speedtest()).await
}

#[tauri::command]
fn live(state: State<Live>) -> Value {
    match state.0.lock() {
        Ok(mut s) => core::collect::live(&mut s),
        Err(_) => json!({}),
    }
}

#[tauri::command]
fn speed_history() -> Value {
    Value::Array(core::store::speed_history())
}

#[tauri::command]
fn app_info() -> Value {
    core::app_info()
}

#[tauri::command]
fn set_settings(patch: Value) -> Value {
    core::store::set_settings(&patch)
}

/// Save text or a data: URL (PNG) through a Save dialog. Returns the saved path, or "" if cancelled.
#[tauri::command]
async fn save_file(app: AppHandle, name: String, content: String, _kind: Option<String>) -> Result<String, String> {
    let picked = app.dialog().file().set_file_name(&name).blocking_save_file();
    let Some(p) = picked else { return Ok(String::new()) };
    let path = p.into_path().map_err(|e| e.to_string())?;
    core::write_file(&path.to_string_lossy(), &content)?;
    Ok(path.to_string_lossy().to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Live(Mutex::new(core::LiveState::new())))
        .invoke_handler(tauri::generate_handler![
            scan, updates, public_ip, run_speedtest, live, speed_history, app_info, set_settings, save_file
        ])
        .run(tauri::generate_context!())
        .expect("failed to start DeepScan");
}
