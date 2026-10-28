//! Collectors. Each returns a JSON value shaped exactly like the demo data.
//! On Windows they read the real machine; elsewhere they return demo data.
use crate::demo;
use crate::util::*;
use serde_json::{json, Value};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{Networks, ProcessesToUpdate, System};

pub static HW_CACHE: Mutex<Option<Value>> = Mutex::new(None);
pub static ST_CACHE: Mutex<Option<Value>> = Mutex::new(None);

fn err(msg: &str) -> Value {
    json!({ "error": msg })
}

fn mem_type(n: i64) -> &'static str {
    match n {
        20 => "DDR",
        21 => "DDR2",
        24 => "DDR3",
        26 => "DDR4",
        30 => "LPDDR4",
        34 => "DDR5",
        35 => "LPDDR5",
        _ => "RAM",
    }
}

fn squash(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- hardware
const HW_PS: &str = r#"
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$gpu = @(Get-CimInstance Win32_VideoController | ForEach-Object { [pscustomobject]@{ name=$_.Name; vram=$_.AdapterRAM; driverDate=$(if($_.DriverDate){$_.DriverDate.ToString('yyyy-MM-dd')}else{''}); w=$_.CurrentHorizontalResolution; h=$_.CurrentVerticalResolution; hz=$_.CurrentRefreshRate } })
$board = Get-CimInstance Win32_BaseBoard | Select-Object -First 1
$bios = Get-CimInstance Win32_BIOS | Select-Object -First 1
$mem = @(Get-CimInstance Win32_PhysicalMemory | ForEach-Object { [pscustomobject]@{ slot=$_.DeviceLocator; cap=$_.Capacity; speed=$_.ConfiguredClockSpeed; type=$_.SMBIOSMemoryType; maker=$_.Manufacturer } })
$arr = Get-CimInstance Win32_PhysicalMemoryArray | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem | Select-Object -First 1
$cs = Get-CimInstance Win32_ComputerSystem | Select-Object -First 1
$ver = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').DisplayVersion
$tpm = try { Get-Tpm } catch { $null }
$sb = try { Confirm-SecureBootUEFI } catch { $null }
$therm = @(Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature | ForEach-Object { [math]::Round(($_.CurrentTemperature/10)-273.15,1) })
[pscustomobject]@{
  cpu=[pscustomobject]@{ name=$cpu.Name; cores=$cpu.NumberOfCores; threads=$cpu.NumberOfLogicalProcessors; mhz=$cpu.MaxClockSpeed; socket=$cpu.SocketDesignation; l3=$cpu.L3CacheSize }
  gpu=$gpu
  board=[pscustomobject]@{ maker=$board.Manufacturer; model=$board.Product }
  bios=[pscustomobject]@{ version=$bios.SMBIOSBIOSVersion; date=$(if($bios.ReleaseDate){$bios.ReleaseDate.ToString('yyyy-MM-dd')}else{''}); serial=$bios.SerialNumber }
  mem=$mem; slots=$arr.MemoryDevices
  os=[pscustomobject]@{ name=$os.Caption; build=$os.BuildNumber; arch=$os.OSArchitecture; display=$ver; installed=$(if($os.InstallDate){$os.InstallDate.ToString('yyyy-MM-dd')}else{''}) }
  cs=[pscustomobject]@{ name=$cs.Name; maker=$cs.Manufacturer; model=$cs.Model; domain=$cs.Domain; ram=$cs.TotalPhysicalMemory }
  tpm=$tpm; secureBoot=$sb; thermal=$therm
} | ConvertTo-Json -Depth 5 -Compress
"#;

pub fn hardware() -> Value {
    if !IS_WIN {
        return demo::hardware();
    }
    let Some(raw) = ps_json(HW_PS, 45) else { return err("Could not read hardware information.") };
    let (cpu, board, bios, os, cs) = (g(&raw, "cpu"), g(&raw, "board"), g(&raw, "bios"), g(&raw, "os"), g(&raw, "cs"));

    let gpus: Vec<Value> = as_list(g(&raw, "gpu"))
        .iter()
        .map(|x| {
            let vram = f(g(x, "vram"));
            let vram_txt = if vram >= 4_000_000_000.0 {
                "4 GB or more".to_string()
            } else if vram > 0.0 {
                format!("{} GB", (vram / 1_073_741_824.0).round() as i64)
            } else {
                String::new()
            };
            let disp = if f(g(x, "w")) > 0.0 {
                format!("{} x {} at {} Hz", f(g(x, "w")) as i64, f(g(x, "h")) as i64, f(g(x, "hz")) as i64)
            } else {
                String::new()
            };
            json!({"name": s(g(x, "name")), "vram": vram_txt, "driver_date": s(g(x, "driverDate")), "display": disp})
        })
        .collect();

    let mods: Vec<Value> = as_list(g(&raw, "mem"))
        .iter()
        .map(|m| {
            json!({"slot": s(g(m, "slot")), "size_gb": gb(g(m, "cap"), 0), "type": mem_type(g(m, "type").as_i64().unwrap_or(0)),
                   "speed": g(m, "speed"), "maker": s(g(m, "maker"))})
        })
        .collect();

    let tpm = g(&raw, "tpm");
    let tpm_txt = if tpm.is_null() {
        "Unknown (needs administrator)"
    } else if g(tpm, "TpmPresent").as_bool() == Some(true) && g(tpm, "TpmReady").as_bool() == Some(true) {
        "Present and ready"
    } else if g(tpm, "TpmPresent").as_bool() == Some(true) {
        "Present"
    } else {
        "Not found"
    };
    let sb_txt = match g(&raw, "secureBoot").as_bool() {
        Some(true) => "Enabled",
        Some(false) => "Disabled",
        None => "Unknown",
    };
    let sensors: Vec<Value> = as_list(g(&raw, "thermal"))
        .iter()
        .enumerate()
        .map(|(i, t)| json!({"name": format!("ACPI thermal zone {}", i + 1), "value": t, "unit": "°C"}))
        .collect();

    json!({
        "demo": false,
        "device": {"name": s(g(cs, "name")), "maker": s(g(cs, "maker")), "model": s(g(cs, "model")), "serial": s(g(bios, "serial")), "domain": s(g(cs, "domain"))},
        "os": {"name": s(g(os, "name")).replace("Microsoft ", ""), "version": s(g(os, "display")), "build": s(g(os, "build")),
               "arch": s(g(os, "arch")), "install_date": s(g(os, "installed"))},
        "cpu": {"name": squash(&s(g(cpu, "name"))), "cores": g(cpu, "cores"), "threads": g(cpu, "threads"),
                "max_ghz": round(f(g(cpu, "mhz")) / 1000.0, 2), "socket": s(g(cpu, "socket")), "l3_mb": (f(g(cpu, "l3")) / 1024.0).round()},
        "gpu": gpus,
        "board": {"maker": s(g(board, "maker")), "model": s(g(board, "model")), "bios_version": s(g(bios, "version")), "bios_date": s(g(bios, "date"))},
        "tpm": tpm_txt, "secure_boot": sb_txt,
        "memory": {"total_gb": gb(g(cs, "ram"), 0), "slots_total": g(&raw, "slots"), "modules": mods},
        "sensors": sensors,
    })
}

// ----------------------------------------------------------------- storage
const ST_PS: &str = r#"
$disks = @(Get-PhysicalDisk | ForEach-Object { $d=$_; $rc=$d | Get-StorageReliabilityCounter; [pscustomobject]@{ id=[string]$d.DeviceId; name=$d.FriendlyName; media=[string]$d.MediaType; bus=[string]$d.BusType; size=$d.Size; health=[string]$d.HealthStatus; serial=$d.SerialNumber; temp=$rc.Temperature; hours=$rc.PowerOnHours; wear=$rc.Wear; rerr=$rc.ReadErrorsTotal; werr=$rc.WriteErrorsTotal } })
$vols = @(Get-Volume | Where-Object { $_.DriveLetter } | ForEach-Object { [pscustomobject]@{ letter=[string]$_.DriveLetter; label=$_.FileSystemLabel; fs=$_.FileSystem; size=$_.Size; free=$_.SizeRemaining; type=[string]$_.DriveType } })
$map = @(Get-Partition | Where-Object { $_.DriveLetter } | ForEach-Object { [pscustomobject]@{ letter=[string]$_.DriveLetter; disk=[string]$_.DiskNumber } })
$bl = @(try { Get-BitLockerVolume | ForEach-Object { [pscustomobject]@{ letter=([string]$_.MountPoint).TrimEnd(':'); on=([string]$_.ProtectionStatus) } } } catch { })
[pscustomobject]@{ disks=$disks; vols=$vols; map=$map; bl=$bl } | ConvertTo-Json -Depth 4 -Compress
"#;

fn temp_size_mb(limit: Duration) -> i64 {
    let root = std::env::var("TEMP").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let start = Instant::now();
    let (mut total, mut stack) = (0u64, vec![root]);
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                match e.metadata() {
                    Ok(m) if m.is_dir() => stack.push(e.path()),
                    Ok(m) => total += m.len(),
                    Err(_) => {}
                }
            }
        }
        if start.elapsed() > limit {
            break;
        }
    }
    (total as f64 / 1_048_576.0).round() as i64
}

