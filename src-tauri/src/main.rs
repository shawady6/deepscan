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

// ---- Defender
#[tauri::command]
async fn defender_status() -> Value {
    blocking(|| core::defender::status()).await
}
#[tauri::command]
async fn defender_pause(minutes: u32) -> Value {
    blocking(move || core::defender::pause(minutes)).await
}
#[tauri::command]
async fn defender_resume() -> Value {
    blocking(|| core::defender::resume()).await
}

// ---- Updates
#[tauri::command]
async fn update_check() -> Value {
    blocking(|| core::update::check()).await
}

/// Download and verify the new version, swap it in, start it and close this instance.
#[tauri::command]
async fn update_install(app: AppHandle, asset_url: String, sha_url: Option<String>) -> Value {
    let r = spawn_blocking(move || core::update::install(&asset_url, sha_url.as_deref())).await;
    match r {
        Ok(Ok(exe)) => match std::process::Command::new(exe).spawn() {
            Ok(_) => {
                app.exit(0);
                json!({"ok": true})
            }
            Err(e) => json!({"ok": false, "error": format!("Updated, but could not start the new version: {e}. Start DeepScan again manually.")}),
        },
        Ok(Err(e)) => json!({"ok": false, "error": e}),
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

// ---- Repair tab
#[tauri::command]
fn repair_state() -> Value {
    core::repair::repair_state()
}
#[tauri::command]
async fn health_check(id: String) -> Value {
    blocking(move || core::repair::health_check(&id)).await
}
#[tauri::command]
async fn bloat_scan() -> Value {
    blocking(|| core::repair::bloat_scan()).await
}
#[tauri::command]
async fn bloat_remove(ids: Vec<String>) -> Value {
    blocking(move || core::repair::bloat_remove(&ids)).await
}
#[tauri::command]
async fn tweaks_apply(ids: Vec<String>) -> Value {
    blocking(move || core::repair::tweaks_apply(&ids)).await
}
#[tauri::command]
async fn history_undo(id: String) -> Value {
    blocking(move || core::repair::history_undo(&id)).await
}
#[tauri::command]
fn fix_script(id: String) -> Value {
    core::repair::fix_script(&id)
}
#[tauri::command]
async fn fix_run(id: String) -> Value {
    blocking(move || core::repair::fix_run(&id)).await
}
#[tauri::command]
async fn restore_info() -> Value {
    blocking(|| core::repair::restore_info()).await
}
#[tauri::command]
async fn restore_create() -> Value {
    blocking(|| core::repair::restore_create()).await
}
#[tauri::command]
async fn restore_open() -> Value {
    blocking(|| core::repair::restore_open()).await
}
#[tauri::command]
async fn restart_now() -> Value {
    blocking(|| core::repair::restart_now()).await
}

fn main() {
    core::update::cleanup();
    std::thread::spawn(core::defender::reconcile);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Live(Mutex::new(core::LiveState::new())))
        .invoke_handler(tauri::generate_handler![
            scan, updates, public_ip, run_speedtest, live, speed_history, app_info, set_settings, save_file,
            repair_state, health_check, bloat_scan, bloat_remove, tweaks_apply, history_undo, fix_script, fix_run,
            restore_info, restore_create, restore_open, restart_now, update_check, update_install,
            defender_status, defender_pause, defender_resume
        ])
        .run(tauri::generate_context!())
        .expect("failed to start DeepScan");
}
