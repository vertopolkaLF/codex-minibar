use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // windows-reactor-setup extracts the Windows App SDK MSIX into a shared
    // folder without an arch suffix. Multi-arch release builds can leave the
    // wrong native DLLs cached; drop them before deploy when the PE machine
    // does not match this target (otherwise host `cargo run` loads ARM64/x86
    // runtimes into an x64 process → ERROR_BAD_EXE_FORMAT / 0x800700C1).
    invalidate_stale_was_extract();

    windows_reactor_setup::as_self_contained();

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/app-icon.ico");
        resource
            .compile()
            .expect("compile Windows application icon");
    }
}

fn invalidate_stale_was_extract() {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let extract = PathBuf::from(local)
        .join("windows-reactor-setup")
        .join("temp")
        .join("Microsoft.WindowsAppSDK.Runtime-2.1.3")
        .join(".msix_extract");
    let dll = extract.join("Microsoft.WindowsAppRuntime.dll");
    if !dll.is_file() {
        return;
    }

    let want = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => 0x014C_u16,
        Ok("aarch64") => 0xAA64,
        _ => 0x8664, // x86_64
    };

    match pe_machine(&dll) {
        Some(machine) if machine == want => {}
        _ => {
            let _ = fs::remove_dir_all(&extract);
        }
    }
}

fn pe_machine(path: &Path) -> Option<u16> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 0x40 || bytes[0] != b'M' || bytes[1] != b'Z' {
        return None;
    }
    let pe_off = u32::from_le_bytes(bytes[0x3C..0x40].try_into().ok()?) as usize;
    let machine_off = pe_off.checked_add(4)?;
    let end = machine_off.checked_add(2)?;
    if bytes.len() < end {
        return None;
    }
    Some(u16::from_le_bytes(bytes[machine_off..end].try_into().ok()?))
}
