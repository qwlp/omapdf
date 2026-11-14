# Original reading fixtures

All bundled PDF/EPUB sample content is original, generated for Omapdf and dedicated
to the public domain under [CC0-1.0](LICENSE). No third-party books, photographs,
font files or downloaded text are embedded in these documents. PDF text uses the
standard Helvetica font reference; the reader supplies its rendering font.

| File | Purpose |
| --- | --- |
| `reading-lab.pdf` | 32 pages of text, vector graphics and three-level nested contents |
| `large-reading-lab.pdf` | 1,220 pages for lazy loading, background indexing and navigation |
| `color-chart.pdf` | Four image-only pages with an original generated geometric color chart |
| `reading-lab.epub` | 28 original chapters, internal/external links, basic styling and reflow anchors |

Regenerate every document deterministically using only Python's standard library:

```sh
python3 scripts/generate_examples.py
```

The large fixture exercises page-count and indexing behavior. Its compressed size
is about 1 MB; it does not simulate a scan-heavy document hundreds of MB in size.

The Rust programs in this directory are development utilities covered by the
project source license. For example:

```sh
cargo run --release --example render_bench -- examples/reading-lab.pdf
cargo run --release --example large_open_bench -- examples/large-reading-lab.pdf
cargo run --release --example book_bench -- examples/reading-lab.epub
```
