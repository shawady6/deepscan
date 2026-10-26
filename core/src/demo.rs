//! Sample data so the UI can be developed on any OS (same shapes as the real collectors).
use serde_json::Value;

fn load(t: &str) -> Value {
    serde_json::from_str(t).expect("bad demo json")
}
pub fn hardware() -> Value { load(include_str!("demo/hardware.json")) }
pub fn storage() -> Value { load(include_str!("demo/storage.json")) }
pub fn network() -> Value { load(include_str!("demo/network.json")) }
pub fn security() -> Value { load(include_str!("demo/security.json")) }
pub fn updates() -> Value { load(include_str!("demo/updates.json")) }
pub fn software() -> Value { load(include_str!("demo/software.json")) }
