use tesseract_to_markdown::{
    MarkdownOptions, TableColumnGeometry, TableRegionModel, analyze_tesseract_tsv,
    tesseract_tsv_to_markdown,
};

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
    assert_eq!(result.stats.unassigned_table_cell_count, 0);
    assert_eq!(result.stats.malformed_row_count, 0);
}

fn table_with_columns(tables: &[TableRegionModel], column_count: usize) -> &TableRegionModel {
    tables
        .iter()
        .find(|table| table.columns.len() == column_count)
        .expect("table with expected column count")
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
    assert_eq!(
        result
            .markdown
            .lines()
            .find(|line| line.starts_with(line_item_header))
            .expect("item table header")
            .matches('|')
            .count(),
        9
    );
}

#[test]
fn table_models_expose_supported_regions_and_provenance() {
    let analysis =
        analyze_tesseract_tsv(TSV, &MarkdownOptions::default()).expect("valid fixture TSV");
    assert_eq!(analysis.tables.len(), 3);
    assert!(
        analysis
            .tables
            .iter()
            .all(|table| table.diagnostics.accepted)
    );

    let metadata = table_with_columns(&analysis.tables, 2);
    let items = table_with_columns(&analysis.tables, 8);
    let footer = table_with_columns(&analysis.tables, 3);

    assert_eq!(metadata.rows.len(), 8);
    assert!(metadata.bbox.left >= 600.0);
    assert_eq!(metadata.diagnostics.excluded_non_table_cell_count, 8);
    assert_eq!(metadata.diagnostics.unassigned_cell_count, 0);
    assert!((metadata.diagnostics.assignment_coverage - 1.0).abs() < f64::EPSILON);
    let metadata_text = metadata
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .map(|cell| cell.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(metadata_text.contains("Rechnung:"));
    assert!(!metadata_text.contains("3 18"));
    assert!(!metadata_text.contains("se ar"));

    assert_eq!(items.rows.len(), 20);
    assert_eq!(items.diagnostics.assigned_cell_count, 137);
    assert_eq!(items.diagnostics.unassigned_cell_count, 0);
    assert!((items.diagnostics.assignment_coverage - 1.0).abs() < f64::EPSILON);
    assert!(items.diagnostics.structural_confidence >= 0.95);
    assert!(items.diagnostics.recognition_confidence < 95.0);
    assert!(items.bbox.top < footer.bbox.top);
    assert!(items.bbox.top + items.bbox.height < footer.bbox.top);
    assert!(
        items.columns[4..].iter().all(|column| {
            column.geometry == TableColumnGeometry::RightAligned && column.numeric
        })
    );

    let low_confidence_unit = items
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .find(|cell| cell.text == "[€]")
        .expect("low-confidence unit qualifier");
    assert_eq!(low_confidence_unit.column_index, Some(7));
    assert!(low_confidence_unit.recognition_confidence < 50.0);
    assert_ne!(
        low_confidence_unit.words,
        [] as [tesseract_to_markdown::TableWordProvenance; 0]
    );
}

#[test]
fn sparse_rows_units_and_recognition_strings_are_preserved() {
    let result =
        tesseract_tsv_to_markdown(TSV, &MarkdownOptions::default()).expect("valid fixture TSV");

    assert!(result.markdown.contains(
        "| Hieb/Los | Holzart | Sorte | Giite | Starke | Menge/Einheit \\[m’/#\\] | Preis/Einheit \\[E/ME\\] | Gesamt \\[€\\] |"
    ));
    assert!(
        result
            .markdown
            .contains("|  | Fi | PZ | B | 2a | 8,672 | 28,00 | 242,82 |")
    );
    assert!(
        result
            .markdown
            .contains("| Ki | Pz | NFK | 2b | 0,238 | 10,00 | 2,38 |")
    );
    assert!(!result.markdown.contains("| 3 18 |"));
    assert!(!result.markdown.contains("| se ar |"));
    assert!(result.markdown.contains("\n3 18 2 y 16\n"));
    assert!(result.markdown.contains("\nse ar\n"));
}

#[test]
fn confidence_filter_does_not_remove_structural_word_boxes() {
    let options = MarkdownOptions {
        min_confidence: 50.0,
        ..MarkdownOptions::default()
    };
    let analysis = analyze_tesseract_tsv(TSV, &options).expect("valid fixture TSV");
    let items = table_with_columns(&analysis.tables, 8);

    assert!(items.diagnostics.accepted);
    assert_eq!(items.diagnostics.unassigned_cell_count, 0);
    let structural_only_unit = items
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .find(|cell| {
            cell.text.is_empty()
                && cell.column_index == Some(7)
                && cell.recognition_confidence < 50.0
        })
        .expect("filtered unit box still supports the final column");
    assert_ne!(
        structural_only_unit.words,
        [] as [tesseract_to_markdown::TableWordProvenance; 0]
    );
}
