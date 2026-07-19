# Tesseract to Markdown

Dependency-free Rust library and CLI for converting Tesseract TSV layout output
into GitHub-Flavored Markdown.

The converter reconstructs visual lines from word boxes, detects repeated table
geometry, separates adjacent column schemas, preserves sparse leading cells, and
emits GFM tables with numeric alignment. It does not silently correct OCR text.

## Install a release

Every successful change on `main` creates a [GitHub release](https://github.com/digitaldavidag/tesseract-to-markdown/releases/latest)
with ready-to-run binaries. Download and extract one of these archives:

- `tesseract-to-markdown-linux-x86_64.tar.gz` for Intel/AMD Linux servers
- `tesseract-to-markdown-linux-aarch64.tar.gz` for ARM64 Linux servers
- `tesseract-to-markdown-macos-universal.tar.gz` for any modern Mac
- `tesseract-to-markdown-macos-aarch64.tar.gz` for Apple Silicon only
- `tesseract-to-markdown-macos-x86_64.tar.gz` for Intel Macs only

The Linux executables are statically linked. The universal macOS executable
contains both Apple Silicon and Intel code and is the recommended Mac download.

Because this repository is private, authenticated command-line downloads use
the GitHub CLI:

```bash
gh release download --repo digitaldavidag/tesseract-to-markdown \
  --pattern 'tesseract-to-markdown-macos-universal.tar.gz'
tar -xzf tesseract-to-markdown-macos-universal.tar.gz
sudo install -m 0755 \
  tesseract-to-markdown-macos-universal/tesseract-to-markdown \
  /usr/local/bin/tesseract-to-markdown
```

Replace the archive pattern and extracted directory with a Linux target when
installing on a server. Each release also includes `SHA256SUMS`.

## CLI

Generate TSV with Tesseract and pipe it into the converter:

```bash
tesseract invoice.png stdout -l deu+eng tsv \
  | cargo run --release -- -
```

Or convert one or more existing TSV files:

```bash
cargo run --release -- invoice-page-1.tsv invoice-page-2.tsv > invoice.md
```

Available options:

```text
--min-confidence N
--table-min-rows N
--page-headings
--stats
-V, --version
```

`--min-confidence` defaults to zero. Low-confidence word boxes can still carry
useful layout evidence, so raising this value should be an explicit decision.

## Library

```rust
use tesseract_to_markdown::{
    MarkdownOptions,
    analyze_tesseract_tsv,
    tesseract_tsv_to_markdown,
};

let analysis = analyze_tesseract_tsv(
    &tsv,
    &MarkdownOptions::default(),
)?;
for table in &analysis.tables {
    eprintln!(
        "table bbox={:?} columns={} coverage={:.3}",
        table.bbox,
        table.columns.len(),
        table.diagnostics.assignment_coverage,
    );
}

let result = tesseract_tsv_to_markdown(
    &tsv,
    &MarkdownOptions::default(),
)?;

println!("{}", result.markdown);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Table analysis is independent from Markdown rendering. Each table model exposes
its page and region bounding box, inferred column geometry, row and cell
provenance, recognition confidence, structural confidence, assignment coverage,
and any cells excluded from or unassigned within the proposed grid. A grid with
unassigned cells inside its table bounding box is rejected and its OCR text
falls back to ordinary line rendering.

## Scope and limitations

- Input is Tesseract TSV with `level=5` word rows.
- Output is GitHub-Flavored Markdown with headings, lists, key/value tables,
  layout tables, and page separators.
- Missing OCR tokens cannot be reconstructed from geometry.
- Recognition strings are preserved rather than guessed or normalized.
- GFM pipe tables cannot represent row or column spans.
- Ambiguous layouts remain heuristic and should be validated against the scan.

## Validation fixture

The repository contains one reviewed, fully synthetic forestry invoice with
handwriting, punch-hole shadows, scan noise, and reverse-side bleed-through.
No real invoice or customer data is included.

See [`fixtures/README.md`](fixtures/README.md) for the fixture-review rule.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
