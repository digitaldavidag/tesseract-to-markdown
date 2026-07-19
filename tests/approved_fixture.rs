use tesseract_to_markdown::{MarkdownOptions, tesseract_tsv_to_markdown};

const TSV: &str = include_str!("../fixtures/synthetic-invoice-handwriting-bleedthrough.tsv");
const EXPECTED: &str =
    include_str!("../fixtures/synthetic-invoice-handwriting-bleedthrough.expected.md");

#[test]
fn reviewed_synthetic_invoice_matches_markdown_snapshot() {
    let result =
        tesseract_tsv_to_markdown(TSV, &MarkdownOptions::default()).expect("valid fixture TSV");

    assert_eq!(result.markdown, EXPECTED);
    assert_eq!(result.stats.page_count, 1);
    assert_eq!(result.stats.table_count, 3);
    assert_eq!(result.stats.unassigned_table_cell_count, 1);
    assert_eq!(result.stats.malformed_row_count, 0);
}

#[test]
fn line_item_grid_remains_separate_from_footer_schema() {
    let result =
        tesseract_tsv_to_markdown(TSV, &MarkdownOptions::default()).expect("valid fixture TSV");

    let line_item_header = "| Hieb/Los | Holzart | Sorte | Giite | Starke | Menge/Einheit";
    let first_lot = "| T-17/4 | Fi | BZ | B | 1b2 | 1,196 | 18,00 | 21,53 |";
    let second_lot = "| T-23/4 | Fi | PZ | B | 1b2 | 1,191 | 18,00 | 21,44 |";
    let footer = "| Beispielweg 12 | Testbank A | Testbank B |";

    assert!(result.markdown.contains(line_item_header));
    assert!(result.markdown.contains(first_lot));
    assert!(result.markdown.contains(second_lot));
    assert!(result.markdown.contains(footer));

    let item_table = result
        .markdown
        .split(footer)
        .next()
        .expect("content before footer");
    assert!(!item_table.contains("IBAN"));
}
