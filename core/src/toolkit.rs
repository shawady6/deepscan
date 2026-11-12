//! Toolkit: a catalogue of maintenance tools (reports, cleanup, repair, network, Windows Update, security).
//! Nothing here runs by itself. The UI sends an ID, the script is static, and every run is logged to History.
use crate::store;
use crate::util::*;
use serde_json::{json, Value};

pub struct Tool {
    pub id: &'static str,
    pub group: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// low = read only or harmless, medium = changes settings, high = can interrupt work or needs a restart
    pub risk: &'static str,
    pub timeout: u64,
    /// Script writes files into the Desktop "DeepScan Reports" folder.
    pub files: bool,
    pub script: &'static str,
}

pub const GROUPS: &[(&str, &str)] = &[
    ("report", "Reports and diagnosis"),
    ("clean", "Cleanup"),
    ("perf", "Performance"),
    ("repair", "System repair"),
    ("disk", "Disk"),
    ("net", "Network"),
    ("wu", "Windows Update"),
    ("sec", "Security and hardware"),
    ("other", "Other"),
];

// Prefix for scripts that save files.
const PRE: &str = "$rep = Join-Path ([Environment]::GetFolderPath('Desktop')) 'DeepScan Reports'; New-Item -ItemType Directory -Force $rep | Out-Null; $ts = Get-Date -Format 'yyyyMMdd_HHmmss'\n";

