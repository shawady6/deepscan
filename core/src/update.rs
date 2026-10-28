//! In-app updates: look for a newer GitHub release, download DeepScan.exe, verify it, swap it in and restart.
//!
//! Integrity: the release must carry `DeepScan.exe.sha256`; the download is refused if the hash does not match.
//! (This protects against corrupted or truncated downloads. Code signing is a separate step.)
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = "shawady6/deepscan";
const EXE: &str = "DeepScan.exe";
const MAX_BYTES: u64 = 200 * 1024 * 1024;

/// "v1.2.3", "1.2" or "1.2.3-beta" to (1, 2, 3). Anything else is None.
pub fn parse_ver(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut it = core.split('.');
    let mut next = || it.next().map(|p| p.parse::<u64>().ok()).unwrap_or(Some(0));
    Some((next()?, next()?, next()?))
}

/// Turn the GitHub "latest release" JSON into what the UI needs.
pub fn parse_release(rel: &Value, current: &str) -> Value {
    let tag = rel["tag_name"].as_str().unwrap_or("");
    let (Some(latest), Some(cur)) = (parse_ver(tag), parse_ver(current)) else {
        return json!({"ok": false, "error": "Could not read the latest version."});
    };
    let assets = rel["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |name: &str| assets.iter().find(|a| a["name"] == name).and_then(|a| a["browser_download_url"].as_str()).map(String::from);
    let size = assets.iter().find(|a| a["name"] == EXE).and_then(|a| a["size"].as_u64());
    json!({
        "ok": true, "current": current, "latest": tag.trim_start_matches(['v', 'V']), "available": latest > cur,
        "notes": rel["body"].as_str().unwrap_or("").chars().take(1500).collect::<String>(),
        "page": rel["html_url"], "asset_url": url_of(EXE), "sha_url": url_of("DeepScan.exe.sha256"), "size": size,
    })
}

pub fn check() -> Value {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url).set("User-Agent", "DeepScan").set("Accept", "application/vnd.github+json").timeout(Duration::from_secs(8)).call();
    match resp {
        Ok(r) => match r.into_string().ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            Some(v) => parse_release(&v, VERSION),
            None => json!({"ok": false, "error": "Unexpected answer from GitHub."}),
        },
        Err(ureq::Error::Status(404, _)) => json!({"ok": false, "error": "No published release found yet.", "none": true}),
        Err(ureq::Error::Status(403, _)) => json!({"ok": false, "error": "GitHub is limiting requests right now. Try again later."}),
        Err(_) => json!({"ok": false, "error": "Could not reach GitHub. Check your internet connection."}),
    }
}

fn exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

fn fetch_text(url: &str) -> Result<String, String> {
    ureq::get(url).set("User-Agent", "DeepScan").timeout(Duration::from_secs(20)).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

/// Download, verify and put the new exe in place of the running one. Returns the path to start.
/// The old exe is renamed to `DeepScan.old.exe` (Windows allows renaming a running exe, not overwriting it).
pub fn install(asset_url: &str, sha_url: Option<&str>) -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("Updating is only available on Windows.".into());
    }
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    if !asset_url.starts_with(&prefix) || sha_url.map(|u| !u.starts_with(&prefix)).unwrap_or(true) {
        return Err("The update does not come from the official release, or it has no checksum. It was not installed.".into());
    }
    let current = exe_path()?;
    let staged = current.with_file_name("DeepScan.update.exe");
    let old = current.with_extension("old.exe");

    let expected = fetch_text(sha_url.unwrap())?.split_whitespace().next().unwrap_or("").to_lowercase();
    let resp = ureq::get(asset_url).set("User-Agent", "DeepScan").timeout(Duration::from_secs(600)).call().map_err(|e| format!("Download failed: {e}"))?;
    let (mut reader, mut hasher, mut total) = (resp.into_reader().take(MAX_BYTES + 1), Sha256::new(), 0u64);
    let mut file = std::fs::File::create(&staged).map_err(|e| format!("Cannot write next to the app: {e}"))?;
    let mut buf = vec![0u8; 65536];
    let mut head = [0u8; 2];
    loop {
        let n = reader.read(&mut buf).map_err(|e| { let _ = std::fs::remove_file(&staged); format!("Download failed: {e}") })?;
        if n == 0 { break; }
        if total == 0 && n >= 2 { head = [buf[0], buf[1]]; }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        total += n as u64;
    }
    drop(file);
    let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if total > MAX_BYTES || total < 1_000_000 || &head != b"MZ" || got != expected {
        let _ = std::fs::remove_file(&staged);
        return Err("The downloaded file did not pass the integrity check. Nothing was changed.".into());
    }
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&current, &old).map_err(|e| format!("Cannot replace the running app: {e}"))?;
    if let Err(e) = std::fs::rename(&staged, &current) {
        let _ = std::fs::rename(&old, &current); // put the old one back
        return Err(format!("Cannot place the new version: {e}"));
    }
    Ok(current)
}

/// Remove leftovers from a previous update (best effort).
pub fn cleanup() {
    if let Ok(cur) = exe_path() {
        let _ = std::fs::remove_file(cur.with_extension("old.exe"));
        let _ = std::fs::remove_file(cur.with_file_name("DeepScan.update.exe"));
    }
}
