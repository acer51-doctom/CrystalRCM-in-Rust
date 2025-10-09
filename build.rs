// build.rs
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let app_name = "crystalrcm-in-rust";

    // Only for macOS
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_dir = PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap_or("target".into()));
    let release_dir = target_dir.join("release");
    let app_bundle = release_dir.join(format!("{}.app", app_name));

    // --- Create .app structure ---
    let macos_dir = app_bundle.join("Contents/MacOS");
    let resources_dir = app_bundle.join("Contents/Resources");
    let frameworks_dir = app_bundle.join("Contents/Frameworks");

    fs::create_dir_all(&macos_dir).unwrap();
    fs::create_dir_all(&resources_dir).unwrap();
    fs::create_dir_all(&frameworks_dir).unwrap();

    // --- Copy executable ---
    let exe_path = release_dir.join(app_name);
    fs::copy(&exe_path, macos_dir.join(app_name)).unwrap();

    // --- Copy icon ---
    let icon_src = manifest_dir.join("icon.icns");
    if icon_src.exists() {
        fs::copy(&icon_src, resources_dir.join("AppIcon.icns")).unwrap();
    }

    // --- Copy libusb ---
    let lib_src = manifest_dir.join("assets/libusb.lib");
    if lib_src.exists() {
        fs::copy(&lib_src, frameworks_dir.join("libusb.lib")).unwrap();
    }

    // --- Read version from Cargo.toml ---
    let cargo_toml = fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    let version = cargo_toml
        .lines()
        .find(|l| l.starts_with("version"))
        .and_then(|l| l.split('"').nth(1))
        .unwrap_or("0.1.0");

    // --- Generate Info.plist ---
    let plist_path = app_bundle.join("Contents/Info.plist");
    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{0}</string>
    <key>CFBundleIdentifier</key>
    <string>com.crystalrcm.{0}</string>
    <key>CFBundleName</key>
    <string>{0}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{1}</string>
    <key>CFBundleVersion</key>
    <string>{1}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon.icns</string>
</dict>
</plist>"#,
        app_name, version
    );
    fs::write(plist_path, plist_content).unwrap();

    // --- Optional ad-hoc signing ---
    let _ = Command::new("codesign")
        .args(&["--force", "--sign", "-", app_bundle.to_str().unwrap()])
        .status();

    // --- Dynamic linking setup ---
    println!("cargo:rustc-link-search=native={}", manifest_dir.join("assets").to_string_lossy());
    println!("cargo:rustc-link-lib=dylib=usb-1.0");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
}
