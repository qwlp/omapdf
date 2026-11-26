mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub(crate) use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

#[cfg(all(feature = "screen-capture", any(feature = "wayland", feature = "x11")))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(all(feature = "screen-capture", any(feature = "wayland", feature = "x11"))))]
pub(crate) type PlatformScreenCaptureFrame = ();

pub(crate) fn profile_startup(event: &str) {
    use std::{
        sync::OnceLock,
        time::Instant,
    };
    static ORIGIN: OnceLock<Option<Instant>> = OnceLock::new();
    if let Some(origin) = *ORIGIN.get_or_init(|| {
        std::env::var_os("OMAPDF_PROFILE_STARTUP")
            .filter(|v| !v.is_empty() && v != "0")
            .map(|_| Instant::now())
    }) {
        eprintln!("[gpui +{:.2}ms] {event}", origin.elapsed().as_secs_f64() * 1000.0);
    }
}
