//! Keep the host Vulkan stack preferred; use an isolated CPU stack if it has no device.
use anyhow::{Context, Result};
use ash::{Entry, vk};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::Command,
};

fn host_has_device() -> bool {
    // No surfaces or application GPU resources exist yet. Destroy the probe
    // before choosing a renderer so driver dependencies cannot leak across exec.
    unsafe {
        let Ok(entry) = Entry::load() else {
            return false;
        };
        let application = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_0);
        let info = vk::InstanceCreateInfo::default().application_info(&application);
        let Ok(instance) = entry.create_instance(&info, None) else {
            return false;
        };
        let available = instance
            .enumerate_physical_devices()
            .is_ok_and(|devices| !devices.is_empty());
        instance.destroy_instance(None);
        available
    }
}

pub fn fallback_command(library: &Path) -> Result<Option<Command>> {
    let Some(root) = std::env::var_os("OMAPDF_PORTABLE_ROOT") else {
        return Ok(None);
    };
    if std::env::var_os("OMAPDF_SOFTWARE_VULKAN").is_some() {
        return Ok(None);
    }
    // Forwarded commands should not initialize graphics. This is only a hint;
    // Server::start still owns the authoritative lock/forwarding protocol.
    if let Ok(lock) = OpenOptions::new()
        .read(true)
        .write(true)
        .open(library.with_extension("lock"))
        && matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock))
    {
        return Ok(None);
    }
    if host_has_device() {
        return Ok(None);
    }
    let root = PathBuf::from(root);
    let software = root.join("software-vulkan");
    let manifest = software.join("icd.json");
    anyhow::ensure!(
        manifest.is_file(),
        "No host Vulkan device or bundled CPU renderer is available"
    );
    let mut libraries = vec![software.join("lib")];
    if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
        libraries.extend(std::env::split_paths(&existing));
    }
    // The portable launcher invokes ld-linux explicitly, so /proc/self/exe
    // can identify the ELF loader rather than the reader being restarted.
    let mut command = Command::new(root.join("bin/omapdf"));
    command
        .args(std::env::args_os().skip(1))
        .env(
            "LD_LIBRARY_PATH",
            std::env::join_paths(libraries).context("CPU renderer library paths")?,
        )
        .env("VK_ICD_FILENAMES", &manifest)
        .env("VK_DRIVER_FILES", &manifest)
        .env("NODEVICE_SELECT", "1")
        .env("VK_LOADER_LAYERS_DISABLE", "~implicit~")
        .env("OMAPDF_SOFTWARE_VULKAN", "1");
    eprintln!("Omapdf: using bundled software Vulkan renderer (no host Vulkan device)");
    Ok(Some(command))
}
