# AppImage glibc baseline correction — 2026-11-25

The catalog failure for v0.0.4 comes from Ubuntu 24.04 ABI requirements in the
embedded reader and bundled libxkbcommon, despite the static outer launcher.
Release CI now builds and packages in Ubuntu 22.04 containers on x86-64 and
ARM64, retaining host glibc/GCC/Wayland/Vulkan libraries. Both download formats
are checked against GLIBC_2.35 after extraction. Packaging scripts support
Ubuntu 22.04's Python 3.10 through the distribution's python3-tomli package.

Local verification:

- Formatting and Clippy passed (existing vendored dependency warnings remain).
- All 32 unit tests and 48 integration tests passed; Unix-socket tests required
  execution outside the sandbox.
- Release build and portable packaging passed.
- The updated runtime smoke check catches the existing bundled libxkbcommon's
  GLIBC_2.38 requirement when given `--max-glibc 2.35`.
- Full ELF scanning also exposed an absolute-loader-path variant in newer ldd
  output. Normalizing dependency names fixes its host-library exclusion; the
  rebuilt artifact passes recursive loader/glibc exclusion checks.
- Local AppImage packaging passed with the workflow's checksum-verified
  appimagetool. The resulting AppImage passed host-library checks, full ELF
  scanning with a permissive ceiling (not a GLIBC_2.35 compatibility claim),
  `--version`, and PDF/EPUB inspection.
- Host library resolution and Vulkan enumeration passed (one GPU); GPU access
  required execution outside the sandbox.
- Native PDF and EPUB inspection passed with PATH=/nonexistent and isolated cache.
- The installed portable executable was backed up and atomically updated;
  byte comparison and `--version` passed. No running reader was restarted.

The initial Ubuntu 22.04 CI run exposed a missing `file` utility in the minimal
container after successfully compiling the reader and portable launcher. Adding
that packaging dependency fixed the failure.

