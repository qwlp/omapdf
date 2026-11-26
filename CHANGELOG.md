# Changelog

## 0.0.12

- Load host and bundled UI fonts on demand by matching face rather than scanning
  the entire system font database or loading every weight/style variant on startup,
  eliminating cold-cache font stalls and reducing memory usage.
- Compile GPUI Blade render pipelines in parallel, cache them across windows, skip
  redundant opaque/transparent pipeline recompilation on Wayland compositors with
  server-side decorations, and defer Wayland cursor-theme and XKB compose-table
  initialization until needed.
- Pre-rasterize the initial page in parallel with GPU initialization, render MuPDF
  pages directly to 4-channel BGRA pixmaps, share one Condvar-driven rasterizer
  with neighbor prefetching across tabs, and remove the accidental scroll debounce
  on newly visible pages.
- Replace IPC polling loops with zero-polling event wakeups, move routine session
  and library persistence off the UI thread, and cache per-frame page layout and
  config metadata stamps.
- Add optional `--daemon` resident mode to keep the GPU, font, and rasterizer
  runtime warm with zero windows for ~20 ms subsequent document opens.

## 0.0.11

- Add a compact horizontal tab strip and a vertical sidebar option under Appearance.
- Restore the main window’s tab order and active document when launched without a file.
- Add Vim-style tab switching, tab closing, and menu navigation.
- Add display-independent regression tests for session persistence and keyboard routing.

## 0.0.10

- Use a pinned Mesa 25 backport built for Ubuntu 22.04 for the AppImage-only CPU
  renderer; Ubuntu’s stock Mesa 23 submits frames but leaves this UI blank.
- Prefer hardware Vulkan devices, choosing the known-good bundled CPU renderer
  instead of an older software-only host driver.
- Keep glibc 2.35 compatibility and the standalone executable free of CPU-renderer
  dependencies. Validate actual UI pixels before and after X11 window resizing.
- Retain a 60 Hz X11 repaint fallback when RandR timing discovery is unavailable.

The 0.0.9 paint-check gate rejected the older Mesa stack before publication.

## 0.0.9

- Drain XCB events buffered by checked requests so initial X11 map/expose events
  can start the refresh loop without waiting for unrelated pointer input.
- Check actual painted window pixels before and after a catalog-sized resize,
  in addition to offline startup survival, for both rendering modes.
- Keep CPU-rendering dependencies exclusive to the AppImage.

## 0.0.8

- Add a Mesa Lavapipe CPU renderer to AppImages only for machines with no host Vulkan
  device, including the AppImage catalog’s driverless test VM. Keep the normal
  standalone executable lean; desktops with working drivers use their host GPU.
- Restart into an isolated software-renderer library path before acquiring the
  primary-instance lock; keep glibc and GCC libraries on the host in both modes.
- Test offline windows with host ICD discovery disabled on both architectures
  and on an independent Ubuntu 22.04 runtime host.

The 0.0.7 release gate caught the missing host Vulkan driver and prevented its
publication; its XCB keyboard-library fix is included here.

## 0.0.7

- Bundle the XCB keyboard extension required by the X11 keyboard bridge; it is
  absent on the AppImage catalog’s otherwise complete X11 test system.
- Verify that the extracted reader loads both keyboard libraries from its
  bundled runtime, and gate releases on a separate Ubuntu 22.04 startup test
  without development-library packages.

## 0.0.6

- Enable native X11 window support when no Wayland display is available, fixing
  startup on X11 desktops and the AppImage catalog’s Xvfb test environment.
  Wayland remains preferred when both display variables are set.
- Bundle the X11 keyboard bridge while keeping host X11/XCB libraries aligned
  with the host Vulkan driver.
- Test visible portable and AppImage windows under Xvfb with Internet sockets
  blocked, and verify they survive beyond the catalog’s startup timeout.

## 0.0.5

- Build release downloads in Ubuntu 22.04 containers so the reader and bundled
  libraries work with glibc 2.35 while continuing to use the host glibc and GPU
  libraries. Previously, Ubuntu 24.04 builds required newer glibc symbols.
- Normalize absolute loader paths reported by newer `ldd` versions so the
  host ELF loader is always excluded from the package.
- Check every embedded ELF file in portable executables and AppImages against
  the release glibc baseline, rather than only inspecting the static launcher.

## 0.0.4

- Give mouse-wheel scrolling a shorter, crisper animation with brief acceleration
  and immediate direction changes; leave keyboard and touchpad behavior unchanged.
- Add a Classic mouse-wheel glide setting to restore the original animation.
- Add optional page separator lines and a draggable document scrollbar, including
  horizontal navigation for horizontal strip layouts. Both default to off.
- Add Wayland checks for both wheel modes, separator visibility, scrollbar clicks
  and dragging, and saved navigation preferences.

## 0.0.3

- Fix startup with current Mesa/RADV drivers requiring Wayland symbols such as
  `wl_display_create_queue_with_name` and `wl_fixes_interface`.
- Use host Wayland libraries and the host Vulkan loader instead of bundling
  older copies that can shadow driver dependencies.
- Extend extracted-runtime regression checks to reject bundled Wayland and
  Vulkan libraries.

## 0.0.2

- Fix portable startup failures when current host Vulkan drivers require glibc
  symbol versions absent from the bundled runtime, including the reported
  AMD RADV `GLIBC_ABI_GNU2_TLS` failure.
- Use the host ELF loader, glibc (including libm and compatibility libraries)
  and GCC runtime instead of bundling versions that can conflict with GPU
  drivers. Release downloads require glibc-based Ubuntu 24.04 or newer.
- Add portable library-resolution regression coverage to x86-64 and ARM64 CI,
  plus an optional host Vulkan enumeration probe.

The separate Intel GPUI initialization report has not been reproduced; this
release addresses driver ABI conflicts rather than claiming a shader fix.
