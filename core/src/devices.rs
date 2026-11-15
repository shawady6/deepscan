//! Battery health, connected devices (USB, Bluetooth, input, cameras...), and USB flash drives: detect, check, and (after strong confirmation) format.
//! Everything runs only when the user presses a button. Formatting is limited to removable USB disks;
//! the system, boot and non-USB disks are refused in Rust, whatever the UI sends.
use crate::store;
use crate::util::*;
use serde_json::{json, Value};

// ===================================================================== battery
const BATTERY: &str = r##"
$st = Get-CimInstance -Namespace root\wmi -ClassName BatteryStaticData -ErrorAction SilentlyContinue | Select-Object -First 1
$fc = Get-CimInstance -Namespace root\wmi -ClassName BatteryFullChargedCapacity -ErrorAction SilentlyContinue | Select-Object -First 1
$cy = Get-CimInstance -Namespace root\wmi -ClassName BatteryCycleCount -ErrorAction SilentlyContinue | Select-Object -First 1
$b = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $b -and -not $st) { '{"present":false}'; return }
[pscustomobject]@{
  present = $true
  design = if ($st) { [int]$st.DesignedCapacity } else { 0 }
  full = if ($fc) { [int]$fc.FullChargedCapacity } else { 0 }
  cycles = if ($cy) { [int]$cy.CycleCount } else { 0 }
  charge = if ($b) { [int]$b.EstimatedChargeRemaining } else { -1 }
  status = if ($b) { [int]$b.BatteryStatus } else { 0 }
  name = if ($b) { [string]$b.Name } else { '' }
  chem = if ($st) { [string]$st.Chemistry } else { '' }
} | ConvertTo-Json -Compress
"##;

pub fn battery() -> Value {
    if !IS_WIN {
        return json!({"present": true, "demo": true, "design_mwh": 50000, "full_mwh": 41500, "health": 83, "cycles": 412, "charge": 76, "charging": false,
            "grade": "ok", "verdict": "Normal wear for its age", "name": "Demo battery"});
    }
    let Some(v) = ps_json(BATTERY, 40) else { return json!({"ok": false, "error": "Could not read the battery."}) };
    if v["present"] != true {
        return json!({"present": false});
    }
    let design = f(g(&v, "design"));
    let full = f(g(&v, "full"));
    let health = if design > 0.0 && full > 0.0 { (full / design * 100.0).round().min(100.0) } else { 0.0 };
    let (grade, verdict) = match health as i64 {
        0 => ("unknown", "Windows did not report the capacity numbers for this battery"),
        85..=100 => ("ok", "Healthy"),
        70..=84 => ("ok", "Normal wear"),
        50..=69 => ("warn", "Worn. Battery life is clearly shorter than new"),
        _ => ("bad", "Badly worn. Replacement recommended"),
    };
    let status = f(g(&v, "status")) as i64;
    json!({"present": true, "design_mwh": design, "full_mwh": full, "health": health, "cycles": f(g(&v, "cycles")),
        "charge": f(g(&v, "charge")), "charging": matches!(status, 2 | 6 | 7 | 8), "grade": grade, "verdict": verdict, "name": s(g(&v, "name"))})
}

// ===================================================================== connected devices
const PERIPHERALS: &str = r##"
$bad = 'Root Hub|Host Controller|Composite Device|Generic USB Hub|USB Hub|Enumerator|Radio|Bluetooth Adapter|Wireless Bluetooth|Generic Bluetooth|Intel\(R\) Wireless'
$classes = 'Mouse','Keyboard','Camera','Image','WPD','Monitor','Printer','MEDIA','USB','Bluetooth','HIDClass','Biometric','SmartCardReader','Ports'
$rows = @()
foreach ($d in (Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue | Where-Object { $classes -contains $_.Class -or $_.InstanceId -like 'USBPRINT*' })) {
  $id = [string]$d.InstanceId; $name = [string]$d.FriendlyName; if (-not $name) { continue }
  $tr = if ($id -like 'USB*') { 'USB' } elseif ($id -like 'BTH*') { 'Bluetooth' } else { 'Built-in' }
  if ($d.Class -eq 'Bluetooth' -and $id -notmatch '^BTH(ENUM|LE)\\DEV_') { continue }
  if ($d.Class -in 'USB','HIDClass','Ports','MEDIA' -and $tr -ne 'USB' -and $tr -ne 'Bluetooth') { continue }
  if ($name -match $bad) { continue }
  $conn = $null
  if ($tr -eq 'Bluetooth') { try { $conn = [bool](Get-PnpDeviceProperty -InstanceId $id -KeyName '{83DA6326-97A6-4088-9453-A1923F573B29} 15' -ErrorAction Stop).Data } catch { $conn = $null } }
  $rows += [pscustomobject]@{ name=$name; cls=[string]$d.Class; tr=$tr; status=[string]$d.Status; connected=$conn; id=$id }
}
ConvertTo-Json -Compress -Depth 3 -InputObject @($rows | Sort-Object name -Unique)
"##;

