# GPUI compatibility patch

Upstream: crates.io `gpui` 0.2.2, package SHA-256 `979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707`.
Original Apache-2.0 licensing and source notices are retained.

The Wayland clipboard previously advertised only text MIME types and sent only
text payloads even for `ClipboardEntry::Image`. `client.rs` now advertises the
actual image MIME types (and text only when present). `clipboard.rs` sends the
matching encoded image bytes through the asynchronous data-source writer, with
a nonblocking descriptor so large images cannot block the UI on a slow reader.
The selection uses the most recent keyboard or pointer serial (tracked in
`serial.rs`), including pointer-only captures and serial-number wraparound.
No clipboard subprocess or extra runtime dependency is introduced.

The changes are in `src/platform/linux/wayland/client.rs` and
`src/platform/linux/wayland/clipboard.rs`, plus input serial tracking in `serial.rs`. `scripts/region_smoke.py` exercises
real Wayland PNG offers and checks their decoded pixels against native exports.

Startup: Wayland Vulkan device initialization now runs concurrently with
display/keyboard setup. Cosmic Text starts with an empty font database instead
of scanning every installed system font file on startup, loading requested font
families on demand via fontconfig's binary cache and falling back to a full
system font scan only when needed for missing Unicode fallback glyphs or font
listing. Wayland cursor themes are loaded lazily only when server-side cursor
shapes are unavailable. Wayland XKB compose tables are loaded lazily on first
non-modifier key press instead of blocking initial window configuration.
Wayland windows with xdg-decoration support start with server-side opaque
surfaces instead of compiling transparent pipelines and immediately recompiling
opaque ones. Blade pipelines are compiled in parallel and cached per
`(format, alpha, sample_count)` on `BladeContext`, skipping the macOS-only
`surfaces` pipeline on Linux. Wayland and X11 `drop_window` no longer forcibly stop the event loop when the
last window closes; application exit is controlled by `on_window_closed` /
`cx.quit()`, enabling resident `--daemon` mode while preserving normal exit on
last-window close. Changes are in `wayland/client.rs`, `x11/client.rs`,
`wayland/cursor.rs`, `wayland/window.rs`, `blade/blade_context.rs`,
`blade/blade_renderer.rs`, and `text_system.rs`.

X11 startup: checked XCB round trips may buffer MapNotify/Expose while consuming
the socket readiness notification. The client drains buffered events before the
event loop blocks and after each dispatch, allowing its initial refresh timer
to start even on an idle Xvfb display. The offline X11 smoke also checks real
painted pixels before and after resizing the client window.

The X11 refresh loop queries RandR using the screen root and retains a 60 Hz
refresh fallback if mode discovery fails, so headless/older X servers cannot
leave a mapped window without a repaint timer. OMAPDF_TRACE_X11 enables one-frame
and refresh-timing diagnostics used by the offline paint regression checks.
