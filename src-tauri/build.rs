fn main() {
    // The manifest makes Windows ask for administrator rights at launch (needed for BitLocker, TPM and SMBv1 checks).
    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest")));
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