fn category(cls: &str, id: &str, tr: &str) -> &'static str {
    let c = cls.to_ascii_lowercase();
    if tr == "Bluetooth" && c == "bluetooth" {
        return "Bluetooth devices";
    }
    match c.as_str() {
        "mouse" => "Mice and pointing devices",
        "keyboard" => "Keyboards",
        "camera" | "image" => "Cameras and scanners",
        "monitor" => "Monitors",
        "printer" => "Printers",
        "wpd" => "Phones and media players",
        "media" => "Audio devices",
        "biometric" | "smartcardreader" => "Security devices",
        _ if id.starts_with("USBPRINT") => "Printers",
        _ => "Other USB devices",
    }
}

pub fn peripherals() -> Value {
    if !IS_WIN {
        return json!({"demo": true, "groups": [
            {"name": "Keyboards", "items": [{"name": "HID Keyboard Device", "transport": "USB", "ok": true}, {"name": "Standard PS/2 Keyboard", "transport": "Built-in", "ok": true}]},
            {"name": "Mice and pointing devices", "items": [{"name": "HID-compliant mouse", "transport": "USB", "ok": true}]},
            {"name": "Cameras and scanners", "items": [{"name": "Integrated Camera", "transport": "Built-in", "ok": true}]},
            {"name": "Bluetooth devices", "items": [{"name": "WH-1000XM4", "transport": "Bluetooth", "ok": true, "connected": true}, {"name": "Galaxy Buds", "transport": "Bluetooth", "ok": true, "connected": false}]},
            {"name": "Other USB devices", "items": [{"name": "USB Mass Storage Device", "transport": "USB", "ok": false, "problem": "Driver problem"}]}]});
    }
    let Some(v) = ps_json(PERIPHERALS, 60) else { return json!({"ok": false, "error": "Could not list devices.", "groups": []}) };
    let mut groups: Vec<(&str, Vec<Value>)> = Vec::new();
    for d in as_list(&v) {
        let (cls, id, tr) = (s(g(&d, "cls")), s(g(&d, "id")), s(g(&d, "tr")));
        let cat = category(&cls, &id, &tr);
        let status = s(g(&d, "status"));
        let ok = status.is_empty() || status.eq_ignore_ascii_case("ok") || status.eq_ignore_ascii_case("unknown");
        let mut item = json!({"name": s(g(&d, "name")), "transport": tr, "ok": ok});
        if !ok {
            item["problem"] = json!("Driver problem");
        }
        if let Some(b) = g(&d, "connected").as_bool() {
            item["connected"] = json!(b);
        }
        match groups.iter_mut().find(|x| x.0 == cat) {
            Some(x) => x.1.push(item),
            None => groups.push((cat, vec![item])),
        }
    }
    const ORDER: &[&str] = &["Keyboards", "Mice and pointing devices", "Bluetooth devices", "Cameras and scanners", "Audio devices", "Printers", "Monitors", "Phones and media players", "Security devices", "Other USB devices"];
    groups.sort_by_key(|x| ORDER.iter().position(|o| *o == x.0).unwrap_or(99));
    json!({"groups": groups.into_iter().map(|(n, items)| json!({"name": n, "items": items})).collect::<Vec<_>>()})
}

