#!/usr/bin/env bash

# This script compiles the Rust application and bundles the required
# libusb dynamic library into the final .app package for distribution.

# --- Configuration ---
# The name of your application's binary produced by cargo.
# This should match the `name` field in your Cargo.toml.
APP_NAME="crystalrcm-in-rust"

# The name of the libusb dynamic library file in your assets folder.
# IMPORTANT: Make sure your library is named this way!
LIB_NAME="libusb.lib"
SRC_LIB_PATH="./assets/${LIB_NAME}"

# --- Script ---
echo "Starting macOS application bundle process..."

# 1. Check if the libusb library exists
if [ ! -f "$SRC_LIB_PATH" ]; then
    echo "Error: Library not found at ${SRC_LIB_PATH}"
    echo "Please place your libusb dynamic library there and name it correctly."
    exit 1
fi
echo "Found ${LIB_NAME}."

# 2. Compile the application in release mode
echo "Building Rust application..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "Error: Cargo build failed."
    exit 1
fi
echo "Build successful."

# 3. Define paths for the final .app bundle
BUNDLE_PATH="./target/release/${APP_NAME}.app"
FRAMEWORKS_PATH="${BUNDLE_PATH}/Contents/Frameworks"

# 4. Create the Frameworks directory inside the .app bundle
echo "Creating Frameworks directory at ${FRAMEWORKS_PATH}"
mkdir -p "$FRAMEWORKS_PATH"

# 5. Copy the dynamic library into the Frameworks directory
echo "Copying ${LIB_NAME} to the app bundle..."
cp "$SRC_LIB_PATH" "$FRAMEWORKS_PATH/"

if [ $? -eq 0 ]; then
    echo "---"
    echo "✅ Success! Your self-contained application is ready at:"
    echo "   ${BUNDLE_PATH}"
    echo "---"
else
    echo "Error: Failed to copy library to the app bundle."
    exit 1
fi
