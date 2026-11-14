# Changelog

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
