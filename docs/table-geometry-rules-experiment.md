# Table geometry rules experiment

Fixture:
`fixtures/synthetic-invoice-handwriting-bleedthrough.tsv`

Baseline: `main` at `6eefa77`

Command:

```text
cargo run --quiet -- --stats fixtures/synthetic-invoice-handwriting-bleedthrough.tsv
```

## Objective comparison

| Metric | Baseline | Geometry model |
| --- | ---: | ---: |
| Pages | 1 | 1 |
| Retained words | 228 | 228 |
| Low-confidence words | 6 | 6 |
| Layout lines | 38 | 38 |
| Accepted tables | 3 | 3 |
| Accepted-grid unassigned cells | 1 | 0 |
| Malformed TSV rows | 0 | 0 |
| Metadata columns | 5 | 2 |
| Item columns | 8 | 8 |
| Footer columns | 3 | 3 |
| Recognized item rows | 18 | 18 |

The model now reports three distinct accepted regions with 2, 8, and 3
columns. Each has 100% assignment coverage inside its selected bounding box.
The metadata region excludes eight adjacent address/handwriting cells from its
grid and renders those cells as ordinary OCR text instead of discarding them.

## Readable excerpt

Baseline metadata mixed unrelated geometry into a five-column grid:

```markdown
| Musterforst Falkenried, Beispielweg 12, 12345 Musterstadt |  | Rechnung: | TEST-9001/2026 |  |
|  | A | Steuernummer: | TEST-NICHT-GULTIG |  |
| 3 18 | 2 y 16 | Kassenzeichen: |  | KZ-9001-XY |
```

The geometry model keeps the supported metadata grid separate:

```markdown
| Field | Value |
| --- | --- |
| Rechnung: | TEST-9001/2026 |
| Rechnungsdatum: | 15.07.2026 |
...
| Steuernummer: | TEST-NICHT-GULTIG |
| Kassenzeichen: | KZ-9001-XY |
```

The item table remains eight columns, retains both lot groups and all 18
recognized item rows, and keeps the low-confidence unit qualifier as structural
evidence:

```markdown
| Hieb/Los | Holzart | Sorte | Giite | Starke | Menge/Einheit [m’/#] | Preis/Einheit [E/ME] | Gesamt [€] |
| T-17/4 | Fi | BZ | B | 1b2 | 1,196 | 18,00 | 21,53 |
...
| T-23/4 | Fi | PZ | B | 1b2 | 1,191 | 18,00 | 21,44 |
```

The raw OCR strings `Giite`, `Starke`, `[m’/#]`, `[E/ME]`, `Pz`, and lowercase
`c` remain unchanged.
