# Final catalog validation — 0.0.10

The [AppImage catalog retest passed](https://github.com/AppImage/appimage.github.io/pull/12425#issuecomment-7155145673)
for the public `Omapdf-0.0.10-x86_64.AppImage`.
[Catalog run 45562854378](https://github.com/AppImage/appimage.github.io/actions/runs/45562854378)
completed successfully and captured the main window. Remaining bot suggestions
about default dark mode and update metadata are nonblocking.

[Tagged release CI](https://github.com/qwlp/omapdf/actions/runs/45561635929)
passed both architecture jobs and the independent stock Ubuntu 22.04 runtime
job. It checks all 80 tests, lint/formatting, glibc 2.35 compatibility, and actual
painted window pixels before and after resizing, with Internet sockets denied.

The final fix combines buffered-XCB-event draining with a pinned Jammy-built
Mesa 25.0.7 CPU renderer. Diagnostics ruled out a missing refresh timer in the
remaining Ubuntu failure: the old Mesa renderer submitted five nonempty scene
batches on a 16 ms timer but produced black pixels. Replacing the older software
stack with the Jammy backport produced 361 distinct colors before and after
resizing, also verified with the public release download.

Only AppImages bundle the CPU renderer. The published standalone is 31,978,504
bytes (30.5 MiB), contains no CPU runtime, and uses the host GPU driver. The
AppImage is 82,328,056 bytes (78.5 MiB). All 13 standalone and 41 AppImage embedded
ELF files passed the GLIBC_2.35 ceiling. Hardware GPU preference, native document
processing and host glibc/GCC use are preserved.

The published x86-64 files passed SHA-256 checks. The lean standalone was copied
to `dist/omapdf`, backed up and atomically installed at
`/home/tsp/.local/bin/omapdf`. Byte comparison and `--version` confirmed the
installed Omapdf 0.0.10. No running user reader was restarted.

The following sections retain the investigation history for earlier builds.

---

# Painted X11 windows with the Jammy Mesa backport — 0.0.10

The v0.0.8 catalog test successfully opened a window, but its screenshot was
uniform black. The earlier startup check only verified visibility and survival,
so it did not detect this. It now captures real XWD pixels before and after
resizing to 796x570, rejecting a solid-color window.

Two independent issues were identified:

- Checked XCB requests could buffer map/expose events without leaving socket
  readiness. Draining pending events before blocking and after dispatch fixed
  painting locally with hardware and newer software drivers. The local old build
  had one color; the corrected builds had 372/379 colors before and after resize.
- Ubuntu's stock Mesa 23 software renderer could submit a nonempty scene but
  produce black pixels. CI diagnostics confirmed a 16 ms refresh loop and five
  scene batches, ruling out an absent frame timer. The same older CPU stack
  reproduced the failure locally. Immediate and FIFO presentation modes did not
  fix it. Replacing Mesa/LLVM/DRM with the Jammy-built Kisak stable backport made
  that same test paint 361 colors before and after resize.

Build containers now use pinned `mesa-vulkan-drivers=25.0.7~kisak4~j` from the
signed Kisak stable Jammy repository, with its signing fingerprint checked. Both
architectures are available for Jammy. The isolated CPU libraries are still
built for glibc 2.35; native hardware graphics, glibc and GCC remain host-provided.
AppImages prefer hardware Vulkan devices and select the known-good bundled CPU
renderer instead of an older software-only host driver. The standalone carries
no CPU-renderer files and skips this preflight path.

Formatting, Clippy and all 80 tests passed locally. [Main CI](https://github.com/qwlp/omapdf/actions/runs/45560457941)
passed both architectures, offline host/CPU painted-window checks, resize checks,
GLIBC_2.35 checks and the separate stock Ubuntu 22.04 runtime job. The 0.0.9 tag
was blocked by the stronger paint test and did not publish release assets.

Tagged v0.0.10 verification, installation and catalog retesting are in progress.

---

# AppImage-only CPU fallback — 0.0.8

The independent Ubuntu 22.04 runtime gate caught the second environment gap:
the runner has a Vulkan loader but no physical Vulkan device/driver. It prevented
v0.0.7 from publishing. At the user's request, only the AppImage now includes
Mesa Lavapipe and its private native dependencies; the standalone stays lean.

The shared reader selects the fallback only when the optional AppImage manifest
exists and no host Vulkan device is available. It restarts the bundled reader
before acquiring the primary-instance lock, with a private CPU library path and
only the software ICD selected. It preserves host glibc/GCC in both modes and
never adds the CPU libraries to the normal hardware search path. Existing-reader
commands bypass graphics probing; the IPC test covers this without a display,
host driver or usable CPU runtime. The restart uses the bundled reader path
because current_exe() may identify the explicit ELF loader in portable mode.

- Separate packaging outputs: `dist/omapdf` (lean) and `dist/omapdf-appimage`
  (CPU payload used only by the AppImage wrapper). The runtime smoke asserts the
  CPU stack is absent from the standalone and present in the AppImage.
- Formatting, Clippy and all 80 tests passed on both architectures.
- [Main CI](https://github.com/qwlp/omapdf/actions/runs/45552024307) and
  [tagged release CI](https://github.com/qwlp/omapdf/actions/runs/45553405357)
  passed normal offline windows, forced driverless CPU windows, and a separate
  Ubuntu 22.04 runtime host without application development dependencies.
- All 13 standalone and 41 AppImage embedded ELF files passed GLIBC_2.35 checks.
  No glibc/GCC libraries are bundled even in the private CPU stack.
- Downloaded Ubuntu-built artifacts passed checksums and current host Vulkan
  enumeration. The standalone is 31 MiB rounded up; the AppImage is 79 MiB
  rounded up. Local Arch-built CPU payloads are larger due to newer LLVM.
- The final lean executable passed the full native Wayland interaction smoke.
  An earlier boundary screenshot check was transiently unstable; the lean
  build's full rerun passed without changing scrolling code.
- The Ubuntu-built AppImage CPU runtime passed native Wayland window startup,
  native PDF loading, startup commands, saved bookmarks, IPC forwarding and
  clean quit on the current desktop. Temporary settings/library/cache files
  were used and prior focus was restored.

[v0.0.8](https://github.com/qwlp/omapdf/releases/tag/v0.0.8) is public and stable.
[Catalog retesting](https://github.com/AppImage/appimage.github.io/pull/12425#issuecomment-7153891724)
is in progress. Installation and final catalog result will be recorded below.

---

# XCB keyboard runtime correction — 0.0.7

The actual v0.0.6 catalog retest found `libxcb-xkb.so.1` missing on the runner.
The X11 backend worked, but excluding every XCB library had also excluded its
keyboard extension. Build containers had this library through development
packages, hiding the runtime dependency.

The keyboard extension is now bundled with libxkbcommon-x11. Core X11/XCB,
authentication, BSD/MD support and GPU libraries remain on the host. The runtime
smoke explicitly verifies that both keyboard libraries initialize from the
extracted bundle. A separate Ubuntu 22.04 runner installs GUI test tools only
and checks downloaded artifacts without installing application build dependencies;
publishing tagged releases requires this job to pass.

Local formatting, Clippy, all 80 tests, release/portable builds, host Vulkan
initialization, bundled keyboard-library resolution and offline Xvfb startup
passed. Final release verification and catalog retesting are in progress.

---

# X11 catalog startup correction — 0.0.6

The v0.0.5 catalog log shows a panic opening the reader window with the headless
backend. Its runner provides DISPLAY through Xvfb, but GPUI's X11 feature was
disabled, so DISPLAY was ignored. This is separate from the resolved glibc issue.

- Enable GPUI's native X11 backend; its existing selection still prefers Wayland
  whenever WAYLAND_DISPLAY is present.
- Bundle libxkbcommon-x11 with libxkbcommon, while keeping X11/XCB libraries on
  the host to match its Vulkan presentation stack.
- Add an Xvfb smoke check for both download formats. It denies IPv4/IPv6 socket
  creation with seccomp before executing the reader, checks for a visible window
  owned by the reader or an AppImage child, and verifies 12 seconds of survival.
  Settings, library and caches are temporary; cleanup targets only test processes.
- Local formatting, Clippy, 32 unit tests and 48 integration tests passed.
- Local release and portable/AppImage packaging passed.
- Both the portable and AppImage passed real offline X11 window startup under
  isolated Xvfb using the host GPU driver.
- The existing native Wayland interaction smoke passed, including PDF loading,
  prompt input, IPC, search, highlights, settings persistence, scrolling and quit.
  It restored prior focus and did not restart the user's reader.

[Feature CI](https://github.com/qwlp/omapdf/actions/runs/45546982582) passed
on x86-64 and ARM64, including both offline window checks. The extracted runtime
inspection then identified additional transitive X11 authentication/BSD/MD
libraries to keep on the host, avoiding driver-library shadowing.

[Tagged release CI](https://github.com/qwlp/omapdf/actions/runs/45548163706)
passed the final exclusions, all tests and both offline window checks on both
architectures, then published [v0.0.6](https://github.com/qwlp/omapdf/releases/tag/v0.0.6).
Public x86-64 downloads passed SHA-256 checks, the GLIBC_2.35 ceiling for all 12
embedded ELF files, and host Vulkan enumeration. Catalog retesting was requested
in [PR #12425](https://github.com/AppImage/appimage.github.io/pull/12425#issuecomment-7153131061).

---

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
