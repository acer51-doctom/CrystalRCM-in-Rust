#!/usr/bin/env bash

# This script compiles the Rust application and bundles the required
# libusb dynamic library into the final .app package for distribution.
# It also embeds a custom app icon (icon.icns) into the .app bundle.

# --- Configuration ---
APP_NAME="crystalrcm-in-rust"
LIB_NAME="libusb.lib"
SRC_LIB_PATH="./assets/${LIB_NAME}"
ICON_PATH="./icon.icns"

# --- Script ---
echo "Starting macOS application bundle process..."

# 1. Check if the libusb library exists
if [ ! -f "$SRC_LIB_PATH" ]; then
    echo "Error: Library not found at ${SRC_LIB_PATH}"
    exit 1
fi
echo "Found ${LIB_NAME}."

# 2. Check if icon.icns exists
if [ ! -f "$ICON_PATH" ]; then
    echo "⚠️ Warning: No icon found at ${ICON_PATH}. The app will use the default icon."
fi

# 3. Compile the application in release mode
echo "Building Rust application..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "Error: Cargo build failed."
    exit 1
fi
echo "Build successful."

# 4. Define paths for the final .app bundle
BUNDLE_PATH="./target/release/${APP_NAME}.app"
CONTENTS_PATH="${BUNDLE_PATH}/Contents"
MACOS_PATH="${CONTENTS_PATH}/MacOS"
RESOURCES_PATH="${CONTENTS_PATH}/Resources"
FRAMEWORKS_PATH="${CONTENTS_PATH}/Frameworks"

# 5. Create necessary directories
echo "Setting up bundle structure..."
mkdir -p "$MACOS_PATH" "$RESOURCES_PATH" "$FRAMEWORKS_PATH"

# 6. Copy the executable
echo "Copying built executable..."
cp "./target/release/${APP_NAME}" "${MACOS_PATH}/${APP_NAME}"

# 7. Add icon
if [ -f "$ICON_PATH" ]; then
    echo "Adding icon..."
    cp "$ICON_PATH" "${RESOURCES_PATH}/AppIcon.icns"
else
    echo "Skipping icon (no icon.icns found)."
fi

# 8. Create Info.plist
PLIST_PATH="${CONTENTS_PATH}/Info.plist"
echo "Generating Info.plist..."

cat > "$PLIST_PATH" <<EOL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" \
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.${APP_NAME}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
EOL

if [ -f "$ICON_PATH" ]; then
cat >> "$PLIST_PATH" <<EOL
    <key>CFBundleIconFile</key>
    <string>AppIcon.icns</string>
EOL
fi

cat >> "$PLIST_PATH" <<EOL
</dict>
</plist>
EOL

# 9. Copy the dynamic library into Frameworks
echo "Copying ${LIB_NAME} to the app bundle..."
cp "$SRC_LIB_PATH" "$FRAMEWORKS_PATH/"

# 10. Finalize
if [ $? -eq 0 ]; then
    echo "---"
    echo "✅ Success! Your self-contained macOS application is ready at:"
    echo "   ${BUNDLE_PATH}"
    echo "---"
else
    echo "Error: Failed to copy library to the app bundle."
    exit 1
fi