pub fn storage() -> Value {
    if !IS_WIN {
        return demo::storage();
    }
    let Some(raw) = ps_json(ST_PS, 45) else { return err("Could not read storage information.") };
    let letter_disk: Vec<(String, String)> =
        as_list(g(&raw, "map")).iter().map(|m| (s(g(m, "letter")), s(g(m, "disk")))).collect();
    let bl: Vec<(String, &str)> = as_list(g(&raw, "bl"))
        .iter()
        .map(|b| {
            let on = s(g(b, "on"));
            (s(g(b, "letter")), if on == "On" || on == "1" { "on" } else { "off" })
        })
        .collect();

    let disks: Vec<Value> = as_list(g(&raw, "disks"))
        .iter()
        .map(|d| {
            let health_txt = s(g(d, "health"));
            let mut health = match health_txt.as_str() {
                "Healthy" => "ok",
                "Warning" => "warn",
                "Unhealthy" => "bad",
                _ => "unknown",
            };
            if health == "ok" && f(g(d, "rerr")) > 0.0 {
                health = "warn";
            }
            let media = s(g(d, "media"));
            let mut kind = match media.as_str() {
                "SSD" => "SSD".to_string(),
                "HDD" => "HDD".to_string(),
                "" => "Disk".to_string(),
                m => m.to_string(),
            };
            if s(g(d, "bus")) == "NVMe" {
                kind = "NVMe SSD".into();
            }
            let id = s(g(d, "id"));
            let vols: Vec<&String> = letter_disk.iter().filter(|(_, n)| *n == id).map(|(l, _)| l).collect();
            json!({"id": id, "name": s(g(d, "name")), "type": kind, "size_gb": gb(g(d, "size"), 0), "health": health,
                   "status": if health_txt.is_empty() { "Unknown".to_string() } else { health_txt },
                   "serial": s(g(d, "serial")), "temp_c": g(d, "temp"), "power_on_hours": g(d, "hours"), "wear_pct": g(d, "wear"),
                   "read_errors": g(d, "rerr"), "write_errors": g(d, "werr"), "volumes": vols})
        })
        .collect();

    let vols: Vec<Value> = as_list(g(&raw, "vols"))
        .iter()
        .map(|v| {
            let letter = s(g(v, "letter"));
            let disk = letter_disk.iter().find(|(l, _)| *l == letter).map(|(_, d)| d.clone());
            let enc = bl.iter().find(|(l, _)| *l == letter).map(|(_, e)| *e).unwrap_or("unknown");
            json!({"letter": letter, "label": s(g(v, "label")), "fs": s(g(v, "fs")), "size_gb": gb(g(v, "size"), 1),
                   "free_gb": gb(g(v, "free"), 1), "disk": disk, "encryption": enc})
        })
        .collect();

    json!({"demo": false, "disks": disks, "volumes": vols, "temp_mb": temp_size_mb(Duration::from_secs(4))})
}

