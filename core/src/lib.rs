//! DeepScan core: collectors, speed test and settings. No GUI code here, so it builds and tests anywhere.
pub mod collect;
pub mod repair;
mod demo;
pub mod speed;
pub mod store;
pub mod util;

use base64::Engine;
use serde_json::{json, Value};
use std::time::Instant;

pub use collect::LiveState;

/// Run one scan step ("hardware", "storage", "network", "security", "software").
pub fn scan(step: &str) -> Value {
    let t = Instant::now();
    let mut data = match step {
        "hardware" => collect::hardware(),
        "storage" => collect::storage(),
        "network" => collect::network(),
        "security" => collect::security(),
        "software" => collect::software(),
        other => return json!({"error": format!("Unknown step {other}")}),
    };
    if step == "hardware" {
        *collect::HW_CACHE.lock().unwrap() = Some(data.clone());
    } else if step == "storage" {
        *collect::ST_CACHE.lock().unwrap() = Some(data.clone());
    }
    data["_seconds"] = json!((t.elapsed().as_secs_f64() * 10.0).round() / 10.0);
    data
}

pub fn run_speedtest() -> Value {
    let mut r = speed::run();
    if r["ok"] == true {
        store::add_speed_result(&r);
    }
    r["history"] = Value::Array(store::speed_history());
    r
}

pub fn app_info() -> Value {
    json!({"windows": util::IS_WIN, "admin": util::is_admin(), "settings": store::get_settings()})
}

/// Write text or a `data:` URL (PNG) to `path`.
pub fn write_file(path: &str, content: &str) -> Result<(), String> {
    if let Some(rest) = content.strip_prefix("data:") {
        let b64 = rest.split_once(',').map(|x| x.1).ok_or("bad data url")?;
        let raw = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| e.to_string())?;
        std::fs::write(path, raw).map_err(|e| e.to_string())
    } else {
        std::fs::write(path, content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_steps_return_data() {
        for s in ["hardware", "storage", "network", "security", "software"] {
            let v = scan(s);
            assert!(v.get("error").is_none(), "{s}: {v}");
            assert!(v["_seconds"].is_number());
        }
        assert!(scan("nope").get("error").is_some());
    }

    #[test]
    fn live_works() {
        let mut st = LiveState::new();
        let _ = collect::live(&mut st);
        let v = collect::live(&mut st);
        assert!(v["mem_total_gb"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn ping_parse() {
        assert_eq!(super::collect::__ms("Reply from 1.1.1.1: bytes=32 time=14ms TTL=57"), Some(14.0));
        assert_eq!(super::collect::__ms("Reply from 1.1.1.1: bytes=32 time<1ms TTL=57"), Some(1.0));
        assert_eq!(super::collect::__ms("64 bytes from x: icmp_seq=1 ttl=57 time=0.52 ms"), Some(0.52));
    }

    #[test]
    fn repair_catalog_is_consistent() {
        use crate::repair::*;
        let ids: Vec<_> = TWEAKS.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), { let mut x = ids.clone(); x.sort(); x.dedup(); x.len() });
        assert!(FIXES.iter().all(|f| !f.script.trim().is_empty()));
        // every fix referenced by a health check exists
        for c in CHECK_IDS { let r = health_check(c.0); if let Some(f) = r["fix"].as_str() { assert!(f == "restart" || FIXES.iter().any(|x| x.id == f), "{f}"); } }
        assert_eq!(repair_state()["tweaks"].as_array().unwrap().len(), TWEAKS.len());
        assert!(bloat_scan()["items"].is_array());
    }

    #[test]
    fn data_url_roundtrip() {
        let p = std::env::temp_dir().join("ds_test.png");
        write_file(p.to_str().unwrap(), "data:image/png;base64,aGVsbG8=").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"hello");
    }
}
