// build.rs - This script configures the build process.
// For macOS, it tells the Rust compiler to:
// 1. Look for the libusb dynamic library in our local 'assets' folder.
// 2. Link against it.
// 3. Set the "rpath" on the final executable, which is a runtime search path.
//    This tells the program to look for the library inside the Frameworks
//    folder of its own .app bundle.

use std::env;
use std::path::PathBuf;

fn main() {
    // This build configuration is specific to macOS.
    if cfg!(target_os = "macos") {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let lib_dir = manifest_dir.join("assets");

        // 1. Tell rustc where to find the library.
        // The linker will search in the `assets/` directory.
        println!("cargo:rustc-link-search=native={}", lib_dir.to_string_lossy());

        // 2. Tell rustc to link against `libusb-1.0`.
        // The linker will look for a file named `libusb-1.0.dylib` in the search path.
        println!("cargo:rustc-link-lib=dylib=usb-1.0");

        // 3. Set the rpath for the final executable.
        // This tells the executable to look for dynamic libraries in the
        // `../Frameworks` directory relative to its own location. This is the
        // standard location for libraries inside a macOS .app bundle.
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    }

    // This part uses the embed-resource crate to create the basic .app structure
    // and embed the icon, as specified in `build/app.rc`.
    embed_resource::compile("build/app.rc", embed_resource::NONE);
}