// ----------------------------------------------------------------- network
const NET_PS: &str = r#"
$cfg = @(Get-NetIPConfiguration | Where-Object { $_.NetAdapter.Status -eq 'Up' } | ForEach-Object { [pscustomobject]@{ name=$_.InterfaceAlias; desc=$_.InterfaceDescription; ipv4=@($_.IPv4Address.IPAddress); prefix=$_.IPv4Address.PrefixLength; ipv6=@($_.IPv6Address.IPAddress); gateway=@($_.IPv4DefaultGateway.NextHop); dns=@($_.DnsServer.ServerAddresses); dhcp=[string](Get-NetIPInterface -InterfaceAlias $_.InterfaceAlias -AddressFamily IPv4).Dhcp } })
$ad = @(Get-NetAdapter | ForEach-Object { [pscustomobject]@{ name=$_.Name; desc=$_.InterfaceDescription; status=[string]$_.Status; mac=$_.MacAddress; speed=$_.LinkSpeed; media=[string]$_.PhysicalMediaType; virtual=[bool]$_.Virtual } })
[pscustomobject]@{ cfg=$cfg; ad=$ad } | ConvertTo-Json -Depth 4 -Compress
"#;

/// (average ms, loss %) from the system `ping`, parsed without depending on the Windows language.
fn ping(host: &str, count: u32) -> (Option<f64>, i64) {
    let mut cmd = Command::new("ping");
    if IS_WIN {
        cmd.args(["-n", &count.to_string(), "-w", "1000", host]);
    } else {
        cmd.args(["-c", &count.to_string(), "-W", "1", host]);
    }
    let Some(out) = run_cmd(&mut cmd, Duration::from_secs(count as u64 * 2 + 4)) else { return (None, 100) };
    let text = String::from_utf8_lossy(&out);
    let mut times = vec![];
    for line in text.lines() {
        if line.to_lowercase().contains("ttl=") {
            if let Some(ms) = ms_in(line) {
                times.push(ms);
            }
        }
    }
    let loss = ((count as i64 - times.len() as i64) * 100) / count as i64;
    let avg = if times.is_empty() { None } else { Some((times.iter().sum::<f64>() / times.len() as f64).round()) };
    (avg, loss)
}

