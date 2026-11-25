# Changelog

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
