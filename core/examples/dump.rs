//! Prints the Repair tab's demo data as JSON (used to test the UI without Windows).
use deepscan_core::repair::*;
fn main() {
    let checks: Vec<_> = CHECK_IDS.iter().map(|c| (c.0, health_check(c.0))).collect();
    println!("{}", serde_json::json!({"state": repair_state(), "checks": checks.into_iter().collect::<std::collections::HashMap<_,_>>(), "bloat": bloat_scan(),
        "script": fix_script("wu_reset")}));
}