/// First number directly followed by "ms" ("time=14ms", "time<1ms", "time=0.5 ms").
fn ms_in(line: &str) -> Option<f64> {
    let l = line.to_lowercase();
    let at = l.find("ms")?;
    let head = l[..at].trim_end();
    let start = head.rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map(|i| i + 1).unwrap_or(0);
    head[start..].parse().ok()
}

fn wifi() -> Value {
    if !IS_WIN {
        return Value::Null;
    }
    let mut cmd = Command::new("netsh");
    cmd.args(["wlan", "show", "interfaces"]);
    let Some(out) = run_cmd(&mut cmd, Duration::from_secs(8)) else { return Value::Null };
    let text = String::from_utf8_lossy(&out).to_string();
    let get = |key: &str| -> String {
        text.lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(key))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    };
    let ssid = get("ssid");
    if ssid.is_empty() {
        return Value::Null;
    }
    let sig: String = get("signal").chars().filter(|c| c.is_ascii_digit()).collect();
    json!({"ssid": ssid, "signal": sig.parse::<i64>().ok(), "radio": get("radio type"), "channel": get("channel"),
           "security": get("authentication"), "rx": get("receive rate (mbps)"), "tx": get("transmit rate (mbps)")})
}

fn first_str(v: &Value) -> String {
    as_list(v).first().map(s).unwrap_or_default()
}

pub fn network() -> Value {
    if !IS_WIN {
        return demo::network();
    }
    let raw = ps_json(NET_PS, 30).unwrap_or(Value::Null);
    let cfgs = as_list(g(&raw, "cfg"));
    let adapters_raw = as_list(g(&raw, "ad"));
    let cfg = cfgs.iter().find(|c| !as_list(g(c, "gateway")).is_empty()).or(cfgs.first());

    let (mut active, mut wifi_info) = (Value::Null, Value::Null);
    if let Some(cfg) = cfg {
        let ad = adapters_raw.iter().find(|a| g(a, "name") == g(cfg, "name")).cloned().unwrap_or(Value::Null);
        let desc = s(g(cfg, "desc")).to_lowercase();
        let is_wifi = s(g(&ad, "media")).contains("802.11") || desc.contains("wi-fi") || desc.contains("wireless");
        let prefix = g(cfg, "prefix").as_u64().filter(|p| (1..=32).contains(p)).unwrap_or(24) as u32;
        let m = u32::MAX << (32 - prefix);
        let mask = format!("{}.{}.{}.{}", m >> 24, (m >> 16) & 255, (m >> 8) & 255, m & 255);
        let v6: Vec<String> = as_list(g(cfg, "ipv6")).iter().map(s).filter(|x| !x.is_empty()).collect();
        active = json!({"name": s(g(cfg, "name")), "type": if is_wifi { "Wi-Fi" } else { "Ethernet" }, "desc": s(g(cfg, "desc")),
            "mac": s(g(&ad, "mac")), "ipv4": first_str(g(cfg, "ipv4")), "mask": mask, "gateway": first_str(g(cfg, "gateway")),
            "dns": as_list(g(cfg, "dns")).iter().map(s).filter(|x| !x.is_empty()).collect::<Vec<_>>(),
            "ipv6": v6.first().cloned().unwrap_or_default(), "dhcp": s(g(cfg, "dhcp")), "speed": s(g(&ad, "speed"))});
        if is_wifi {
            wifi_info = wifi();
        }
    }

    let mut targets: Vec<(String, String)> = vec![];
    let gw = s(g(&active, "gateway"));
    if !gw.is_empty() {
        targets.push(("Router".into(), gw));
    }
    targets.push(("Google DNS".into(), "8.8.8.8".into()));
    targets.push(("Cloudflare".into(), "1.1.1.1".into()));
    let pings: Vec<Value> = std::thread::scope(|sc| {
        let hs: Vec<_> = targets.iter().map(|(_, h)| sc.spawn(move || ping(h, 4))).collect();
        hs.into_iter()
            .zip(&targets)
            .map(|(h, (label, host))| {
                let (ms, loss) = h.join().unwrap_or((None, 100));
                json!({"label": label, "target": host, "ms": ms, "loss": loss})
            })
            .collect()
    });

    let t = Instant::now();
    let dns_ms = {
        use std::net::ToSocketAddrs;
        if "www.google.com:443".to_socket_addrs().is_ok() { json!(t.elapsed().as_millis() as u64) } else { Value::Null }
    };
    let adapters: Vec<Value> = adapters_raw
        .iter()
        .map(|a| json!({"name": s(g(a, "name")), "desc": s(g(a, "desc")), "status": s(g(a, "status")), "speed": s(g(a, "speed")),
                        "virtual": g(a, "virtual").as_bool().unwrap_or(false)}))
        .collect();
    json!({"demo": false, "hostname": System::host_name().unwrap_or_default(), "active": active, "wifi": wifi_info,
           "adapters": adapters, "ping": pings, "dns_ms": dns_ms})
}

