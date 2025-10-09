#!/usr/bin/env bash
set -e

APP_NAME="crystalrcm-in-rust"
TARGET_DIR="./target/release"
BUNDLE_PATH="${TARGET_DIR}/${APP_NAME}.app"
MACOS_DIR="${BUNDLE_PATH}/Contents/MacOS"
RESOURCES_DIR="${BUNDLE_PATH}/Contents/Resources"
FRAMEWORKS_DIR="${BUNDLE_PATH}/Contents/Frameworks"

echo "🚀 Building Rust binary..."
cargo clean
clear
cargo build --release

# 1. Create .app structure
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR" "$FRAMEWORKS_DIR"

# 2. Copy executable
cp "${TARGET_DIR}/${APP_NAME}" "$MACOS_DIR/${APP_NAME}"

# 3. Copy icon if it exists
if [ -f "./icon.icns" ]; then
    cp "./icon.icns" "$RESOURCES_DIR/AppIcon.icns"
fi

# 4. Copy libusb
if [ -f "./assets/libusb.lib" ]; then
    cp "./assets/libusb.lib" "$FRAMEWORKS_DIR/libusb.lib"
fi

# 5. Read version from Cargo.toml
VERSION=$(grep -m 1 '^version' Cargo.toml | sed -E 's/version *= *"([^"]+)"/\1/')

# 6. Generate Info.plist
cat > "$BUNDLE_PATH/Contents/Info.plist" <<EOL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
"http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.crystalrcm.${APP_NAME}</string>
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

# 7. Optional ad-hoc codesign
codesign --force --sign - "$BUNDLE_PATH" || echo "⚠️ Codesign failed (may require permissions)"

echo "✅ App bundle created at ${BUNDLE_PATH}"