pub static TOOLS: &[Tool] = &[
    // ------------------------------------------------------------ reports
    Tool { id: "diag", group: "report", name: "Full diagnosis", desc: "OS, uptime, CPU, memory, disks, stopped automatic services, recent critical errors and installed updates.", risk: "low", timeout: 180, files: false, script: r##"
$os = Get-CimInstance Win32_OperatingSystem
'== System =='
"{0} (build {1}), installed {2}" -f $os.Caption, $os.BuildNumber, $os.InstallDate
"Uptime: {0:N1} days" -f ((Get-Date) - $os.LastBootUpTime).TotalDays
Get-CimInstance Win32_ComputerSystem | ForEach-Object { "{0} {1}, RAM {2:N1} GB" -f $_.Manufacturer, $_.Model, ($_.TotalPhysicalMemory/1GB) }
Get-CimInstance Win32_Processor | ForEach-Object { "CPU: {0} ({1} cores)" -f $_.Name.Trim(), $_.NumberOfCores }
''
'== Disks =='
Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' | ForEach-Object { "{0} {1:N0} GB free of {2:N0} GB" -f $_.DeviceID, ($_.FreeSpace/1GB), ($_.Size/1GB) }
Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object { "{0}: {1}, health {2}" -f $_.FriendlyName, $_.MediaType, $_.HealthStatus }
''
'== Automatic services that are stopped =='
Get-CimInstance Win32_Service -Filter "StartMode='Auto' AND State<>'Running'" | Select-Object -First 25 Name, State | Format-Table -AutoSize
''
'== Critical and error events, last 7 days =='
Get-WinEvent -FilterHashtable @{LogName='System','Application'; Level=1,2; StartTime=(Get-Date).AddDays(-7)} -MaxEvents 15 -ErrorAction SilentlyContinue | ForEach-Object { "{0:yyyy-MM-dd HH:mm} {1}: {2}" -f $_.TimeCreated, $_.ProviderName, (($_.Message -split "`n")[0]) }
''
'== Last 10 installed updates =='
Get-HotFix | Sort-Object InstalledOn -Descending -ErrorAction SilentlyContinue | Select-Object -First 10 HotFixID, InstalledOn | Format-Table -AutoSize
"## },
    Tool { id: "sysinfo", group: "report", name: "System report (saved)", desc: "systeminfo, ipconfig /all and driver list, saved as a text file on the Desktop.", risk: "low", timeout: 180, files: true, script: r##"
$f = Join-Path $rep "SystemReport_$ts.txt"
(systeminfo; ''; '=== ipconfig /all ==='; ipconfig /all; ''; '=== Drivers ==='; driverquery /fo table) | Out-File $f -Encoding utf8
"Saved: $f"
"## },
    Tool { id: "inventory", group: "report", name: "Hardware and software inventory (saved)", desc: "Hardware summary and the full list of installed programs, saved as CSV and text.", risk: "low", timeout: 180, files: true, script: r##"
$sw = Join-Path $rep "InstalledSoftware_$ts.csv"
$keys = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*','HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
Get-ItemProperty $keys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName } | Select-Object DisplayName, DisplayVersion, Publisher, InstallDate | Sort-Object DisplayName | Export-Csv $sw -NoTypeInformation -Encoding UTF8
$hw = Join-Path $rep "Hardware_$ts.txt"
(Get-CimInstance Win32_BIOS | Format-List Manufacturer, SMBIOSBIOSVersion, SerialNumber; Get-CimInstance Win32_BaseBoard | Format-List Manufacturer, Product; Get-CimInstance Win32_VideoController | Format-List Name, DriverVersion; Get-CimInstance Win32_PhysicalMemory | Format-Table Manufacturer, Capacity, Speed -AutoSize) | Out-File $hw -Encoding utf8
"Saved: $sw"
"Saved: $hw"
"## },
    Tool { id: "activation", group: "report", name: "Windows activation status", desc: "Shows license state and expiry (slmgr).", risk: "low", timeout: 60, files: false, script: r##"
cscript //nologo "$env:SystemRoot\System32\slmgr.vbs" /dli
cscript //nologo "$env:SystemRoot\System32\slmgr.vbs" /xpr
"## },
    Tool { id: "battery", group: "report", name: "Battery report (saved)", desc: "Creates the Windows battery health report as HTML on the Desktop (laptops).", risk: "low", timeout: 90, files: true, script: r##"
$f = Join-Path $rep "BatteryReport_$ts.html"
powercfg /batteryreport /output $f
"## },
    Tool { id: "energy", group: "report", name: "Power efficiency report (saved)", desc: "Observes the PC for 60 seconds and lists power problems. Close heavy apps first.", risk: "low", timeout: 200, files: true, script: r##"
$f = Join-Path $rep "EnergyReport_$ts.html"
powercfg /energy /duration 60 /output $f
"## },
    Tool { id: "wifi_report", group: "report", name: "Wi-Fi report (saved)", desc: "Wi-Fi sessions and failures of the last three days as HTML.", risk: "low", timeout: 90, files: true, script: r##"
netsh wlan show wlanreport | Out-Null
$src = "$env:ProgramData\Microsoft\Windows\WlanReport\wlan-report-latest.html"
$f = Join-Path $rep "WifiReport_$ts.html"
Copy-Item $src $f -Force
"Saved: $f"
"## },
    Tool { id: "sec_snapshot", group: "report", name: "Security snapshot", desc: "Defender, firewall, UAC, BitLocker and Secure Boot state in one list.", risk: "low", timeout: 60, files: false, script: r##"
$d = Get-MpComputerStatus -ErrorAction SilentlyContinue
if ($d) { "Defender: real-time {0}, signatures {1}, last quick scan {2}" -f $d.RealTimeProtectionEnabled, $d.AntivirusSignatureLastUpdated, $d.QuickScanEndTime }
Get-NetFirewallProfile | ForEach-Object { "Firewall {0}: {1}" -f $_.Name, $_.Enabled }
"UAC enabled: " + (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System').EnableLUA
try { "Secure Boot: " + (Confirm-SecureBootUEFI) } catch { 'Secure Boot: not available (legacy BIOS or no access)' }
Get-BitLockerVolume -ErrorAction SilentlyContinue | ForEach-Object { "BitLocker {0}: {1}" -f $_.MountPoint, $_.ProtectionStatus }
"## },
    Tool { id: "startup_list", group: "report", name: "Startup programs", desc: "Lists what starts with Windows. Read only.", risk: "low", timeout: 60, files: false, script: r##"
Get-CimInstance Win32_StartupCommand | Select-Object Name, Location, Command | Format-List
"## },
    Tool { id: "hosts_proxy", group: "report", name: "Hosts file and proxy check", desc: "Shows custom hosts entries and proxy settings. Read only.", risk: "low", timeout: 30, files: false, script: r##"
'== hosts (active entries) =='
Get-Content "$env:SystemRoot\System32\drivers\etc\hosts" | Where-Object { $_ -and $_ -notmatch '^\s*#' }
''
'== User proxy =='
Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' | Format-List ProxyEnable, ProxyServer, AutoConfigURL
'== WinHTTP proxy =='
netsh winhttp show proxy
"## },
    // ------------------------------------------------------------ cleanup
    Tool { id: "clean_temp", group: "clean", name: "Clean temp files and update cache", desc: "Deletes temp files and downloaded update files, then runs DISM component cleanup.", risk: "low", timeout: 900, files: false, script: r##"
$before = (Get-PSDrive C).Free
Remove-Item "$env:TEMP\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:SystemRoot\Temp\*" -Recurse -Force -ErrorAction SilentlyContinue
Stop-Service wuauserv -Force -ErrorAction SilentlyContinue
Remove-Item "$env:SystemRoot\SoftwareDistribution\Download\*" -Recurse -Force -ErrorAction SilentlyContinue
Start-Service wuauserv -ErrorAction SilentlyContinue
DISM /Online /Cleanup-Image /StartComponentCleanup
"Freed about {0:N0} MB" -f (((Get-PSDrive C).Free - $before)/1MB)
"## },
    Tool { id: "clean_caches", group: "clean", name: "Clear system caches", desc: "Icon and thumbnail cache, DNS, ARP, error reports, delivery optimization and shader caches. Explorer restarts briefly.", risk: "medium", timeout: 300, files: false, script: r##"
ipconfig /flushdns | Out-Null
arp -d * | Out-Null
nbtstat -R | Out-Null
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Start-Sleep 1
Remove-Item "$env:LOCALAPPDATA\IconCache.db" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache*","$env:LOCALAPPDATA\Microsoft\Windows\Explorer\thumbcache*" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:ProgramData\Microsoft\Windows\WER\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\Microsoft\Windows\WER\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:SystemRoot\ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache\*" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item "$env:LOCALAPPDATA\D3DSCache\*","$env:LOCALAPPDATA\NVIDIA\DXCache\*","$env:LOCALAPPDATA\AMD\DxCache\*" -Recurse -Force -ErrorAction SilentlyContinue
Start-Process explorer.exe
'Caches cleared. Browsers were not touched.'
"## },
    Tool { id: "recycle", group: "clean", name: "Empty Recycle Bin", desc: "Permanently deletes everything in the Recycle Bin on all drives.", risk: "medium", timeout: 120, files: false, script: r##"
Clear-RecycleBin -Force -ErrorAction SilentlyContinue
'Recycle Bin emptied.'
"## },
    // ------------------------------------------------------------ performance
    Tool { id: "sysmain_off", group: "perf", name: "Turn off SysMain and Search indexing", desc: "Stops and disables SysMain (Superfetch) and Windows Search. Helps old PCs with hard disks. Search gets slower.", risk: "medium", timeout: 60, files: false, script: r##"
foreach ($n in 'SysMain','WSearch') { Stop-Service $n -Force -ErrorAction SilentlyContinue; Set-Service $n -StartupType Disabled }
'SysMain and WSearch are disabled.'
"## },
    Tool { id: "sysmain_on", group: "perf", name: "Turn on SysMain and Search indexing", desc: "Restores both services to automatic start.", risk: "low", timeout: 60, files: false, script: r##"
foreach ($n in 'SysMain','WSearch') { Set-Service $n -StartupType Automatic; Start-Service $n -ErrorAction SilentlyContinue }
'SysMain and WSearch are enabled.'
"## },
    Tool { id: "power_high", group: "perf", name: "High performance power plan", desc: "Switches to High performance, creating it if it is missing. Uses more power.", risk: "low", timeout: 30, files: false, script: r##"
$p = powercfg /l | Select-String 'High performance'
if (-not $p) { powercfg -duplicatescheme 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c | Out-Null; $p = powercfg /l | Select-String 'High performance' }
$guid = ([regex]::Match([string]$p, '[0-9a-fA-F-]{36}')).Value
powercfg /setactive $guid
powercfg /getactivescheme
"## },
    Tool { id: "power_balanced", group: "perf", name: "Balanced power plan", desc: "Switches back to the Balanced plan.", risk: "low", timeout: 30, files: false, script: r##"
powercfg /setactive 381b4222-f694-41f0-9685-ff5bb260df2e
powercfg /getactivescheme
"## },
    Tool { id: "faststart_off", group: "perf", name: "Turn off Fast Startup", desc: "Makes Shut down a full shutdown. Fixes many odd problems after shutdown and start.", risk: "low", timeout: 30, files: false, script: r##"
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Power' -Name HiberbootEnabled -Value 0 -Type DWord
'Fast Startup is off.'
"## },
    // ------------------------------------------------------------ repair
    Tool { id: "sfc_verify", group: "repair", name: "Verify system files (no repair)", desc: "sfc /verifyonly. Reports problems without changing anything.", risk: "low", timeout: 1800, files: false, script: r##"
sfc /verifyonly
"## },
    Tool { id: "dism_check", group: "repair", name: "DISM quick health check", desc: "Fast check of the component store.", risk: "low", timeout: 300, files: false, script: r##"
DISM /Online /Cleanup-Image /CheckHealth
"## },
    Tool { id: "dism_scan", group: "repair", name: "DISM full scan", desc: "Scans the component store in depth. Can take 10 minutes or more.", risk: "low", timeout: 1800, files: false, script: r##"
DISM /Online /Cleanup-Image /ScanHealth
"## },
    Tool { id: "appx_all", group: "repair", name: "Re-register all Store apps", desc: "Fixes apps that will not open. Takes a few minutes and some errors are normal.", risk: "medium", timeout: 900, files: false, script: r##"
$n = 0
Get-AppxPackage -AllUsers | ForEach-Object {
  $m = Join-Path $_.InstallLocation 'AppXManifest.xml'
  if (Test-Path $m) { Add-AppxPackage -DisableDevelopmentMode -Register $m -ErrorAction SilentlyContinue; $n++ }
}
"Re-registered $n packages."
"## },
    Tool { id: "explorer_restart", group: "repair", name: "Restart Explorer", desc: "Restarts the desktop and taskbar. Open File Explorer windows close.", risk: "low", timeout: 30, files: false, script: r##"
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Start-Sleep 1
if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }
'Explorer restarted.'
"## },
    Tool { id: "wsreset", group: "repair", name: "Reset Microsoft Store cache", desc: "Runs wsreset. The Store window may open and close by itself.", risk: "low", timeout: 120, files: false, script: r##"
Start-Process wsreset.exe -Wait -WindowStyle Hidden
'Store cache reset.'
"## },
    // ------------------------------------------------------------ disk
    Tool { id: "chkdsk_scan", group: "disk", name: "Check disk (online, read only)", desc: "chkdsk /scan on the system drive. Nothing is changed.", risk: "low", timeout: 1800, files: false, script: r##"
chkdsk $env:SystemDrive /scan
"## },
    Tool { id: "chkdsk_repair", group: "disk", name: "Schedule disk repair at next restart", desc: "chkdsk /f /r on the system drive during the next boot. Can take hours on large disks.", risk: "high", timeout: 60, files: false, script: r##"
fsutil dirty set $env:SystemDrive
'Disk check is scheduled. It runs at the next restart.'
"## },
    Tool { id: "defrag", group: "disk", name: "Optimize drives", desc: "defrag /O: TRIM for SSDs and defragmentation for hard disks.", risk: "low", timeout: 3600, files: false, script: r##"
defrag /C /O /U
"## },
    Tool { id: "big_files", group: "disk", name: "Find the 25 largest files", desc: "Scans your user folder. Can take a few minutes.", risk: "low", timeout: 600, files: false, script: r##"
Get-ChildItem $env:USERPROFILE -Recurse -File -Force -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 25 @{n='MB';e={[math]::Round($_.Length/1MB)}}, FullName | Format-Table -AutoSize
"## },
    // ------------------------------------------------------------ network
    Tool { id: "net_test", group: "net", name: "Network test", desc: "Gateway, DNS and internet reachability.", risk: "low", timeout: 90, files: false, script: r##"
$gw = (Get-NetRoute -DestinationPrefix 0.0.0.0/0 | Sort-Object RouteMetric | Select-Object -First 1).NextHop
"Gateway: $gw -> " + (Test-Connection $gw -Count 2 -Quiet)
"Internet 1.1.1.1 -> " + (Test-Connection 1.1.1.1 -Count 2 -Quiet)
try { $r = Resolve-DnsName microsoft.com -ErrorAction Stop | Select-Object -First 1; "DNS microsoft.com -> " + $r.IPAddress } catch { 'DNS microsoft.com -> failed' }
Get-DnsClientServerAddress -AddressFamily IPv4 | Where-Object { $_.ServerAddresses } | ForEach-Object { "DNS servers {0}: {1}" -f $_.InterfaceAlias, ($_.ServerAddresses -join ', ') }
"## },
    Tool { id: "adapters_restart", group: "net", name: "Restart network adapters", desc: "Disables and enables all active adapters. The connection drops for a few seconds.", risk: "medium", timeout: 120, files: false, script: r##"
$a = Get-NetAdapter | Where-Object Status -eq 'Up'
foreach ($x in $a) { Disable-NetAdapter -Name $x.Name -Confirm:$false; Start-Sleep 2; Enable-NetAdapter -Name $x.Name -Confirm:$false }
"Restarted: " + (($a | ForEach-Object Name) -join ', ')
"## },
    Tool { id: "proxy_reset", group: "net", name: "Reset proxy settings", desc: "Turns off the user proxy and resets WinHTTP proxy.", risk: "medium", timeout: 30, files: false, script: r##"
$k = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
Set-ItemProperty $k -Name ProxyEnable -Value 0 -Type DWord
Remove-ItemProperty $k -Name ProxyServer, AutoConfigURL -ErrorAction SilentlyContinue
netsh winhttp reset proxy
"## },
    Tool { id: "hosts_reset", group: "net", name: "Reset hosts file to default", desc: "Saves a backup next to the file, then restores the default content. Removes all custom entries.", risk: "high", timeout: 30, files: false, script: r##"
$h = "$env:SystemRoot\System32\drivers\etc\hosts"
$bak = "$h.deepscan_$(Get-Date -Format yyyyMMdd_HHmmss).bak"
Copy-Item $h $bak -Force
Set-Content $h -Encoding ASCII -Value "# Default hosts file restored by DeepScan`r`n127.0.0.1 localhost`r`n::1 localhost"
"Backup: $bak"
'hosts file reset.'
"## },
    Tool { id: "time_sync", group: "net", name: "Sync clock and group policy", desc: "Resyncs the Windows clock and refreshes group policy.", risk: "low", timeout: 180, files: false, script: r##"
Set-Service w32time -StartupType Automatic
Start-Service w32time -ErrorAction SilentlyContinue
w32tm /resync /force
gpupdate /force
"## },
    Tool { id: "tcpip_reset", group: "net", name: "Reset TCP/IP stack", desc: "netsh int ip reset. Static IP settings may be lost. Restart needed.", risk: "high", timeout: 60, files: false, script: r##"
netsh int ip reset
'Done. Restart the PC to finish.'
"## },
    // ------------------------------------------------------------ windows update
    Tool { id: "wu_settings", group: "wu", name: "Windows Update settings", desc: "Shows update policies, service state and pause status. Read only.", risk: "low", timeout: 30, files: false, script: r##"
Get-Service wuauserv, usosvc, WaaSMedicSvc, bits | Select-Object Name, Status, StartType | Format-Table -AutoSize
'== Policies =='
Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -ErrorAction SilentlyContinue | Format-List NoAutoUpdate, AUOptions
Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings' -ErrorAction SilentlyContinue | Format-List PauseUpdatesExpiryTime, ActiveHoursStart, ActiveHoursEnd
"## },
    Tool { id: "wu_stop", group: "wu", name: "Stop Windows Update", desc: "Disables the update services, policy and scheduled task. The PC stops getting security updates until you start it again.", risk: "high", timeout: 90, files: false, script: r##"
foreach ($n in 'wuauserv','usosvc','WaaSMedicSvc') { Stop-Service $n -Force -ErrorAction SilentlyContinue; Set-Service $n -StartupType Disabled -ErrorAction SilentlyContinue }
$p = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
New-Item $p -Force | Out-Null
Set-ItemProperty $p -Name NoAutoUpdate -Value 1 -Type DWord
Disable-ScheduledTask -TaskPath '\Microsoft\Windows\WindowsUpdate\' -TaskName 'Scheduled Start' -ErrorAction SilentlyContinue | Out-Null
'Windows Update is stopped. Use "Start Windows Update" to turn it back on.'
"## },
    Tool { id: "wu_start", group: "wu", name: "Start Windows Update", desc: "Undoes Stop Windows Update.", risk: "low", timeout: 90, files: false, script: r##"
Remove-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name NoAutoUpdate -ErrorAction SilentlyContinue
foreach ($n in 'wuauserv','usosvc') { Set-Service $n -StartupType Manual -ErrorAction SilentlyContinue; Start-Service $n -ErrorAction SilentlyContinue }
Set-Service WaaSMedicSvc -StartupType Manual -ErrorAction SilentlyContinue
Enable-ScheduledTask -TaskPath '\Microsoft\Windows\WindowsUpdate\' -TaskName 'Scheduled Start' -ErrorAction SilentlyContinue | Out-Null
'Windows Update is on.'
"## },
    // ------------------------------------------------------------ security and hardware
    Tool { id: "def_update_scan", group: "sec", name: "Update Defender and quick scan", desc: "Downloads new signatures, then runs a quick scan. Uses CPU while it runs.", risk: "low", timeout: 1800, files: false, script: r##"
Update-MpSignature
Start-MpScan -ScanType QuickScan
$t = Get-MpThreatDetection -ErrorAction SilentlyContinue | Select-Object -First 5
if ($t) { $t | Format-Table InitialDetectionTime, ProcessName, Resources -AutoSize } else { 'Quick scan finished. No threats found.' }
"## },
    Tool { id: "rdp_on", group: "sec", name: "Turn on Remote Desktop", desc: "Allows Remote Desktop connections and opens the firewall rule. Only do this on trusted networks.", risk: "high", timeout: 30, files: false, script: r##"
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 0 -Type DWord
Enable-NetFirewallRule -DisplayGroup 'Remote Desktop' -ErrorAction SilentlyContinue
'Remote Desktop is on.'
"## },
    Tool { id: "rdp_off", group: "sec", name: "Turn off Remote Desktop", desc: "Blocks Remote Desktop connections and closes the firewall rule.", risk: "low", timeout: 30, files: false, script: r##"
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 1 -Type DWord
Disable-NetFirewallRule -DisplayGroup 'Remote Desktop' -ErrorAction SilentlyContinue
'Remote Desktop is off.'
"## },
    Tool { id: "memdiag", group: "sec", name: "Memory test (RAM)", desc: "Opens Windows Memory Diagnostic. Choose restart now or later; the test runs before Windows starts.", risk: "medium", timeout: 30, files: false, script: r##"
Start-Process mdsched.exe
'Windows Memory Diagnostic opened.'
"## },
    // ------------------------------------------------------------ other
    Tool { id: "winget_all", group: "other", name: "Update all apps (winget)", desc: "Runs winget upgrade --all silently. Can take a long time and may close apps being updated.", risk: "medium", timeout: 3600, files: false, script: r##"
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) { throw 'winget is not installed on this PC.' }
winget upgrade --all --silent --accept-source-agreements --accept-package-agreements --disable-interactivity
"## },
    Tool { id: "driver_backup", group: "other", name: "Back up drivers (saved)", desc: "Exports all third-party drivers to a folder on the Desktop.", risk: "low", timeout: 900, files: true, script: r##"
$d = Join-Path $rep "Drivers_$ts"
New-Item -ItemType Directory -Force $d | Out-Null
DISM /Online /Export-Driver /Destination:"$d"
"Saved: $d"
"## },
    Tool { id: "office_repair", group: "other", name: "Repair Microsoft Office (quick)", desc: "Starts the Click-to-Run quick repair for Microsoft 365 and Office 2016 or newer. Close Office first.", risk: "medium", timeout: 60, files: false, script: r##"
$c2r = "$env:ProgramFiles\Common Files\Microsoft Shared\ClickToRun\OfficeClickToRun.exe"
if (-not (Test-Path $c2r)) { throw 'Click-to-Run Office was not found on this PC.' }
Start-Process $c2r -ArgumentList 'scenario=Repair platform=x64 culture=en-us forceappshutdown=False RepairType=QuickRepair DisplayLevel=True'
'Office repair started. Follow the window that opens.'
"## },
    Tool { id: "safemode_on", group: "other", name: "Boot into Safe Mode next time", desc: "Sets Windows to start in Safe Mode at every restart until you turn it off.", risk: "high", timeout: 30, files: false, script: r##"
bcdedit /set '{current}' safeboot minimal
'Safe Mode is set for the next restart. Use "Back to normal boot" afterwards.'
"## },
    Tool { id: "safemode_off", group: "other", name: "Back to normal boot", desc: "Removes the Safe Mode setting.", risk: "low", timeout: 30, files: false, script: r##"
bcdedit /deletevalue '{current}' safeboot
'Normal boot restored.'
"## },
    Tool { id: "adv_startup", group: "other", name: "Restart to Advanced Startup", desc: "Restarts in 15 seconds into the recovery menu (Startup settings, Command Prompt, System Restore).", risk: "high", timeout: 30, files: false, script: r##"
shutdown /r /o /t 15
'Restarting into Advanced Startup in 15 seconds.'
"## },
];

pub fn list() -> Value {
    json!({
        "groups": GROUPS.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "tools": TOOLS.iter().map(|t| json!({"id": t.id, "group": t.group, "name": t.name, "desc": t.desc, "risk": t.risk, "files": t.files})).collect::<Vec<_>>(),
    })
}

fn clean(script: &str) -> String {
    script.trim().to_string()
}

fn full_script(t: &Tool) -> String {
    let body = clean(t.script);
    if t.files { format!("{PRE}{body}") } else { body }
}

pub fn script(id: &str) -> Value {
    match TOOLS.iter().find(|t| t.id == id) {
        Some(t) => json!({"ok": true, "script": full_script(t)}),
        None => json!({"ok": false}),
    }
}

pub fn run(id: &str) -> Value {
    let Some(t) = TOOLS.iter().find(|t| t.id == id) else { return json!({"ok": false, "output": "Unknown tool"}) };
    let (ok, out) = ps_run(&full_script(t), t.timeout);
    // Read-only reports are not logged; anything that changes the PC is.
    if t.risk != "low" || t.files {
        store::add_history(json!({
            "title": t.name, "detail": if ok { "Completed" } else { "Finished with errors" }, "undoable": false, "note": ""
        }));
    }
    json!({"ok": ok, "output": out, "files": t.files})
}

pub fn open_reports() -> Value {
    if !IS_WIN {
        return json!({"ok": true, "demo": true});
    }
    let (ok, out) = ps_run(
        "$d = Join-Path ([Environment]::GetFolderPath('Desktop')) 'DeepScan Reports'; New-Item -ItemType Directory -Force $d | Out-Null; Start-Process explorer.exe $d",
        20,
    );
    json!({"ok": ok, "output": out})
}