pub fn public_ip() -> Value {
    match ureq::get("https://api.ipify.org").timeout(Duration::from_secs(6)).call().and_then(|r| r.into_string().map_err(Into::into)) {
        Ok(ip) => json!({"ok": true, "ip": ip.trim()}),
        Err(_) => json!({"ok": false, "error": "Could not reach the internet."}),
    }
}

// ---------------------------------------------------------------- security
const SEC_PS: &str = r#"
$sys = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$mp = Get-MpComputerStatus
$smb = try { (Get-SmbServerConfiguration).EnableSMB1Protocol } catch { $null }
$rdp = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server').fDenyTSConnections
$admins = @(try { Get-LocalGroupMember -SID S-1-5-32-544 | ForEach-Object { [string]$_.Name } } catch { })
$fw = @(Get-NetFirewallProfile | ForEach-Object { [pscustomobject]@{ name=[string]$_.Name; on=[bool]$_.Enabled } })
$ports = @(Get-NetTCPConnection -State Listen | ForEach-Object { [pscustomobject]@{ port=$_.LocalPort; addr=$_.LocalAddress; pid=$_.OwningProcess } })
$reboot = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
[pscustomobject]@{
  uac=$sys.EnableLUA; defender=$(if($mp){[pscustomobject]@{ rtp=$mp.RealTimeProtectionEnabled; av=$mp.AntivirusEnabled; age=$mp.AntivirusSignatureAge }}else{$null})
  smb1=$smb; rdp=$rdp; admins=$admins; fw=$fw; ports=$ports; reboot=$reboot
} | ConvertTo-Json -Depth 4 -Compress
"#;

fn risky_port(p: i64) -> bool {
    matches!(p, 21 | 23 | 445 | 3389 | 5985 | 5900)
}

const NEEDS_ADMIN: &str = "Run DeepScan as administrator to check this.";

fn chk(id: &str, group: &str, name: &str, detail: impl Into<String>, status: &str, advice: &str) -> Value {
    json!({"id": id, "group": group, "name": name, "detail": detail.into(), "status": status, "advice": advice})
}

fn score(checks: &[Value]) -> i64 {
    let count = |st: &str| checks.iter().filter(|c| c["status"] == st).count() as i64;
    (100 - 15 * count("bad") - 6 * count("warn")).max(0)
}

