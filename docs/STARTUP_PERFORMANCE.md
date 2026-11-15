# Startup performance — 2026-11-15

Measured on this machine (Linux x86-64, Hyprland), comparing the preserved pre-change portable artifact with the rebuilt portable artifact. Seven measured launches per case follow one warm-up; before/after ordering alternates. Settings, library, and runtime/hash caches are isolated. OS page caches and GPU driver caches are not flushed.

| Case | Before median | After median | Reduction |
| --- | ---: | ---: | ---: |
| empty: window | 248.8 ms | 183.6 ms | 26.2% |
| reading-lab.pdf: window | 238.7 ms | 176.0 ms | 26.3% |
| reading-lab.pdf: document-ready command | 330.6 ms | 176.1 ms | 46.7% |
| large-reading-lab.pdf: window | 238.7 ms | 185.3 ms | 22.4% |
| large-reading-lab.pdf: document-ready command | 329.9 ms | 183.7 ms | 44.3% |
| reading-lab.epub: window | 238.2 ms | 172.4 ms | 27.6% |
| reading-lab.epub: document-ready command | 430.8 ms | 168.7 ms | 60.8% |
| Fresh runtime extraction + `--version` | 253.2 ms | 172.4 ms | 31.9% |
| Cached runtime `--version` | 1.4 ms | 1.4 ms | -1.0% |

“Window” means the window appears in Hyprland’s client list. “Document-ready command” means a queued bookmark has been persisted after document preparation. It is not first-page paint latency. The old command path polls every 100 ms; the new path dispatches on load completion. Commands can now finish before compositor registration, so ready time can be slightly earlier than window time.

Fresh extraction creates a new runtime cache for each sample; warm GUI launches reuse their extracted runtime and hash cache. A future update with a different runtime payload will still need one extraction.

The 1,220-page fixture is about 1 MiB, so it tests page-count scaling rather than hashing throughput.

A separate visible-page smoke check captured the large PDF's first drawn page at 0.43 s before and 0.34 s after. This is a single screenshot-based check with polling/capture overhead, not a seven-run median or hardware presentation measurement.

A separate synthetic 128 MiB, one-page PDF isolates full-file hashing. Seven `--inspect` runs measured 78.0 ms before and 5.4 ms after caching (93.1% reduction). The first uncached open of the new build took 77.6 ms. All runs returned exactly the same document ID. The synthetic file has a large unused stream, so these numbers do not represent rendering an image-heavy scan.

## Changes

- Start Vulkan device initialization and system-font discovery on workers, overlapping them with Wayland setup. Join before their resources are used; preserve the full font database and stable font IDs.
- Resolve appearance/host font while GPUI initializes and reuse the initial font selection across tabs. Remove duplicate system/user font-directory entries from the portable fontconfig file.
- Prepare the initial document while the platform initializes. Revalidate its revision and typography before reuse.
- Extract only first-page text for EPUB preview. Visible-page requests, search, and background indexing populate the shared lazy text cache. Reflow migrations index both document versions off the UI thread before moving annotation anchors.
- Cache SHA-256 IDs by canonical path, typography, file size, modification time, device, inode, and ctime. Preserve existing PDF/EPUB identity algorithms. Changed files, invalid caches, and unavailable caches fall back to hashing.
- Dispatch startup commands on loading completion. Keep periodic handling for IPC/settings and asynchronous command continuations.
- Skip the resize debounce when no page raster exists yet.
- Use optimized zlib-rs decompression with runtime CPU detection and a 256 KiB output buffer in the static launcher. Preserve extraction locking, staging/atomic replacement, and license notices.

GPU device/surface creation, driver shader-cache misses, EPUB pagination, first-open hashing, and the first runtime extraction still have real costs. These changes overlap or reduce them; they do not make them disappear.

## Reproduce

```sh
python3 scripts/startup_bench.py \
  --binary before=/path/to/old/omapdf \
  --binary after=dist/omapdf \
  --runs 7 --output /tmp/startup.json
```

Raw samples and artifact SHA-256s: [startup-benchmark.json](startup-benchmark.json). Hash-cache samples: [startup-hash-benchmark.json](startup-hash-benchmark.json).

## Verification

- `cargo fmt --all --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked`: 32 unit tests and 48 integration tests passed. Unix-socket tests need execution outside the restricted sandbox.
- Portable launcher formatting and Clippy passed with its payload/version build variables set.
- Portable runtime smoke with host Vulkan GPU enumeration passed.
- Wayland EPUB, large PDF, reload, and multidocument smoke tests passed on the release reader.
- Final portable password/unlock and EPUB reflow tests passed with `PATH=/nonexistent`; zoom anchoring and large-PDF visible rendering/search/navigation smoke tests also passed.
