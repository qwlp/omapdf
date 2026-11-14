//! A static launcher for the bundled GPUI runtime. GPU drivers remain host supplied.
use flate2::read::GzDecoder;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{OpenOptionsExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Component, Path, PathBuf},
    process::Command,
};
#[cfg(target_arch = "x86_64")]
const LOADER: &str = "/lib64/ld-linux-x86-64.so.2";
#[cfg(target_arch = "aarch64")]
const LOADER: &str = "/lib/ld-linux-aarch64.so.1";
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("Portable builds support Linux x86-64 and ARM64");
const PAYLOAD: &[u8] = include_bytes!(env!("OMAPDF_PORTABLE_PAYLOAD"));
const VERSION: &str = env!("OMAPDF_PORTABLE_VERSION");
unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
fn unpack(root: &Path) -> io::Result<()> {
    let mut decoder = GzDecoder::new(PAYLOAD);
    loop {
        let mut length = [0u8; 4];
        decoder.read_exact(&mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 {
            break;
        }
        if length > 4096 {
            return Err(io::Error::other("Invalid bundled filename"));
        }
        let mut path = vec![0; length];
        decoder.read_exact(&mut path)?;
        let path = PathBuf::from(String::from_utf8(path).map_err(io::Error::other)?);
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::other("Invalid bundled path"));
        }
        let mut length = [0u8; 8];
        decoder.read_exact(&mut length)?;
        let length = u64::from_le_bytes(length);
        if length > 512 * 1024 * 1024 {
            return Err(io::Error::other("Invalid bundled file size"));
        }
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(path)?;
        let copied = io::copy(&mut (&mut decoder).take(length), &mut file)?;
        if copied != length {
            return Err(io::Error::other("Truncated bundled runtime"));
        }
    }
    Ok(())
}
fn run() -> io::Result<()> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or_else(|| {
            io::Error::other("Set HOME or XDG_CACHE_HOME for portable runtime extraction")
        })?
        .join("omapdf/runtime");
    fs::create_dir_all(&cache)?;
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))?;
    let root = cache.join(VERSION);
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(cache.join(format!("{VERSION}.lock")))?;
    // SAFETY: lock is an open file descriptor; flock releases automatically on close.
    if unsafe { flock(lock.as_raw_fd(), 2) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if !root.join("COMPLETE").exists() {
        let staging = cache.join(format!("{VERSION}.{}", std::process::id()));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir(&staging)?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
        if let Err(error) = unpack(&staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        File::create(staging.join("COMPLETE"))?.write_all(VERSION.as_bytes())?;
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::rename(staging, &root)?;
    }
    drop(lock);
    let libraries = root.join("lib");
    // Host ICDs/layers need the host glibc ABI. The loader and all glibc DSOs
    // must stay together; using a bundled loader with host libc is unsafe too.
    if !Path::new(LOADER).is_file() {
        return Err(io::Error::other(format!(
            "Host glibc loader {LOADER} is required (musl systems are unsupported)"
        )));
    }
    let mut command = Command::new(LOADER);
    command
        .arg("--library-path")
        .arg(&libraries)
        .arg(root.join("bin/omapdf"))
        .args(std::env::args_os().skip(1));
    command
        .env("LD_LIBRARY_PATH", &libraries)
        .env("OMAPDF_PORTABLE_ROOT", &root);
    // Include bundled UI fonts alongside the host's configured fonts.
    command.env("FONTCONFIG_FILE", root.join("fonts.conf"));
    command.env("XKB_CONFIG_ROOT", root.join("share/X11/xkb"));
    command.env("XLOCALEDIR", root.join("share/X11/locale"));
    Err(command.exec())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Omapdf portable runtime: {error}");
        std::process::exit(1);
    }
}