pub fn security() -> Value {
    if !IS_WIN {
        return demo::security();
    }
    let Some(raw) = ps_json(SEC_PS, 45) else { return err("Could not read security settings.") };
    let hw = HW_CACHE.lock().ok().and_then(|x| x.clone()).unwrap_or(Value::Null);
    let st = ST_CACHE.lock().ok().and_then(|x| x.clone()).unwrap_or(Value::Null);
    let (p, a, n) = ("Protection", "Accounts and access", "Network and updates");
    let mut checks: Vec<Value> = vec![];

    let d = g(&raw, "defender");
    if d.is_null() {
        checks.push(chk("defender", p, "Antivirus", "Microsoft Defender is not reporting (another antivirus may be in charge)", "unknown", ""));
    } else if g(d, "rtp").as_bool() != Some(true) {
        checks.push(chk("defender", p, "Microsoft Defender", "Real-time protection is OFF", "bad", "Turn it on in Windows Security > Virus & threat protection."));
    } else if f(g(d, "age")) > 7.0 {
        checks.push(chk("defender", p, "Microsoft Defender", format!("Definitions are {} days old", f(g(d, "age")) as i64), "warn", "Open Windows Security and check for updates."));
    } else {
        checks.push(chk("defender", p, "Microsoft Defender", format!("Real-time protection is on, definitions are {} days old", f(g(d, "age")) as i64), "ok", ""));
    }

    let fw = as_list(g(&raw, "fw"));
    let off: Vec<String> = fw.iter().filter(|x| g(x, "on").as_bool() != Some(true)).map(|x| s(g(x, "name"))).collect();
    if fw.is_empty() {
        checks.push(chk("firewall", p, "Windows Firewall", "Could not read firewall state", "unknown", ""));
    } else if off.len() == fw.len() {
        checks.push(chk("firewall", p, "Windows Firewall", "All profiles are off", "bad", "Turn the firewall on in Windows Security > Firewall."));
    } else if !off.is_empty() {
        checks.push(chk("firewall", p, "Windows Firewall", format!("Off for: {}", off.join(", ")), "warn", "Turn the firewall on for every profile."));
    } else {
        checks.push(chk("firewall", p, "Windows Firewall", "All profiles are on", "ok", ""));
    }

    let sb = s(g(&hw, "secure_boot"));
    checks.push(chk("secureboot", p, "Secure Boot", if sb.is_empty() { "Unknown".to_string() } else { sb.clone() },
        match sb.as_str() { "Enabled" => "ok", "Disabled" => "warn", _ => "unknown" },
        if sb == "Disabled" { "Enable Secure Boot in the BIOS/UEFI settings." } else { "" }));
    let tpm = { let t = s(g(&hw, "tpm")); if t.is_empty() { "Unknown".to_string() } else { t } };
    checks.push(chk("tpm", p, "TPM", tpm.clone(),
        if tpm.starts_with("Present") { "ok" } else if tpm == "Not found" { "warn" } else { "unknown" }, ""));

    let vols: Vec<Value> = as_list(g(&st, "volumes")).into_iter().filter(|v| !g(v, "disk").is_null()).collect();
    if vols.is_empty() || vols.iter().all(|v| g(v, "encryption") == "unknown") {
        checks.push(chk("bitlocker", p, "BitLocker", if is_admin() { "Not available on this edition" } else { NEEDS_ADMIN }, "unknown", ""));
    } else {
        let unprot: Vec<String> = vols.iter().filter(|v| g(v, "encryption") == "off").map(|v| s(g(v, "letter"))).collect();
        let detail = vols.iter()
            .map(|v| format!("{}: {}", s(g(v, "letter")), if g(v, "encryption") == "on" { "protected" } else { "not protected" }))
            .collect::<Vec<_>>().join(" · ");
        let advice = if unprot.is_empty() { String::new() } else { format!("Turn on BitLocker for {}", unprot.iter().map(|x| format!("{x}:")).collect::<Vec<_>>().join(", ")) };
        checks.push(chk("bitlocker", p, "BitLocker", detail, if unprot.is_empty() { "ok" } else { "warn" }, &advice));
    }

    let uac = g(&raw, "uac").as_i64() == Some(1);
    checks.push(chk("uac", a, "User Account Control", if uac { "Enabled" } else { "Disabled" }, if uac { "ok" } else { "bad" },
        if uac { "" } else { "Turn UAC on in Control Panel > User Accounts." }));
    let admins = as_list(g(&raw, "admins")).len();
    checks.push(chk("admins", a, "Local administrators", format!("{admins} accounts in the Administrators group"),
        if admins <= 2 { "ok" } else { "warn" }, if admins <= 2 { "" } else { "Remove accounts that do not need admin rights." }));
    let rdp_off = g(&raw, "rdp").as_i64() == Some(1);
    checks.push(chk("rdp", a, "Remote Desktop (RDP)", if rdp_off { "Disabled" } else { "Enabled" }, if rdp_off { "ok" } else { "warn" },
        if rdp_off { "" } else { "Turn it off in Settings > System > Remote Desktop if you do not use it." }));

    match g(&raw, "smb1").as_bool() {
        None => checks.push(chk("smb1", n, "SMBv1 protocol", NEEDS_ADMIN, "unknown", "")),
        Some(on) => checks.push(chk("smb1", n, "SMBv1 protocol", if on { "Old and insecure, still turned on" } else { "Turned off" },
            if on { "bad" } else { "ok" },
            if on { "In PowerShell as admin: Disable-WindowsOptionalFeature -Online -FeatureName SMB1Protocol" } else { "" })),
    }

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let (mut ports, mut seen): (Vec<(bool, i64, Value)>, Vec<i64>) = (vec![], vec![]);
    for pr in as_list(g(&raw, "ports")) {
        let port = g(&pr, "port").as_i64().unwrap_or(0);
        if seen.contains(&port) {
            continue;
        }
        seen.push(port);
        let pid = g(&pr, "pid").as_u64().unwrap_or(0) as u32;
        let name = sys.process(sysinfo::Pid::from_u32(pid)).map(|x| x.name().to_string_lossy().to_string()).unwrap_or_else(|| "System".into());
        let addr = s(g(&pr, "addr"));
        let local = addr == "127.0.0.1" || addr == "::1";
        let review = risky_port(port) && !local;
        ports.push((review, port, json!({"port": port, "process": name, "scope": if local { "This PC only" } else { "Local network" },
                                         "flag": if review { "review" } else { "ok" }})));
    }
    ports.sort_by_key(|(review, port, _)| (!*review, *port));
    let risky: Vec<String> = ports.iter().filter(|(r, _, _)| *r).map(|(_, p, _)| p.to_string()).collect();
    checks.push(chk("ports", n, "Listening ports",
        if risky.is_empty() { format!("{} ports open, none risky", ports.len()) } else { format!("{} port(s) worth reviewing ({})", risky.len(), risky.join(", ")) },
        if risky.is_empty() { "ok" } else { "warn" },
        if risky.is_empty() { "" } else { "Close ports you do not need or block them in the firewall." }));
    let reboot = g(&raw, "reboot").as_bool() == Some(true);
    checks.push(chk("reboot", n, "Pending restart", if reboot { "A restart is waiting" } else { "None" }, if reboot { "warn" } else { "ok" },
        if reboot { "Restart the PC to finish installing updates." } else { "" }));
    checks.push(chk("updates", n, "Windows updates", "Checking...", "unknown", ""));

    let sc = score(&checks);
    let port_rows: Vec<Value> = ports.into_iter().take(30).map(|(_, _, v)| v).collect();
    json!({"demo": false, "score": sc, "checks": checks, "ports": port_rows})
}

