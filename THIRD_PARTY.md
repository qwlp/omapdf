# Third-party components

The project source retains its GPL-3.0-or-later license. The combined executable links MuPDF 0.8.0 / its bundled MuPDF C engine, licensed AGPL-3.0. The combined distribution must meet the applicable GPL/AGPL source-availability and notice requirements. The AGPL text is in `assets/AGPL-3.0.txt`; MuPDF's original source and dependency notices are distributed with its pinned crates and upstream repository: https://github.com/messense/mupdf-rs/tree/v0.8.0 .

MuPDF and its enabled document libraries are statically linked. Bundled URW base14 and Droid fallback fonts retain their upstream notices. The portable build collects MuPDF third-party license files into its runtime `licenses/mupdf` directory.

GPUI 0.2.2 retains its Apache-2.0 license. Its source is vendored under `vendor/gpui` with a native Wayland image-clipboard patch; the original license and patch provenance are included under `licenses/gpui` in the portable runtime. JetBrainsMono Nerd Font is bundled under the SIL Open Font License (`assets/fonts/OFL.txt`). Noto Sans regular, medium, and bold are bundled under the SIL Open Font License (`assets/fonts/Noto-OFL.txt`). Icons retain their Lucide/Feather notices (`assets/icons/LICENSE`). `vendor/xattr` retains its original MIT/Apache notices.

The portable executable includes the userspace libraries listed in `dist/runtime-manifest.txt`, with their available system-package notices in `licenses/runtime`. They are loaded from the private extracted runtime. The ELF loader, glibc libraries, GCC runtime, hardware-specific GPU drivers and Wayland compositor are supplied by the host and are not redistributed.

Library migration statically links SQLite through `rusqlite` 0.32.1 and
`libsqlite3-sys` 0.30.1 with the bundled feature. The Rust bindings retain their
MIT license; SQLite is in the public domain (see its bundled amalgamation
notices). No host SQLite library or `sqlite3` command is required. MD5 is used
only to identify legacy documents, alongside the reader’s SHA-256
identity. RustCrypto `md-5` retains its MIT/Apache-2.0 notices, and Chrono retains
its MIT/Apache-2.0 notices. The portable builder collects available license and
notice texts from locked Rust dependencies under `licenses/rust`.

Omapdf originated as a rewrite of ahrm/Sioyek. Preference values, key notation and
command help were adapted from that GPL-3.0-or-later project. Upstream source and
authorship: https://github.com/ahrm/sioyek . This attribution is retained independently
of Omapdf’s application identity.

AppImage releases additionally use the AppImage type-2 runtime. Its MIT notice and
third-party library notices are retained in `assets/APPIMAGE-RUNTIME-LICENSE` and
inside the AppImage. Runtime source: https://github.com/AppImage/type2-runtime .
The AppImage wraps the same portable reader and its bundled license collection.
