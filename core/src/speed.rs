//! Internet speed test against Cloudflare's public endpoints (runs only on button press).
use serde_json::{json, Value};
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOWN_URL: &str = "https://speed.cloudflare.com/__down?bytes=12000000";
const UP_URL: &str = "https://speed.cloudflare.com/__up";
const WORKERS: usize = 4;
const UA: &str = "DeepScan/1.0";

fn latency() -> Option<(f64, f64)> {
    let addr = "speed.cloudflare.com:443".to_socket_addrs().ok()?.next()?;
    let mut samples = vec![];
    for _ in 0..4 {
        let t = Instant::now();
        if TcpStream::connect_timeout(&addr, Duration::from_secs(3)).is_ok() {
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    if samples.is_empty() {
        return None;
    }
    let avg = samples.iter().sum::<f64>() / samples.len() as f64;
    let jitter = samples.windows(2).map(|w| (w[0] - w[1]).abs()).sum::<f64>() / (samples.len().max(2) - 1) as f64;
    Some((avg.round(), (jitter * 10.0).round() / 10.0))
}

fn download(secs: u64) -> f64 {
    let total = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let hs: Vec<_> = (0..WORKERS)
        .map(|_| {
            let total = total.clone();
            std::thread::spawn(move || {
                let Ok(resp) = ureq::get(DOWN_URL).set("User-Agent", UA).timeout(Duration::from_secs(12)).call() else { return };
                let mut r = resp.into_reader();
                let mut buf = vec![0u8; 65536];
                while Instant::now() < deadline {
                    match r.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            total.fetch_add(n as u64, Ordering::Relaxed);
                        }
                    }
                }
            })
        })
        .collect();
    hs.into_iter().for_each(|h| {
        let _ = h.join();
    });
    total.load(Ordering::Relaxed) as f64 * 8.0 / start.elapsed().as_secs_f64().max(0.001) / 1e6
}

fn upload() -> f64 {
    let payload = Arc::new(vec![b'0'; 2_000_000]);
    let sent = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let hs: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (p, sent) = (payload.clone(), sent.clone());
            std::thread::spawn(move || {
                if ureq::post(UP_URL).set("User-Agent", UA).timeout(Duration::from_secs(15)).send_bytes(&p).is_ok() {
                    sent.fetch_add(p.len() as u64, Ordering::Relaxed);
                }
            })
        })
        .collect();
    hs.into_iter().for_each(|h| {
        let _ = h.join();
    });
    sent.load(Ordering::Relaxed) as f64 * 8.0 / start.elapsed().as_secs_f64().max(0.001) / 1e6
}

pub fn run() -> Value {
    let Some((ping, jitter)) = latency() else {
        return json!({"ok": false, "error": "No internet connection."});
    };
    let down = download(8);
    let up = upload();
    json!({
        "ok": true,
        "time": chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "down": (down * 10.0).round() / 10.0,
        "up": (up * 10.0).round() / 10.0,
        "ping": ping,
        "jitter": jitter,
    })
}