pub fn updates() -> Value {
    if !IS_WIN {
        return demo::updates();
    }
    let script = r#"
$s = New-Object -ComObject Microsoft.Update.Session
$r = $s.CreateUpdateSearcher().Search('IsInstalled=0 and IsHidden=0')
$sec = 0
foreach ($u in $r.Updates) { foreach ($c in $u.Categories) { if ($c.Name -eq 'Security Updates') { $sec++; break } } }
[pscustomobject]@{ count=$r.Updates.Count; security=$sec } | ConvertTo-Json -Compress
"#;
    match ps_json(script, 90) {
        Some(raw) => json!({"ok": true, "count": g(&raw, "count"), "security": g(&raw, "security")}),
        None => json!({"ok": false}),
    }
}

// ---------------------------------------------------------------- software
#[cfg(windows)]
fn programs() -> Vec<Value> {
    use winreg::enums::*;
    use winreg::RegKey;
    let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let roots = [(HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY), (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY), (HKEY_CURRENT_USER, 0)];
    let (mut out, mut seen): (Vec<Value>, Vec<(String, String)>) = (vec![], vec![]);
    for (hive, flag) in roots {
        let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ | flag) else { continue };
        for name in key.enum_keys().flatten() {
            let Ok(sub) = key.open_subkey(&name) else { continue };
            let Ok(display) = sub.get_value::<String, _>("DisplayName") else { continue };
            if display.trim().is_empty()
                || sub.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1
                || sub.get_value::<String, _>("ParentKeyName").is_ok()
            {
                continue;
            }
            let version = sub.get_value::<String, _>("DisplayVersion").unwrap_or_default();
            let ident = (display.clone(), version.clone());
            if seen.contains(&ident) {
                continue;
            }
            seen.push(ident);
            let d = sub.get_value::<String, _>("InstallDate").unwrap_or_default();
            let date = if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) { format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..8]) } else { String::new() };
            let size = sub.get_value::<u32, _>("EstimatedSize").ok().filter(|x| *x > 0).map(|x| (x as f64 / 1024.0).round() as i64);
            out.push(json!({"name": display.trim(), "publisher": sub.get_value::<String, _>("Publisher").unwrap_or_default().trim(),
                            "version": version.trim(), "size_mb": size, "installed": date}));
        }
    }
    out.sort_by_key(|p| p["name"].as_str().unwrap_or("").to_lowercase());
    out
}
#[cfg(not(windows))]
fn programs() -> Vec<Value> { vec![] }