[CI run 45471811613](https://github.com/qwlp/omapdf/actions/runs/45471811613)
passed on both architectures: formatting, Clippy, all tests, release/portable
builds, GPU enumeration, native document inspection and AppImage packaging.
The extracted portable and AppImage ELF checks passed the GLIBC_2.35 ceiling.
Downloaded x86-64 artifacts passed SHA-256 verification and the same GLIBC_2.35
checks locally; the portable also enumerated the current host GPU successfully.
This verifies the Ubuntu-built artifact with newer host drivers as well.

[Tagged release CI run 45474103857](https://github.com/qwlp/omapdf/actions/runs/45474103857)
also passed both architecture jobs and published all eight binary/checksum assets
for [v0.0.5](https://github.com/qwlp/omapdf/releases/tag/v0.0.5). The release is
public, stable and selected by the latest-release API. Published x86-64 downloads
passed checksums, GLIBC_2.35 checks and host GPU enumeration. The published portable
was copied to `dist/omapdf` and atomically installed with a backup; byte comparison
and `omapdf --version` confirmed the installed v0.0.5.

Omapdf remains Wayland-only. The catalog's current screenshot workflow uses
X11/Xvfb, which is a separate display-environment limitation once the glibc
loader failure is resolved.

---

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
- Artifact SHA-256: `092752042e2ecf6a0c4566885d2d1ae0c322ab2c48d5c1223f2f07f657b8f672`.
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

## Published-runtime GPU regression

The first Ubuntu-built portable download failed on an Arch/NVIDIA Wayland host
because a driver dlopen selected the host's newer `libdl.so.2` alongside bundled
Ubuntu libc. Vulkan reported `GLIBC_ABI_DT_X86_64_PLT` missing and no supported GPU.
That release added the matching libc compatibility DSOs (`libdl`,
`libpthread`, `librt`, `libutil`, `libresolv`) explicitly. An isolated reproduction
using the exact Ubuntu libc package passed the real Wayland settings smoke after
adding those DSOs. GPU drivers remain host supplied.

The README screenshot shows only Omapdf and an original CC0 sample, captured from
the verified native Wayland build with isolated state and restored focus/cursor.

The corrected tagged CI run passed on both architectures and republished the
prerelease. A fresh x86-64 CI download passed checksum verification and native
Wayland document opening without runtime modifications; the README screenshot
was recaptured from that download. The matching local portable build was backed
up and installed atomically, and its installed `cmp`/version checks passed.

The corrected x86-64 AppImage also passed a fresh native Wayland launch, bookmark
creation, IPC and clean exit. Public release downloads match the tested CI
artifacts byte-for-byte and pass their published SHA-256 checksums.


## 0.0.2 host GPU ABI fix

Supersedes the bundled-glibc workaround above. Host Vulkan ICDs and layers can
require a newer glibc ABI than the build machine provides. The portable builder
now excludes the ELF loader, all glibc companions and GCC runtime libraries;
the launcher uses the host loader so drivers share the host libc/libm/libgcc.
The supported release baseline is glibc-based Ubuntu 24.04 or newer.

- Formatting, Clippy and 76 native tests passed (Unix socket access required).
- Portable regression confirms extracted artifacts contain no loader/glibc/GCC
  DSOs and the launched reader actually initializes the host libc/libm/libgcc.
- Vulkan instance creation and physical-device enumeration passed on NVIDIA.
- Native Wayland portable smoke passed rendering, scrolling, IPC and clean quit;
  desktop focus and cursor restored, isolated settings/library/cache removed.
- Portable PDF/EPUB inspection passed with `PATH=/nonexistent`.
- AMD RADV and Intel hardware were unavailable locally. This removes the reported
  bundled-libc ABI conflict; the separate vague Intel initialization report is
  not confirmed as a shader bug or independently verified as fixed.

### Published 0.0.2 artifacts

The `v0.0.2` GitHub Actions release passed native x86-64 and ARM64 builds,
formatting, Clippy, all tests, portable runtime regression and AppImage checks.
Both downloaded portable executables match their published SHA-256 files.
The Ubuntu-built x86-64 download also passed the host-library/Vulkan probe and
the full native Wayland smoke on this Arch/NVIDIA machine, with isolated state
and restored focus/cursor. The AUR recipe uses these verified release hashes
and explicitly depends on host glibc and GCC libraries. Package assembly and
source verification passed for both architectures; x86-64 package extraction
preserves the executable byte-for-byte and its desktop entry validates.
ARM64 GUI behavior was not tested locally.

The local 0.0.2 portable build was installed with a backup and atomic replacement;
installed artifact comparison and `--version` passed.

## 0.0.3 host Wayland ABI fix

The reported 0.0.2 RADV failure required `wl_display_create_queue_with_name`
and `wl_fixes_interface`, missing from the bundled Wayland client library.
Packaging now excludes all Wayland libraries and the Vulkan loader so host
Mesa ICDs/layers resolve against their system libraries. The extracted-artifact
regression rejects these libraries, and GPU probing checks actual dlopened
library paths. CI also runs Vulkan enumeration using Mesa software drivers.

Local 0.0.3 validation:

- Formatting and Clippy passed (existing vendored GPUI warnings remain).
- All 29 unit tests and 47 integration tests passed with local socket access.
- The updated runtime regression rejects the installed 0.0.2 artifact.
- The rebuilt portable artifact passes host-library checks and Vulkan GPU
  enumeration (one NVIDIA GPU on this machine; AMD user confirmation pending).
- Portable PDF/EPUB inspection and the full Wayland window smoke script passed.
- The portable executable was installed with a backup and atomic replacement;
  byte comparison passed and the installed executable reports Omapdf 0.0.3.

Published 0.0.3 validation:

- Both native GitHub Actions architecture builds and the release job passed,
  including GPU enumeration through Mesa software drivers and AppImage checks.
- Downloaded x86-64 and ARM64 executables/AppImages match their published
  SHA-256 checksum files. The source archive checksum was recorded for AUR.
- The published Ubuntu-built x86-64 executable passes the host-library/GPU
  probe on this desktop, and its AppImage reports Omapdf 0.0.3.
- The updated AUR recipe explicitly depends on host Wayland, passes source
  checksum verification, and builds an x86-64 package successfully.
- The downloaded x86-64 executable also passed the full isolated Wayland
  window smoke script, with focus restored afterward.
- AUR metadata was published as commit `0a67393` for omapdf-bin 0.0.3-1.

## 0.0.4 scrolling and navigation settings

Local x86-64 validation on 2026-11-15:

- Formatting and Clippy checks passed.
- All 31 unit tests and 47 integration tests passed.
- Release reader and portable launcher built as version 0.0.4.
- Real Wayland wheel tests passed for the crisp default and classic glide,
  including exact notch distances, repeated inputs, and reversal.
- Navigation-aid checks passed for separator visibility, persisted toggles,
  vertical scrollbar track clicks and thumb dragging, and horizontal navigation.
- Menu motion checks covered settings, contents, search, reversal, and transient
  notification behavior.
- Portable runtime resolution passed; host Vulkan enumeration found one GPU.
  The GPU probe requires host device access outside the agent sandbox.
- Installed the portable artifact with a backup and atomic replacement; byte
  comparison passed and the installed executable reports Omapdf 0.0.4.

These are local checks. Version-tag publishing and ARM64 CI verification are
separate steps. The AUR recipe remains on the last published, checksum-verified
release until 0.0.4 downloads are available.

## 0.0.4 final local release checks

- All 32 unit tests and 48 integration tests passed; local Unix-socket tests ran
  outside the restricted sandbox.
- Formatting, Clippy, and whitespace checks passed.
- Wayland visual checks verified repeated cached `gg`/`Shift+G` jumps without
  additional input, page/zoom animation, and fitted inline/helper preview pages.
- Real Wayland wheel input passed notch-distance, repeated-input, and reversal
  checks while an inline preview was open.
- The rebuilt portable runtime passed host-library resolution and Vulkan GPU
  enumeration. The installed portable copy matches `dist/omapdf`, reports
  0.0.4, and was replaced atomically after backing up the previous executable.

## 0.0.4 published release and AUR

- Release tag `v0.0.4` points to `64891a7`; all application changes are on main.
- Tagged GitHub Actions run `43720384604` passed x86-64 and ARM64 formatting,
  Clippy, native tests, release/portable builds, host-runtime/GPU probes, and
  AppImage build/CLI checks. Main's corresponding run also passed.
- GitHub release `v0.0.4` is published as the latest stable release, with portable
  executables and AppImages for both architectures and SHA-256 manifests.
- All four public asset downloads match their published SHA-256 checksums.
  The downloaded x86-64 portable executable and AppImage report 0.0.4; the
  portable executable inspects the PDF fixture and passes host-library/GPU
  runtime verification on this machine.
- Updated `omapdf-bin` to 0.0.4-1 using the verified tagged source archive and
  architecture-specific binary hashes. Source validation and a local x86-64
  package build passed; the packaged executable matches the public release
  download and the desktop entry validates.
- AUR update commit: `e25d436`.
