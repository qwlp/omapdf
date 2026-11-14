# Implementation notes

Omapdf is a native Linux Wayland PDF and EPUB reader written in Rust with GPUI.
MuPDF provides document parsing, text extraction, rendering and PDF annotation
interchange. Document processing does not require external command-line tools.

## Architecture

- `main.rs`, `workspace.rs`, `ipc.rs`: startup, tabs/windows and single-instance command forwarding per library.
- `document.rs`, `raster.rs`: native MuPDF sessions, lazy PDF text extraction, background indexing and raster caching.
- `reader.rs`, `layout.rs`, `scroll.rs`: interaction, page geometry, four layouts and smooth scrolling.
- `config.rs`, `settings.rs`, `commands.rs`: preferences, keybindings, settings UI and command help.
- `state.rs`, `interchange.rs`, `legacy_sqlite.rs`: persistent state, backups and read-only library migration.
- `annotations.rs`, `selection.rs`, `region.rs`: PDF annotations, cross-page visible selection and native image capture.
- `reload.rs`: stable document/configuration reload with invalid-write recovery.

Rendering uses a dedicated native worker, adjacent-page prefetch and a pixel cache
targeting 128 MiB. Active images and native allocations consume additional memory;
a single oversized raster can exceed that target. PDF geometry loads before its
full text index, so large documents can be read while indexing continues. EPUB
text extraction is eager to support text-anchor remapping during reflow.

Document-open command continuations follow their destination tabs/windows and
password prompts. Unlocked document credentials remain process-local. Annotation
exports preserve source encryption and write new PDFs without replacing originals.
Library saves use atomic replacement; backups and migration retain source records.

See [the roadmap](ROADMAP.md) for TODOs, [supported commands](COMMANDS.md), and
[validation](VALIDATION.md) for test coverage. Dispatcher coverage alone does not
establish full behavior for a feature.