#[cfg(windows)]
fn startup() -> Vec<Value> {
    use winreg::enums::*;
    use winreg::RegKey;
    let mut items = vec![];
    let spots = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run", "Registry (user)", "Run"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "Registry (machine)", "Run"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "Registry (machine, 32-bit)", "Run32"),
    ];
    for (hive, path, wh, approved) in spots {
        let Ok(key) = RegKey::predef(hive).open_subkey(path) else { continue };
        for (name, _) in key.enum_values().flatten() {
            let cmd: String = key.get_value(&name).unwrap_or_default();
            let mut enabled = true;
            if let Ok(ak) = RegKey::predef(hive).open_subkey(format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{approved}")) {
                if let Ok(v) = ak.get_raw_value(&name) {
                    if let Some(b) = v.bytes.first() {
                        enabled = b % 2 == 0;
                    }
                }
            }
            items.push(json!({"name": name, "command": cmd.chars().take(160).collect::<String>(), "location": wh, "enabled": enabled}));
        }
    }
    let folders = [
        (std::env::var("APPDATA").unwrap_or_default() + r"\Microsoft\Windows\Start Menu\Programs\Startup", "Startup folder (user)"),
        (std::env::var("PROGRAMDATA").unwrap_or_default() + r"\Microsoft\Windows\Start Menu\Programs\Startup", "Startup folder (all users)"),
    ];
    for (folder, wh) in folders {
        if let Ok(rd) = std::fs::read_dir(&folder) {
            for e in rd.flatten() {
                let fname = e.file_name().to_string_lossy().to_string();
                if fname.eq_ignore_ascii_case("desktop.ini") {
                    continue;
                }
                let stem = std::path::Path::new(&fname).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or(fname.clone());
                items.push(json!({"name": stem, "command": e.path().to_string_lossy(), "location": wh, "enabled": true}));
            }
        }
    }
    items
}
#[cfg(not(windows))]
fn startup() -> Vec<Value> { vec![] }

fn processes(top: usize) -> Vec<Value> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    std::thread::sleep(Duration::from_millis(600));
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let mut agg: Vec<(String, f64, f64)> = vec![];
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_string();
        let (cpu, mem) = (p.cpu_usage() as f64 / cores, p.memory() as f64 / 1_048_576.0);
        match agg.iter_mut().find(|e| e.0 == name) {
            Some(e) => { e.1 += cpu; e.2 += mem; }
            None => agg.push((name, cpu, mem)),
        }
    }
    agg.sort_by(|a, b| (b.1, b.2).partial_cmp(&(a.1, a.2)).unwrap_or(std::cmp::Ordering::Equal));
    agg.into_iter().take(top).map(|(n, c, m)| json!({"name": n, "cpu": round(c, 1), "mem_mb": m.round()})).collect()
}

pub fn software() -> Value {
    if !IS_WIN {
        return demo::software();
    }
    json!({"demo": false, "programs": programs(), "startup": startup(), "processes": processes(8)})
}

// -------------------------------------------------------------------- live
pub struct LiveState {
    sys: System,
    nets: Networks,
    prev: Option<(Instant, u64, u64)>,
    ticks: u32,
}

impl LiveState {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        Self { sys, nets: Networks::new_with_refreshed_list(), prev: None, ticks: 0 }
    }
}

impl Default for LiveState {
    fn default() -> Self { Self::new() }
}

pub fn live(st: &mut LiveState) -> Value {
    st.sys.refresh_cpu_usage();
    st.sys.refresh_memory();
    // Re-list interfaces only now and then (adapters rarely change); other ticks just read the counters.
    st.ticks += 1;
    st.nets.refresh(st.ticks % 30 == 0);
    let (rx, tx) = st.nets.iter().fold((0u64, 0u64), |a, (_, n)| (a.0 + n.total_received(), a.1 + n.total_transmitted()));
    let now = Instant::now();
    let (mut down, mut up) = (0.0, 0.0);
    if let Some((t, prx, ptx)) = st.prev {
        let dt = now.duration_since(t).as_secs_f64();
        if dt > 0.0 {
            down = rx.saturating_sub(prx) as f64 * 8.0 / dt / 1e6;
            up = tx.saturating_sub(ptx) as f64 * 8.0 / dt / 1e6;
        }
    }
    st.prev = Some((now, rx, tx));
    let (used, total) = (st.sys.used_memory() as f64, st.sys.total_memory() as f64);
    let uptime = System::uptime();
    json!({"cpu": st.sys.global_cpu_usage().round(), "mem": if total > 0.0 { (used / total * 100.0).round() } else { 0.0 },
           "mem_used_gb": round(used / 1_073_741_824.0, 1), "mem_total_gb": round(total / 1_073_741_824.0, 1),
           "down_mbps": round(down, 1), "up_mbps": round(up, 1), "uptime_s": uptime})
}
#[cfg(test)]
pub fn __ms(l: &str) -> Option<f64> { ms_in(l) }
