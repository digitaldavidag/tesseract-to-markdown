# Contributing

Run formatting, linting, and tests before submitting changes:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Document fixtures

Do not add real customer, municipal, banking, tax, personal, or otherwise
sensitive documents.

Every new invoice image must be synthetic or demonstrably redistributable and
must be reviewed explicitly before it is committed. Derived OCR files may only
be committed after the source image has been approved.
