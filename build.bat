@echo off
rem Build DeepScan.exe on Windows. Needs Rust (https://rustup.rs) - WebView2 is already part of Windows 10/11.
cargo build --release -p deepscan || exit /b 1
echo.
echo Done: target\release\DeepScan.exe
