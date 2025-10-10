#!/usr/bin/env bash
set -euo pipefail

# Trap errors
trap 'echo "Uh oh! An error occured. Please check output!" >&2' ERR

# --- Config ---
APP_NAME="crystalrcm-rust-edition" # VERY CASE SENSITIVE.
TARGET_DIR="/target/release"
BUNDLE_PATH="${TARGET_DIR}/${APP_NAME}.app"
MACOS_DIR="${BUNDLE_PATH}/Contents/MacOS"
RESOURCES_DIR="${BUNDLE_PATH}/Contents/Resources"
FRAMEWORKS_DIR="${BUNDLE_PATH}/Contents/Frameworks"

echo "🚀 Building Rust binaries for $(rustc --version)..."
cargo update
cargo clean
clear
echo "Initiating building process! 3... 2... 1... GO!"
cargo build --release

# --- Create .app bundle ---
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR" "$FRAMEWORKS_DIR"

# Copy executable
cp "${TARGET_DIR}/${APP_NAME}" "$MACOS_DIR/"

# Copy icon if it exists
if [ -f "./icon.icns" ]; then
    cp "./icon.icns" "$RESOURCES_DIR/AppIcon.icns"
fi

# Copy libusb if it exists
if [ -f "./assets/libusb.lib" ]; then
    cp "./assets/libusb.lib" "$FRAMEWORKS_DIR/libusb.lib"
fi

# Read version from Cargo.toml
VERSION=$(grep -m 1 '^version' Cargo.toml | sed -E 's/version *= *"([^"]+)"/\1/')

# Generate Info.plist
cat > "$BUNDLE_PATH/Contents/Info.plist" <<EOL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.acer51doctom.crystalrcm</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
EOL

if [ -f "./icon.icns" ]; then
cat >> "$BUNDLE_PATH/Contents/Info.plist" <<EOL
    <key>CFBundleIconFile</key>
    <string>AppIcon.icns</string>
EOL
fi

cat >> "$BUNDLE_PATH/Contents/Info.plist" <<EOL
</dict>
</plist>
EOL

# Sign the app
codesign --force --sign - "$BUNDLE_PATH" || echo "⚠️ Codesign failed (may require permissions)"

echo "✅ .app bundle created at ${BUNDLE_PATH}"