// ===================================================================== USB drives
const USB_LIST: &str = r##"
$out = @()
foreach ($d in (Get-Disk | Where-Object { $_.BusType -eq 'USB' })) {
  $vols = @(Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue | Get-Volume -ErrorAction SilentlyContinue | Where-Object { $_.DriveLetter } | ForEach-Object {
    [pscustomobject]@{ letter=[string]$_.DriveLetter; label=[string]$_.FileSystemLabel; fs=[string]$_.FileSystem; size=[double]$_.Size; free=[double]$_.SizeRemaining; health=[string]$_.HealthStatus } })
  $out += [pscustomobject]@{ disk=[int]$d.Number; name=[string]$d.FriendlyName; size=[double]$d.Size; health=[string]$d.HealthStatus; op=[string]$d.OperationalStatus -join ','; style=[string]$d.PartitionStyle; boot=[bool]($d.IsBoot -or $d.IsSystem); volumes=$vols }
}
ConvertTo-Json -Compress -Depth 4 -InputObject @($out)
"##;

pub fn usb_list() -> Value {
    if !IS_WIN {
        return json!({"drives": [{"disk": 2, "name": "Generic Flash Disk", "size_gb": 14.5, "health": "Healthy", "style": "MBR", "boot": false,
            "volumes": [{"letter": "E", "label": "USB", "fs": "FAT32", "size_gb": 14.5, "free_gb": 9.2}]}], "demo": true});
    }
    let Some(v) = ps_json(USB_LIST, 40) else { return json!({"ok": false, "error": "Could not list USB drives.", "drives": []}) };
    let drives: Vec<Value> = as_list(&v)
        .iter()
        .filter(|d| d["boot"] != true)
        .map(|d| {
            json!({"disk": d["disk"], "name": s(g(d, "name")), "size_gb": gb(g(d, "size"), 1), "health": s(g(d, "health")), "style": s(g(d, "style")),
                "boot": false,
                "volumes": as_list(&d["volumes"]).iter().map(|x| json!({"letter": s(g(x, "letter")), "label": s(g(x, "label")), "fs": s(g(x, "fs")),
                    "size_gb": gb(g(x, "size"), 1), "free_gb": gb(g(x, "free"), 1)})).collect::<Vec<_>>()})
        })
        .collect();
    json!({"drives": drives})
}

/// A single A-Z letter that currently belongs to a removable USB disk (not system or boot). Checked fresh on every call.
fn usb_letter_ok(letter: &str) -> Result<char, String> {
    let mut it = letter.chars();
    let (Some(c), None) = (it.next(), it.next()) else { return Err("Invalid drive letter".into()) };
    let c = c.to_ascii_uppercase();
    if !c.is_ascii_uppercase() {
        return Err("Invalid drive letter".into());
    }
    if !IS_WIN {
        return Ok(c);
    }
    let script = format!(
        "$p = Get-Partition -DriveLetter {c} -ErrorAction SilentlyContinue; if (-not $p) {{ 'none'; return }}; $d = Get-Disk -Number $p.DiskNumber; if ($d.BusType -eq 'USB' -and -not $d.IsBoot -and -not $d.IsSystem -and $c -ne $env:SystemDrive[0]) {{ 'ok' }} else {{ 'no' }}"
    );
    let (_, out) = ps_run(&script, 30);
    match out.trim() {
        "ok" => Ok(c),
        "none" => Err(format!("Drive {c}: was not found.")),
        _ => Err(format!("Drive {c}: is not a removable USB drive. DeepScan only works on USB drives.")),
    }
}

pub fn usb_script_check() -> &'static str {
    USB_CHECK
}

