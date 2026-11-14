# Verification — 2026-11-14

## Omapdf identity and configuration

The application package, executable, Rust crate, Wayland IDs, desktop entry,
portable launcher, runtime cache and default configuration/library paths use
Omapdf. Supported defaults are bundled in `assets/prefs.config`; unused upstream
configuration/reference assets have been removed. Command descriptions and the
supported-command inventory are generated from Omapdf's own documentation data.
Missing functionality is tracked as unchecked TODOs in [ROADMAP.md](ROADMAP.md).

Checks for this change:

- `cargo fmt --all --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed; vendored GPUI still emits its existing dependency warnings.
- `cargo test --locked`: passed, 29 unit tests and 47 integration tests. The test suite needs local Unix-socket access.
- `cargo build --release --locked`: passed.
- Python scripts compile; the source desktop entry validates.
- Portable packaging builds a static PIE launcher with the native reader and runtime.
- An isolated Wayland identity check verified `io.omapdf`, native Wayland windows, default XDG `omapdf` config/data/cache paths, settings persistence and IPC forwarding.
- Settings smoke: passed against the release build.
- Live-reload smoke: passed against the portable build with external tools removed from its PATH.
- Staged installation and repeated installation: passed; the binary matches the artifact, existing installations receive backups, and desktop entries validate.

Logs are under `/tmp/omapdf-rename-{clippy,tests,release,portable,identity,settings,reload,install}.log`.

## Original CC0 fixtures

All former bundled publications have been removed. The replacement PDF/EPUB
files contain original prose and generated geometric graphics, dedicated to the
public domain under CC0-1.0. Their generator uses only Python's standard library.
See [examples/README.md](../examples/README.md) and [the dedication](../examples/LICENSE).

- Re-running `scripts/generate_examples.py` produces byte-for-byte identical files.
- The native EPUB fixture has 280 pages at default typography, 28 chapters, internal/external links and working text anchors.
- All 76 Rust tests pass against the replacement fixtures, including reflow remapping, nested outline levels, native pixels and concurrent EPUB typography.
- Portable EPUB smoke with no external tools: passed, covering search, selection, annotations, typography reflow and saved positions.
- Portable multi-document smoke with no external tools: passed, covering tabs, windows, repeated/nested opens and command continuation routing.
- Final installed-build PDF/EPUB theme smoke: passed, including live palette/preference refresh and rounded/square corners.
- Portable PDF quality smoke: passed, covering page-30 native rendering, filtered nested contents and settings.
- The generated 1,220-page PDF passed immediate last-page navigation, background search and annotation. In this run metadata was ready in 0.25 seconds and the first page drew in 0.92 seconds.

The large generated fixture is about 1 MB. These timings do not represent a
scan-heavy or hundreds-of-megabytes document. Earlier measurements on removed
publications are not current fixture benchmarks.

Fixture smoke logs: `/tmp/omapdf-cc0-{epub,multidoc,large,quality,theme}.log`.

## Installed copy and desktop integration

- Portable executable installed at `$HOME/.local/bin/omapdf`.
- `cmp dist/omapdf $HOME/.local/bin/omapdf`: passed.
- Installed `--version`: `Omapdf 0.0.1-alpha`.
- Artifact SHA-256: `5aed8278020b50ecb8ddee85d34e8a6a5c716dc5445bb8ed8315d3ad38b0f69c`.
- Desktop entry: `$HOME/.local/share/applications/io.omapdf.desktop`, with absolute, unquoted executable paths.
- Desktop validation and database refresh: passed.
- Both `xdg-mime` and `gio mime` report `io.omapdf.desktop` as the default PDF handler.
- Existing settings and library copied to `~/.config/omapdf/` and `~/.local/share/omapdf/`; original directories retained. The copied library reflects the migration snapshot.
- Former binary, desktop entry, icons, settings and library snapshot backed up under `$HOME/.local/state/omapdf/backups/20261114T030540843660Z`.
- Running reader processes were left untouched. Reopening uses the new executable; an already-running old session continues using its original data directory.

## Coverage and limits

Before this branding change, the reader had passed a broader 22-script Wayland
regression suite covering selection, region capture, passwords, library migration,
ruler movement, menus, smooth scrolling, search, themes and layouts. That earlier
suite is historical coverage; this change reran the focused checks listed above.

Native annotation interchange handles highlight, underline, strikeout and text/free-text
notes. Other embedded types remain in source/exported PDFs. Legacy JSON imports
do not apply native deletion records. Removing an embedded annotation from the
library does not erase its pixels in the source PDF; export and reopen a new PDF
to view the revised document. Further feature work is tracked in the roadmap.

## Alpha publication packaging

Version `0.0.1-alpha` has passed formatting, Clippy and all 76 native Rust tests.
The architecture-aware portable builder passes local x86-64 packaging and CLI
inspection. The AppImage wraps the same portable executable and passes local
version and EPUB inspection checks without FUSE via extraction mode. A native
Wayland AppImage smoke also passed document opening, bookmark creation, IPC and
clean exit with isolated state and restored focus. Workflow
syntax is checked with actionlint 1.7.12. GitHub CI builds and tests on native
Linux x86-64 and ARM64 runners; tagged versions publish portable executables,
AppImages and SHA-256 files after both builds succeed.

Alpha logs: `/tmp/omapdf-alpha-{clippy,tests,release,portable,appimage,install}.log`.
