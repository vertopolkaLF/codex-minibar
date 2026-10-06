use std::path::Path;

fn main() {
    embed_application_manifest();

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app-icon.ico");
        resource
            .compile()
            .expect("compile Windows application icon");
    }
}

/// Embed the application manifest: Common Controls v6 (GPUI and the popup
/// host import TaskDialogIndirect/SetWindowSubclass) and per-monitor v2 DPI
/// awareness, which both GPUI windows rely on for crisp scaling.
fn embed_application_manifest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_abi = std::env::var("CARGO_CFG_TARGET_ABI").unwrap_or_default();
    match (target_env.as_str(), target_abi.as_str()) {
        ("msvc", _) => {
            println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
                manifest.display()
            );
        }
        ("gnu", "llvm") => {
            println!("cargo:rustc-link-arg-bins=-Wl,/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bins=-Wl,/MANIFESTINPUT:{}",
                manifest.display()
            );
        }
        _ => {}
    }
}