// Read-only file system scan, then a 64 MB write, read-back and compare on free space (detects bad blocks and write errors).
const USB_CHECK: &str = r##"
$L = '@@L@@'
$vol = Get-Volume -DriveLetter $L
"Drive ${L}: $($vol.FileSystem), $([math]::Round($vol.Size/1GB,1)) GB, health $($vol.HealthStatus)"
''
'== File system scan (read only) =='
try { Repair-Volume -DriveLetter $L -Scan | Out-String } catch { "Scan not available: $($_.Exception.Message)" }
''
'== Write and read-back test (64 MB) =='
if ($vol.SizeRemaining -lt 200MB) { 'Skipped: less than 200 MB free.'; return }
$f = "${L}:\deepscan_test.tmp"
$n = 64; $chunk = New-Object byte[] (1MB); $rnd = New-Object Random 42
$sha = [Security.Cryptography.SHA256]::Create()
$hashes = @()
try {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $fs = New-Object IO.FileStream($f, 'Create', 'Write', 'None', 1MB, 'WriteThrough')
  for ($i=0; $i -lt $n; $i++) { $rnd.NextBytes($chunk); $hashes += [BitConverter]::ToString($sha.ComputeHash($chunk)); $fs.Write($chunk, 0, $chunk.Length) }
  $fs.Flush($true); $fs.Dispose(); $w = $sw.Elapsed.TotalSeconds
  $sw.Restart()
  $fs = New-Object IO.FileStream($f, 'Open', 'Read', 'None', 1MB, ([IO.FileOptions]0x20000000))
  $bad = 0
  for ($i=0; $i -lt $n; $i++) { $r = $fs.Read($chunk, 0, $chunk.Length); if ($r -ne $chunk.Length -or [BitConverter]::ToString($sha.ComputeHash($chunk)) -ne $hashes[$i]) { $bad++ } }
  $fs.Dispose(); $r = $sw.Elapsed.TotalSeconds
  "Write speed: {0:N1} MB/s" -f ($n / [math]::Max($w, 0.001))
  "Read speed:  {0:N1} MB/s" -f ($n / [math]::Max($r, 0.001))
  if ($bad -eq 0) { 'Result: all data read back correctly. No errors found.' } else { "Result: $bad of $n blocks came back different. This drive is unreliable. Do not trust it with important files." }
} finally { if ($fs) { $fs.Dispose() }; Remove-Item $f -Force -ErrorAction SilentlyContinue }
"##;

pub fn usb_check(letter: &str) -> Value {
    let c = match usb_letter_ok(letter) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "output": e}),
    };
    if !IS_WIN {
        return json!({"ok": true, "output": format!("Demo: drive {c}: checked. Write 24.1 MB/s, read 31.8 MB/s. No errors found.")});
    }
    let (ok, out) = ps_run(&USB_CHECK.replace("@@L@@", &c.to_string()), 900);
    json!({"ok": ok, "output": out})
}

pub fn usb_format(letter: &str, fs: &str, label: &str, full: bool) -> Value {
    let c = match usb_letter_ok(letter) {
        Ok(c) => c,
        Err(e) => return json!({"ok": false, "output": e}),
    };
    let fs = match fs.to_ascii_lowercase().as_str() {
        "fat32" => "FAT32",
        "exfat" => "exFAT",
        "ntfs" => "NTFS",
        _ => return json!({"ok": false, "output": "Unsupported file system"}),
    };
    // Label: letters, digits, space, dash, underscore only. FAT32 labels are upper case and at most 11 characters.
    let mut label: String = label.chars().filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_')).take(32).collect();
    if fs == "FAT32" {
        label = label.to_ascii_uppercase().chars().take(11).collect();
    }
    if !IS_WIN {
        return json!({"ok": true, "output": format!("Demo: {c}: formatted as {fs}")});
    }
    let script = format!(
        "Format-Volume -DriveLetter {c} -FileSystem {fs} -NewFileSystemLabel '{label}' -Full:${} -Force -Confirm:$false | Select-Object DriveLetter, FileSystem, FileSystemLabel, Size | Format-List",
        if full { "true" } else { "false" }
    );
    let (ok, out) = ps_run(&script, if full { 7200 } else { 300 });
    store::add_history(json!({"title": format!("Formatted USB drive {c}: as {fs}"), "detail": if ok { "Completed" } else { "Failed" }, "undoable": false,
        "note": "Formatting cannot be undone."}));
    json!({"ok": ok, "output": out})
}

pub fn usb_open(letter: &str) -> Value {
    let Ok(c) = usb_letter_ok(letter) else { return json!({"ok": false}) };
    if !IS_WIN {
        return json!({"ok": true});
    }
    let (ok, _) = ps_run(&format!("Start-Process explorer.exe '{c}:\\'"), 20);
    json!({"ok": ok})
}
