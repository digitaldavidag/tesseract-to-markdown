# Tesseract to Markdown

Dependency-free Rust library and CLI for converting Tesseract TSV layout output
into GitHub-Flavored Markdown.

The converter reconstructs visual lines from word boxes, detects repeated table
geometry, separates adjacent column schemas, preserves sparse leading cells, and
emits GFM tables with numeric alignment. It does not silently correct OCR text.

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
```

`--min-confidence` defaults to zero. Low-confidence word boxes can still carry
useful layout evidence, so raising this value should be an explicit decision.

## Library

```rust
use tesseract_to_markdown::{
    MarkdownOptions,
    tesseract_tsv_to_markdown,
};

let result = tesseract_tsv_to_markdown(
    &tsv,
    &MarkdownOptions::default(),
)?;

println!("{}", result.markdown);
# Ok::<(), Box<dyn std::error::Error>>(())
```

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
