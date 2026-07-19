//! Convert Tesseract TSV layout output into GitHub-Flavored Markdown.
//!
//! The crate deliberately separates TSV parsing, layout reconstruction, table
//! modeling, and Markdown rendering. OCR recognition strings are preserved.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fmt;

const TSV_COLUMNS: [&str; 12] = [
    "level",
    "page_num",
    "block_num",
    "par_num",
    "line_num",
    "word_num",
    "left",
    "top",
    "width",
    "height",
    "conf",
    "text",
];

#[derive(Clone, Debug, PartialEq)]
pub struct TesseractBox {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl TesseractBox {
    fn right(&self) -> f64 {
        self.left + self.width
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TesseractWord {
    pub page_number: u32,
    pub block_number: u32,
    pub paragraph_number: u32,
    pub line_number: u32,
    pub word_number: u32,
    pub confidence: f64,
    pub text: String,
    pub bbox: TesseractBox,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TesseractPage {
    pub page_number: u32,
    pub width: f64,
    pub height: f64,
    pub words: Vec<TesseractWord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TesseractTsvParseResult {
    pub pages: Vec<TesseractPage>,
    pub malformed_row_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownOptions {
    /// Retain words at or above this Tesseract confidence.
    pub min_confidence: f64,
    /// Minimum adjacent layout rows required before emitting a GFM table.
    pub table_min_rows: usize,
    /// Override automatic page headings. `None` enables them for multi-page input.
    pub include_page_headings: Option<bool>,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            min_confidence: 0.0,
            table_min_rows: 3,
            include_page_headings: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownStats {
    pub page_count: usize,
    pub word_count: usize,
    pub low_confidence_word_count: usize,
    pub line_count: usize,
    pub table_count: usize,
    pub unassigned_table_cell_count: usize,
    pub malformed_row_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownResult {
    pub markdown: String,
    pub stats: MarkdownStats,
}

/// Horizontal evidence used to infer a table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableColumnGeometry {
    LeftAligned,
    RightAligned,
}

/// Stable reference back to one Tesseract `level=5` word row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableWordProvenance {
    pub block_number: u32,
    pub paragraph_number: u32,
    pub line_number: u32,
    pub word_number: u32,
}

/// One OCR cell retained in an inferred table row.
#[derive(Clone, Debug, PartialEq)]
pub struct TableCellModel {
    pub text: String,
    pub bbox: TesseractBox,
    pub column_index: Option<usize>,
    pub recognition_confidence: f64,
    pub words: Vec<TableWordProvenance>,
}

/// One visual row in an inferred table region.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRowModel {
    pub bbox: TesseractBox,
    pub cells: Vec<TableCellModel>,
}

/// Geometry and rendering alignment for an inferred table column.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumnModel {
    pub anchor: f64,
    pub geometry: TableColumnGeometry,
    pub numeric: bool,
}

/// Objective evidence supporting or rejecting a proposed table grid.
#[derive(Clone, Debug, PartialEq)]
pub struct TableDiagnostics {
    pub accepted: bool,
    pub assigned_cell_count: usize,
    pub unassigned_cell_count: usize,
    pub excluded_non_table_cell_count: usize,
    pub assignment_coverage: f64,
    pub structural_confidence: f64,
    pub recognition_confidence: f64,
    pub notes: Vec<String>,
}

/// Inspectable table-region model produced before Markdown rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct TableRegionModel {
    pub page_number: u32,
    pub bbox: TesseractBox,
    pub columns: Vec<TableColumnModel>,
    pub rows: Vec<TableRowModel>,
    pub diagnostics: TableDiagnostics,
}

/// Layout analysis produced independently from Markdown rendering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TesseractLayoutAnalysis {
    pub tables: Vec<TableRegionModel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TsvError {
    MissingColumns(Vec<String>),
}

impl fmt::Display for TsvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingColumns(columns) => {
                write!(
                    formatter,
                    "invalid Tesseract TSV: missing columns {}",
                    columns.join(", ")
                )
            }
        }
    }
}

impl Error for TsvError {}

#[derive(Clone, Debug)]
struct LayoutCell {
    text: String,
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    confidence: f64,
    words: Vec<TesseractWord>,
}

#[derive(Clone, Debug)]
struct LayoutLine {
    words: Vec<TesseractWord>,
    cells: Vec<LayoutCell>,
    text: String,
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    height: f64,
    confidence: f64,
}

#[derive(Clone, Debug)]
struct TableGrid {
    headers: Vec<String>,
    alignments: Vec<ColumnAlignment>,
    rows: Vec<Vec<String>>,
    unassigned_cell_count: usize,
    model: TableRegionModel,
    cell_assignments: Vec<Vec<Option<usize>>>,
}

#[derive(Clone, Copy, Debug)]
enum ColumnAlignment {
    Left,
    Right,
}

#[derive(Clone, Debug)]
struct TableRun {
    from: usize,
    to: usize,
    grid: TableGrid,
}

struct AnchorCluster {
    sum: f64,
    count: usize,
    rows: HashSet<usize>,
    cells: HashSet<(usize, usize)>,
    deviation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Left,
    Right,
}

struct EdgeCluster {
    anchor: AnchorCluster,
    edge: Edge,
}

#[derive(Clone, Copy)]
struct ColumnGeometry {
    anchor: f64,
    edge: Edge,
    center: f64,
}

fn median(mut values: Vec<f64>, fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        values[middle - 1].midpoint(values[middle])
    }
}

fn word_right(word: &TesseractWord) -> f64 {
    word.bbox.left + word.bbox.width
}

fn word_bottom(word: &TesseractWord) -> f64 {
    word.bbox.top + word.bbox.height
}

fn word_center_y(word: &TesseractWord) -> f64 {
    word.bbox.top + word.bbox.height / 2.0
}

fn parse_f64(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn parse_u32(value: &str) -> Option<u32> {
    value.parse::<u32>().ok()
}

/// Parse Tesseract TSV, retaining page dimensions and `level=5` word rows.
///
/// # Errors
///
/// Returns [`TsvError::MissingColumns`] when the TSV header is incomplete.
pub fn parse_tesseract_tsv(tsv: &str) -> Result<TesseractTsvParseResult, TsvError> {
    let tsv = tsv.strip_prefix('\u{feff}').unwrap_or(tsv);
    let rows: Vec<&str> = tsv.lines().collect();
    let Some(header_index) = rows.iter().position(|row| !row.trim().is_empty()) else {
        return Ok(TesseractTsvParseResult {
            pages: Vec::new(),
            malformed_row_count: 0,
        });
    };

    let header: Vec<&str> = rows[header_index].split('\t').map(str::trim).collect();
    let indexes: HashMap<&str, usize> = header
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index))
        .collect();
    let missing: Vec<String> = TSV_COLUMNS
        .iter()
        .filter(|column| !indexes.contains_key(**column))
        .map(ToString::to_string)
        .collect();
    if !missing.is_empty() {
        return Err(TsvError::MissingColumns(missing));
    }

    let mut pages = BTreeMap::<u32, TesseractPage>::new();
    let mut malformed_row_count = 0;

    for row in rows.iter().skip(header_index + 1) {
        if row.trim().is_empty() {
            continue;
        }
        let columns: Vec<&str> = row.split('\t').collect();
        let field = |name: &str| -> &str {
            indexes
                .get(name)
                .and_then(|index| columns.get(*index))
                .copied()
                .unwrap_or("")
        };

        let Some(level) = parse_u32(field("level")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(page_number) = parse_u32(field("page_num")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(left) = parse_f64(field("left")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(top) = parse_f64(field("top")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(width) = parse_f64(field("width")).filter(|value| *value >= 0.0) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(height) = parse_f64(field("height")).filter(|value| *value >= 0.0) else {
            malformed_row_count += 1;
            continue;
        };
        if page_number == 0 {
            malformed_row_count += 1;
            continue;
        }

        let page = pages.entry(page_number).or_insert_with(|| TesseractPage {
            page_number,
            width: 0.0,
            height: 0.0,
            words: Vec::new(),
        });

        if level == 1 {
            page.width = page.width.max(left + width);
            page.height = page.height.max(top + height);
            continue;
        }
        if level != 5 {
            continue;
        }

        let text = field("text").to_owned();
        if text.trim().is_empty() {
            continue;
        }

        let Some(block_number) = parse_u32(field("block_num")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(paragraph_number) = parse_u32(field("par_num")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(line_number) = parse_u32(field("line_num")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(word_number) = parse_u32(field("word_num")) else {
            malformed_row_count += 1;
            continue;
        };
        let Some(confidence) = parse_f64(field("conf")) else {
            malformed_row_count += 1;
            continue;
        };

        page.words.push(TesseractWord {
            page_number,
            block_number,
            paragraph_number,
            line_number,
            word_number,
            confidence,
            text,
            bbox: TesseractBox {
                left,
                top,
                width,
                height,
            },
        });
        page.width = page.width.max(left + width);
        page.height = page.height.max(top + height);
    }

    Ok(TesseractTsvParseResult {
        pages: pages.into_values().collect(),
        malformed_row_count,
    })
}

fn is_obvious_graphic_artifact(word: &TesseractWord, body_height: f64) -> bool {
    let compact: String = word
        .text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if word.confidence < 50.0 && word.bbox.height > body_height * 2.5 {
        return true;
    }
    let alpha_numeric_length = compact
        .chars()
        .filter(|character| character.is_alphanumeric())
        .count();
    if alpha_numeric_length > 1 || word.confidence >= 60.0 {
        return false;
    }
    word.bbox.height > body_height * 2.5
        || word.bbox.width < (body_height * 0.15).max(3.0)
        || word.bbox.height < (body_height * 0.15).max(3.0)
}

fn line_overlap(line: &LayoutLine, word: &TesseractWord) -> f64 {
    let reference_height = median(
        line.words.iter().map(|item| item.bbox.height).collect(),
        line.height,
    );
    let reference_center = median(
        line.words.iter().map(word_center_y).collect(),
        line.top.midpoint(line.bottom),
    );
    let reference_top = reference_center - reference_height / 2.0;
    let reference_bottom = reference_center + reference_height / 2.0;
    let overlap = reference_bottom.min(word_bottom(word)) - reference_top.max(word.bbox.top);
    overlap.max(0.0) / line.height.min(word.bbox.height).max(1.0)
}

fn words_to_text(words: &[TesseractWord]) -> String {
    let mut result = String::new();
    for word in words {
        if result.is_empty() {
            result.push_str(&word.text);
            continue;
        }
        let no_space_before = word
            .text
            .chars()
            .next()
            .is_some_and(|character| ",.;:!?%)}]".contains(character));
        let no_space_after_previous = result
            .chars()
            .next_back()
            .is_some_and(|character| "([/{".contains(character));
        if !no_space_before && !no_space_after_previous {
            result.push(' ');
        }
        result.push_str(&word.text);
    }
    result.trim().to_owned()
}

fn split_cells(words: &[TesseractWord], page_width: f64, min_confidence: f64) -> Vec<LayoutCell> {
    if words.is_empty() {
        return Vec::new();
    }
    let mut ordered = words.to_vec();
    ordered.sort_by(|left, right| {
        left.bbox
            .left
            .total_cmp(&right.bbox.left)
            .then(left.word_number.cmp(&right.word_number))
    });
    let word_height = median(ordered.iter().map(|word| word.bbox.height).collect(), 16.0);
    let gap_threshold = 20.0_f64.max(word_height * 1.35).max(page_width * 0.012);
    let mut groups: Vec<Vec<TesseractWord>> = vec![Vec::new()];

    for word in ordered {
        let starts_new_cell = groups
            .last()
            .and_then(|group| group.last())
            .is_some_and(|previous| word.bbox.left - word_right(previous) >= gap_threshold);
        if starts_new_cell {
            groups.push(Vec::new());
        }
        groups
            .last_mut()
            .expect("cell groups always contain one group")
            .push(word);
    }

    groups
        .into_iter()
        .map(|group| {
            let left = group
                .iter()
                .map(|word| word.bbox.left)
                .fold(f64::INFINITY, f64::min);
            let right = group
                .iter()
                .map(word_right)
                .fold(f64::NEG_INFINITY, f64::max);
            let top = group
                .iter()
                .map(|word| word.bbox.top)
                .fold(f64::INFINITY, f64::min);
            let bottom = group
                .iter()
                .map(word_bottom)
                .fold(f64::NEG_INFINITY, f64::max);
            let total_width: f64 = group.iter().map(|word| word.bbox.width).sum();
            let confidence = if total_width > 0.0 {
                group
                    .iter()
                    .map(|word| word.confidence * word.bbox.width)
                    .sum::<f64>()
                    / total_width
            } else {
                median(group.iter().map(|word| word.confidence).collect(), 0.0)
            };
            LayoutCell {
                left,
                right,
                top,
                bottom,
                confidence,
                text: words_to_text(
                    &group
                        .iter()
                        .filter(|word| word.confidence >= min_confidence)
                        .cloned()
                        .collect::<Vec<_>>(),
                ),
                words: group,
            }
        })
        .collect()
}

fn build_layout_lines(
    page: &TesseractPage,
    min_confidence: f64,
) -> (Vec<LayoutLine>, Vec<TesseractWord>) {
    let body_height = median(
        page.words.iter().map(|word| word.bbox.height).collect(),
        16.0,
    );
    let structural_words: Vec<TesseractWord> = page
        .words
        .iter()
        .filter(|word| !is_obvious_graphic_artifact(word, body_height))
        .cloned()
        .collect();
    let retained_words: Vec<TesseractWord> = structural_words
        .iter()
        .filter(|word| word.confidence >= min_confidence)
        .cloned()
        .collect();
    let mut ordered = structural_words;
    ordered.sort_by(|left, right| {
        word_center_y(left)
            .total_cmp(&word_center_y(right))
            .then(left.bbox.left.total_cmp(&right.bbox.left))
    });
    let mut lines: Vec<LayoutLine> = Vec::new();

    for word in ordered {
        let mut best_index = None;
        let mut best_score = f64::NEG_INFINITY;
        for index in (0..lines.len()).rev() {
            let line = &lines[index];
            if line.top > word_bottom(&word) + body_height {
                continue;
            }
            if line.bottom < word.bbox.top - body_height {
                break;
            }
            let overlap = line_overlap(line, &word);
            let reference_center = median(
                line.words.iter().map(word_center_y).collect(),
                line.top.midpoint(line.bottom),
            );
            let distance = (reference_center - word_center_y(&word)).abs();
            let tolerance = (line.height.min(word.bbox.height) * 0.55).max(2.0);
            if overlap < 0.35 && distance > tolerance {
                continue;
            }
            let score = overlap * 10.0 - distance / body_height.max(1.0);
            if score > best_score {
                best_index = Some(index);
                best_score = score;
            }
        }

        if let Some(index) = best_index {
            let line = &mut lines[index];
            line.left = line.left.min(word.bbox.left);
            line.right = line.right.max(word_right(&word));
            line.top = line.top.min(word.bbox.top);
            line.bottom = line.bottom.max(word_bottom(&word));
            line.words.push(word);
            line.height = median(
                line.words.iter().map(|item| item.bbox.height).collect(),
                line.height,
            );
        } else {
            lines.push(LayoutLine {
                left: word.bbox.left,
                right: word_right(&word),
                top: word.bbox.top,
                bottom: word_bottom(&word),
                height: word.bbox.height,
                confidence: word.confidence,
                text: if word.confidence >= min_confidence {
                    word.text.clone()
                } else {
                    String::new()
                },
                cells: Vec::new(),
                words: vec![word],
            });
        }
    }

    for line in &mut lines {
        line.words.sort_by(|left, right| {
            left.bbox
                .left
                .total_cmp(&right.bbox.left)
                .then(left.word_number.cmp(&right.word_number))
        });
        line.cells = split_cells(&line.words, page.width, min_confidence);
        line.text = words_to_text(
            &line
                .words
                .iter()
                .filter(|word| word.confidence >= min_confidence)
                .cloned()
                .collect::<Vec<_>>(),
        );
        line.top = median(
            line.words.iter().map(|word| word.bbox.top).collect(),
            line.top,
        );
        line.bottom = median(line.words.iter().map(word_bottom).collect(), line.bottom);
        line.height = median(
            line.words.iter().map(|word| word.bbox.height).collect(),
            line.height,
        );
        let total_width: f64 = line.words.iter().map(|word| word.bbox.width).sum();
        line.confidence = if total_width > 0.0 {
            line.words
                .iter()
                .map(|word| word.confidence * word.bbox.width)
                .sum::<f64>()
                / total_width
        } else {
            median(line.words.iter().map(|word| word.confidence).collect(), 0.0)
        };
    }
    lines.sort_by(|left, right| {
        left.top
            .total_cmp(&right.top)
            .then(left.left.total_cmp(&right.left))
    });
    (lines, retained_words)
}

fn looks_numeric(value: &str) -> bool {
    let normalized = value
        .replace('\u{00a0}', " ")
        .replace(['€', '$', '£', '¥'], "")
        .replace("EUR", "")
        .replace("USD", "")
        .replace("GBP", "")
        .replace("m³", "")
        .replace("m3", "")
        .replace("Stck", "")
        .replace("Stk", "")
        .replace("Fm", "")
        .replace("fm", "");
    let text = normalized.trim();
    if text.is_empty() {
        return false;
    }
    let trimmed = text.trim_start_matches(['-', '+', '(', ')']);
    let mut characters = trimmed.chars();
    if !characters
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        return false;
    }
    trimmed.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_whitespace()
            || ".,/'’-%()+".contains(character)
            || character.is_ascii_alphabetic()
    })
}

fn is_table_candidate(line: &LayoutLine, page_width: f64) -> bool {
    if line.cells.len() >= 3 {
        return true;
    }
    if line.cells.len() != 2 {
        return false;
    }
    let first = &line.cells[0];
    let second = &line.cells[1];
    first.text.ends_with(':')
        || looks_numeric(&first.text)
        || looks_numeric(&second.text)
        || second.left - first.right > page_width * 0.12
}

fn cluster_column_anchors(lines: &[LayoutLine], page_width: f64) -> Vec<f64> {
    let body_height = median(lines.iter().map(|line| line.height).collect(), 16.0);
    let tolerance = 18.0_f64.max(body_height * 1.4).max(page_width * 0.035);
    let mut points: Vec<(f64, usize)> = lines
        .iter()
        .enumerate()
        .flat_map(|(row, line)| line.cells.iter().map(move |cell| (cell.left, row)))
        .collect();
    points.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut clusters: Vec<AnchorCluster> = Vec::new();
    for (x, row) in points {
        let nearest = clusters
            .iter()
            .enumerate()
            .filter_map(|(index, cluster)| {
                let distance = (cluster.sum / cluster.count as f64 - x).abs();
                (distance <= tolerance).then_some((index, distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index);

        if let Some(index) = nearest {
            let cluster = &mut clusters[index];
            cluster.sum += x;
            cluster.count += 1;
            cluster.rows.insert(row);
        } else {
            clusters.push(AnchorCluster {
                sum: x,
                count: 1,
                rows: HashSet::from([row]),
                cells: HashSet::new(),
                deviation: 0.0,
            });
        }
    }

    let repeated: Vec<&AnchorCluster> = clusters
        .iter()
        .filter(|cluster| cluster.rows.len() >= 2)
        .collect();
    let selected: Vec<&AnchorCluster> = if repeated.len() >= 2 {
        repeated
    } else {
        clusters.iter().collect()
    };
    let mut anchors: Vec<f64> = selected
        .into_iter()
        .map(|cluster| cluster.sum / cluster.count as f64)
        .collect();
    anchors.sort_by(f64::total_cmp);
    anchors
}

fn cluster_column_edges(lines: &[LayoutLine], page_width: f64) -> Vec<EdgeCluster> {
    let body_height = median(lines.iter().map(|line| line.height).collect(), 16.0);
    let tolerance = 12.0_f64.max(body_height).max(page_width * 0.012);
    let mut result = Vec::new();

    for edge in [Edge::Left, Edge::Right] {
        let mut points: Vec<(f64, usize, usize)> = lines
            .iter()
            .enumerate()
            .flat_map(|(row, line)| {
                line.cells.iter().enumerate().map(move |(cell, value)| {
                    (
                        if edge == Edge::Left {
                            value.left
                        } else {
                            value.right
                        },
                        row,
                        cell,
                    )
                })
            })
            .collect();
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        let mut clusters: Vec<AnchorCluster> = Vec::new();

        for (x, row, cell) in points {
            let nearest = clusters
                .iter()
                .enumerate()
                .filter_map(|(index, cluster)| {
                    let distance = (cluster.sum / cluster.count as f64 - x).abs();
                    (distance <= tolerance).then_some((index, distance))
                })
                .min_by(|left, right| left.1.total_cmp(&right.1));
            if let Some((index, distance)) = nearest {
                let cluster = &mut clusters[index];
                cluster.sum += x;
                cluster.count += 1;
                cluster.rows.insert(row);
                cluster.cells.insert((row, cell));
                cluster.deviation += distance;
            } else {
                clusters.push(AnchorCluster {
                    sum: x,
                    count: 1,
                    rows: HashSet::from([row]),
                    cells: HashSet::from([(row, cell)]),
                    deviation: 0.0,
                });
            }
        }
        result.extend(
            clusters
                .into_iter()
                .map(|anchor| EdgeCluster { anchor, edge }),
        );
    }
    result
}

fn cell_at(lines: &[LayoutLine], location: (usize, usize)) -> &LayoutCell {
    &lines[location.0].cells[location.1]
}

fn shared_cell_ratio(left: &EdgeCluster, right: &EdgeCluster) -> f64 {
    let shared = left.anchor.cells.intersection(&right.anchor.cells).count();
    shared as f64 / left.anchor.cells.len().min(right.anchor.cells.len()).max(1) as f64
}

fn infer_column_geometry(lines: &[LayoutLine], page_width: f64) -> Vec<ColumnGeometry> {
    let support = (lines.len() * 7).div_ceil(10).max(2);
    let candidates: Vec<EdgeCluster> = cluster_column_edges(lines, page_width)
        .into_iter()
        .filter(|cluster| cluster.anchor.rows.len() >= support)
        .collect();
    let mut consumed = HashSet::new();
    let mut columns = Vec::new();

    for index in 0..candidates.len() {
        if consumed.contains(&index) {
            continue;
        }
        let mut group = vec![index];
        consumed.insert(index);
        let mut cursor = 0;
        while cursor < group.len() {
            let member = group[cursor];
            for other in 0..candidates.len() {
                if !consumed.contains(&other)
                    && shared_cell_ratio(&candidates[member], &candidates[other]) >= 0.65
                {
                    consumed.insert(other);
                    group.push(other);
                }
            }
            cursor += 1;
        }

        let cells: HashSet<(usize, usize)> = group
            .iter()
            .flat_map(|candidate| candidates[*candidate].anchor.cells.iter().copied())
            .collect();
        let numeric_ratio = cells
            .iter()
            .filter(|location| looks_numeric(&cell_at(lines, **location).text))
            .count() as f64
            / cells.len().max(1) as f64;
        let preferred_edge = if numeric_ratio >= 0.6 {
            Edge::Right
        } else {
            Edge::Left
        };
        let best = group
            .iter()
            .map(|candidate| &candidates[*candidate])
            .filter(|candidate| candidate.edge == preferred_edge)
            .max_by(|left, right| {
                left.anchor
                    .rows
                    .len()
                    .cmp(&right.anchor.rows.len())
                    .then_with(|| right.anchor.deviation.total_cmp(&left.anchor.deviation))
            })
            .or_else(|| {
                group
                    .iter()
                    .map(|candidate| &candidates[*candidate])
                    .max_by_key(|candidate| candidate.anchor.rows.len())
            })
            .expect("column candidate group is not empty");
        let centers: Vec<f64> = best
            .anchor
            .cells
            .iter()
            .map(|location| {
                let cell = cell_at(lines, *location);
                cell.left.midpoint(cell.right)
            })
            .collect();
        columns.push(ColumnGeometry {
            anchor: best.anchor.sum / best.anchor.count as f64,
            edge: best.edge,
            center: median(centers, best.anchor.sum / best.anchor.count as f64),
        });
    }

    columns.sort_by(|left, right| left.center.total_cmp(&right.center));
    columns
}

fn column_bounds(columns: &[ColumnGeometry], index: usize, page_width: f64) -> (f64, f64) {
    let outer_margin = 50.0_f64.max(page_width * 0.045);
    let left = if index == 0 {
        columns[index].center - outer_margin
    } else {
        columns[index - 1].center.midpoint(columns[index].center)
    };
    let right = if index + 1 == columns.len() {
        columns[index].center + outer_margin
    } else {
        columns[index].center.midpoint(columns[index + 1].center)
    };
    (left, right)
}

fn assign_cell(cell: &LayoutCell, columns: &[ColumnGeometry], page_width: f64) -> Option<usize> {
    let tolerance = 20.0_f64.max(page_width * 0.02);
    columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            let (left, right) = column_bounds(columns, index, page_width);
            let overlap = (cell.right.min(right) - cell.left.max(left)).max(0.0);
            let overlap_ratio = overlap / (cell.right - cell.left).max(1.0);
            let edge_distance = match column.edge {
                Edge::Left => (cell.left - column.anchor).abs(),
                Edge::Right => (cell.right - column.anchor).abs(),
            };
            if overlap_ratio < 0.25 && edge_distance > tolerance {
                return None;
            }
            let center_distance = (cell.left.midpoint(cell.right) - column.center).abs();
            let score = edge_distance / tolerance + center_distance / page_width.max(1.0)
                - overlap_ratio * 2.0;
            Some((index, score))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(index, _)| index)
}

fn numeric_ratio<'a>(values: impl Iterator<Item = &'a String>) -> f64 {
    let values: Vec<&String> = values.filter(|value| !value.is_empty()).collect();
    if values.is_empty() {
        0.0
    } else {
        values.iter().filter(|value| looks_numeric(value)).count() as f64 / values.len() as f64
    }
}

fn box_for_cells<'a>(cells: impl Iterator<Item = &'a LayoutCell>) -> Option<TesseractBox> {
    let cells: Vec<&LayoutCell> = cells.collect();
    if cells.is_empty() {
        return None;
    }
    let left = cells
        .iter()
        .map(|cell| cell.left)
        .fold(f64::INFINITY, f64::min);
    let top = cells
        .iter()
        .map(|cell| cell.top)
        .fold(f64::INFINITY, f64::min);
    let right = cells
        .iter()
        .map(|cell| cell.right)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = cells
        .iter()
        .map(|cell| cell.bottom)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(TesseractBox {
        left,
        top,
        width: right - left,
        height: bottom - top,
    })
}

fn build_table_grid(lines: &[LayoutLine], page_number: u32, page_width: f64) -> Option<TableGrid> {
    let mut columns = infer_column_geometry(lines, page_width);
    if columns.len() < 2 {
        return None;
    }

    let tentative_assignments: Vec<Vec<Option<usize>>> = lines
        .iter()
        .map(|line| {
            line.cells
                .iter()
                .map(|cell| assign_cell(cell, &columns, page_width))
                .collect()
        })
        .collect();
    let tentative_rows: Vec<Vec<String>> = lines
        .iter()
        .zip(&tentative_assignments)
        .map(|(line, assignments)| {
            let mut values = vec![String::new(); columns.len()];
            for (cell, assignment) in line.cells.iter().zip(assignments) {
                if let Some(column) = assignment {
                    if !values[*column].is_empty() {
                        values[*column].push(' ');
                    }
                    values[*column].push_str(&cell.text);
                }
            }
            values
        })
        .collect();
    let first_numeric_ratio = numeric_ratio(tentative_rows[0].iter());
    let later_numeric_ratio = numeric_ratio(tentative_rows.iter().skip(1).flatten());
    let first_looks_like_header =
        first_numeric_ratio < 0.35 && later_numeric_ratio > first_numeric_ratio + 0.15;
    if first_looks_like_header && columns.len() >= 3 {
        for (cell, assignment) in lines[0].cells.iter().zip(&tentative_assignments[0]) {
            if assignment.is_none() {
                columns.push(ColumnGeometry {
                    anchor: cell.left,
                    edge: Edge::Left,
                    center: cell.left.midpoint(cell.right),
                });
            }
        }
        columns.sort_by(|left, right| left.center.total_cmp(&right.center));
    }

    let cell_assignments: Vec<Vec<Option<usize>>> = lines
        .iter()
        .map(|line| {
            line.cells
                .iter()
                .map(|cell| assign_cell(cell, &columns, page_width))
                .collect()
        })
        .collect();
    let mut rows: Vec<Vec<String>> = lines
        .iter()
        .zip(&cell_assignments)
        .map(|(line, assignments)| {
            let mut values = vec![String::new(); columns.len()];
            for (cell, assignment) in line.cells.iter().zip(assignments) {
                if let Some(column) = assignment {
                    if !values[*column].is_empty() {
                        values[*column].push(' ');
                    }
                    values[*column].push_str(&cell.text);
                }
            }
            values
        })
        .collect();

    if !rows
        .iter()
        .any(|row| row.iter().filter(|value| !value.is_empty()).count() >= 2)
    {
        return None;
    }

    let paired_rows = rows
        .iter()
        .filter(|row| !row[0].is_empty() && !row[1].is_empty())
        .count();
    let non_numeric_first = rows
        .iter()
        .filter(|row| !row[0].is_empty() && !looks_numeric(&row[0]))
        .count();
    let key_value = columns.len() == 2
        && paired_rows >= 2.max(rows.len().div_ceil(2))
        && non_numeric_first >= rows.len().div_ceil(2);

    let first_looks_like_header = !key_value
        && first_numeric_ratio < 0.35
        && later_numeric_ratio > first_numeric_ratio + 0.15;

    let mut headers = if key_value {
        vec!["Field".to_owned(), "Value".to_owned()]
    } else if first_looks_like_header {
        rows.remove(0)
    } else {
        (1..=columns.len())
            .map(|index| format!("Column {index}"))
            .collect()
    };

    if first_looks_like_header && !rows.is_empty() {
        let qualifier = &rows[0];
        let values: Vec<&String> = qualifier.iter().filter(|value| !value.is_empty()).collect();
        let bracketed = !values.is_empty()
            && values.len() <= headers.len().div_ceil(2)
            && values.iter().all(|value| {
                let value = value.trim();
                (value.starts_with('[') && value.ends_with(']'))
                    || (value.starts_with('(') && value.ends_with(')'))
            });
        if bracketed {
            let qualifier = rows.remove(0);
            for (index, value) in qualifier.into_iter().enumerate() {
                if !value.is_empty() {
                    if !headers[index].is_empty() {
                        headers[index].push(' ');
                    }
                    headers[index].push_str(&value);
                }
            }
        }
    }

    let alignment_samples = if rows.is_empty() {
        std::slice::from_ref(&headers)
    } else {
        rows.as_slice()
    };
    let alignments = (0..columns.len())
        .map(|column| {
            let values: Vec<&String> = alignment_samples
                .iter()
                .filter_map(|row| (!row[column].is_empty()).then_some(&row[column]))
                .collect();
            let numeric_ratio = if values.is_empty() {
                0.0
            } else {
                values.iter().filter(|value| looks_numeric(value)).count() as f64
                    / values.len() as f64
            };
            if numeric_ratio >= 0.65 {
                ColumnAlignment::Right
            } else {
                ColumnAlignment::Left
            }
        })
        .collect::<Vec<_>>();

    let assigned_cells: Vec<&LayoutCell> = lines
        .iter()
        .zip(&cell_assignments)
        .flat_map(|(line, assignments)| {
            line.cells
                .iter()
                .zip(assignments)
                .filter_map(|(cell, assignment)| assignment.is_some().then_some(cell))
        })
        .collect();
    let bbox = box_for_cells(assigned_cells.iter().copied())?;
    let overlaps_table = |cell: &LayoutCell| {
        let overlap = (cell.right.min(bbox.right()) - cell.left.max(bbox.left)).max(0.0);
        overlap / (cell.right - cell.left).max(1.0) >= 0.25
    };
    let assigned_cell_count = cell_assignments
        .iter()
        .flatten()
        .filter(|assignment| assignment.is_some())
        .count();
    let unassigned_cell_count = lines
        .iter()
        .zip(&cell_assignments)
        .flat_map(|(line, assignments)| line.cells.iter().zip(assignments))
        .filter(|(cell, assignment)| assignment.is_none() && overlaps_table(cell))
        .count();
    let excluded_non_table_cell_count = lines
        .iter()
        .zip(&cell_assignments)
        .flat_map(|(line, assignments)| line.cells.iter().zip(assignments))
        .filter(|(cell, assignment)| assignment.is_none() && !overlaps_table(cell))
        .count();
    let assignment_coverage =
        assigned_cell_count as f64 / (assigned_cell_count + unassigned_cell_count).max(1) as f64;
    let recognition_confidence = if assigned_cells.is_empty() {
        0.0
    } else {
        assigned_cells
            .iter()
            .map(|cell| cell.confidence)
            .sum::<f64>()
            / assigned_cells.len() as f64
    };
    let row_pitch = median(
        lines
            .windows(2)
            .map(|pair| pair[1].top - pair[0].top)
            .filter(|distance| *distance > 0.0)
            .collect(),
        1.0,
    );
    let rhythm_consistency = if lines.len() < 3 {
        1.0
    } else {
        let consistent = lines
            .windows(2)
            .map(|pair| pair[1].top - pair[0].top)
            .filter(|distance| (*distance - row_pitch).abs() <= row_pitch.max(1.0) * 0.35)
            .count();
        consistent as f64 / (lines.len() - 1) as f64
    };
    let structural_confidence = assignment_coverage * 0.75 + rhythm_consistency * 0.25;
    let accepted = assignment_coverage >= 0.95 && unassigned_cell_count == 0;
    let mut notes = Vec::new();
    if excluded_non_table_cell_count > 0 {
        notes.push(format!(
            "{excluded_non_table_cell_count} cells remain outside the supported table bbox"
        ));
    }
    if !accepted {
        notes.push("grid rejected because cells inside its bbox were unassigned".to_owned());
    }

    let model_rows = lines
        .iter()
        .zip(&cell_assignments)
        .map(|(line, assignments)| {
            let cells: Vec<TableCellModel> = line
                .cells
                .iter()
                .zip(assignments)
                .filter(|(cell, assignment)| assignment.is_some() || overlaps_table(cell))
                .map(|(cell, assignment)| TableCellModel {
                    text: cell.text.clone(),
                    bbox: TesseractBox {
                        left: cell.left,
                        top: cell.top,
                        width: cell.right - cell.left,
                        height: cell.bottom - cell.top,
                    },
                    column_index: *assignment,
                    recognition_confidence: cell.confidence,
                    words: cell
                        .words
                        .iter()
                        .map(|word| TableWordProvenance {
                            block_number: word.block_number,
                            paragraph_number: word.paragraph_number,
                            line_number: word.line_number,
                            word_number: word.word_number,
                        })
                        .collect(),
                })
                .collect();
            let bbox = box_for_cells(
                line.cells
                    .iter()
                    .zip(assignments)
                    .filter(|(_, assignment)| assignment.is_some())
                    .map(|(cell, _)| cell),
            )
            .unwrap_or(TesseractBox {
                left: line.left,
                top: line.top,
                width: line.right - line.left,
                height: line.bottom - line.top,
            });
            TableRowModel { bbox, cells }
        })
        .collect();
    let column_models = columns
        .iter()
        .enumerate()
        .map(|(index, column)| TableColumnModel {
            anchor: column.anchor,
            geometry: match column.edge {
                Edge::Left => TableColumnGeometry::LeftAligned,
                Edge::Right => TableColumnGeometry::RightAligned,
            },
            numeric: matches!(alignments[index], ColumnAlignment::Right),
        })
        .collect();
    let model = TableRegionModel {
        page_number,
        bbox,
        columns: column_models,
        rows: model_rows,
        diagnostics: TableDiagnostics {
            accepted,
            assigned_cell_count,
            unassigned_cell_count,
            excluded_non_table_cell_count,
            assignment_coverage,
            structural_confidence,
            recognition_confidence,
            notes,
        },
    };

    Some(TableGrid {
        headers,
        alignments,
        rows,
        unassigned_cell_count,
        model,
        cell_assignments,
    })
}

fn split_table_schemas(
    lines: &[LayoutLine],
    page: &TesseractPage,
    table_min_rows: usize,
) -> Vec<Vec<LayoutLine>> {
    if lines.len() < table_min_rows * 2 {
        return vec![lines.to_vec()];
    }
    let seed_size = lines.len().min((table_min_rows + 2).max(5));
    let anchors = cluster_column_anchors(&lines[..seed_size], page.width);
    if anchors.len() < 2 {
        return vec![lines.to_vec()];
    }
    let body_height = median(
        lines[..seed_size].iter().map(|line| line.height).collect(),
        16.0,
    );
    let row_pitch = median(
        lines[1..seed_size]
            .iter()
            .zip(lines[..seed_size - 1].iter())
            .map(|(line, previous)| line.top - previous.top)
            .filter(|distance| *distance > 0.0)
            .collect(),
        body_height * 1.6,
    );
    let tolerance = 24.0_f64.max(body_height * 2.0).max(page.width * 0.03);
    let mut incompatible_from = None;

    for index in seed_size..lines.len() {
        let starts_new_vertical_region =
            lines[index].top - lines[index - 1].top > (row_pitch * 1.6).max(body_height * 2.6);
        if starts_new_vertical_region {
            let before = &lines[..index];
            let after = &lines[index..];
            if before.len() >= table_min_rows && after.len() >= table_min_rows {
                let mut result = vec![before.to_vec()];
                result.extend(split_table_schemas(after, page, table_min_rows));
                return result;
            }
        }

        let matched = lines[index]
            .cells
            .iter()
            .filter(|cell| {
                anchors
                    .iter()
                    .any(|anchor| (anchor - cell.left).abs() <= tolerance)
            })
            .count();
        let compatible = !lines[index].cells.is_empty()
            && matched as f64 / lines[index].cells.len() as f64 >= 0.75;
        if compatible {
            incompatible_from = None;
            continue;
        }
        let start = *incompatible_from.get_or_insert(index);
        if index - start < 1 {
            continue;
        }

        let before = &lines[..start];
        let after = &lines[start..];
        if before.len() < table_min_rows || after.len() < table_min_rows {
            incompatible_from = None;
            continue;
        }
        let mut result = vec![before.to_vec()];
        result.extend(split_table_schemas(after, page, table_min_rows));
        return result;
    }
    vec![lines.to_vec()]
}

fn find_tables(lines: &[LayoutLine], page: &TesseractPage, table_min_rows: usize) -> Vec<TableRun> {
    let mut runs = Vec::new();
    let body_height = median(lines.iter().map(|line| line.height).collect(), 16.0);
    let candidates: Vec<bool> = lines
        .iter()
        .map(|line| is_table_candidate(line, page.width))
        .collect();
    let mut start = None;

    let flush = |to: usize, start: &mut Option<usize>, runs: &mut Vec<TableRun>| {
        let Some(from) = start.take() else {
            return;
        };
        let candidate = &lines[from..to];
        let mut schema_from = from;
        for schema in split_table_schemas(candidate, page, table_min_rows) {
            let schema_to = schema_from + schema.len();
            if schema.len() >= table_min_rows
                && let Some(grid) = build_table_grid(&schema, page.page_number, page.width)
            {
                runs.push(TableRun {
                    from: schema_from,
                    to: schema_to,
                    grid,
                });
            }
            schema_from = schema_to;
        }
    };

    for index in 0..lines.len() {
        let separated = index > 0
            && lines[index].top - lines[index - 1].bottom
                > (body_height * 2.5).max(page.height * 0.025);
        if separated {
            flush(index, &mut start, &mut runs);
        }
        if candidates[index] {
            start.get_or_insert(index);
        } else {
            let bridges_to_next = start.is_some()
                && candidates.get(index + 1).copied().unwrap_or(false)
                && lines[index + 1].top - lines[index].bottom
                    <= (body_height * 2.5).max(page.height * 0.025);
            if !bridges_to_next {
                flush(index, &mut start, &mut runs);
            }
        }
    }
    flush(lines.len(), &mut start, &mut runs);
    runs
}

fn escape_inline(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if "\\`*_[]<>".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn escape_table_cell(value: &str) -> String {
    escape_inline(value)
        .replace('|', "\\|")
        .replace(['\r', '\n'], "<br>")
}

fn render_table(grid: &TableGrid) -> String {
    let render_row = |row: &[String]| {
        format!(
            "| {} |",
            row.iter()
                .map(|value| escape_table_cell(value))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let separators: Vec<String> = grid
        .alignments
        .iter()
        .map(|alignment| match alignment {
            ColumnAlignment::Left => "---".to_owned(),
            ColumnAlignment::Right => "---:".to_owned(),
        })
        .collect();
    std::iter::once(render_row(&grid.headers))
        .chain(std::iter::once(render_row(&separators)))
        .chain(grid.rows.iter().map(|row| render_row(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_residual_cells(lines: &[LayoutLine], grid: &TableGrid) -> Vec<String> {
    lines
        .iter()
        .zip(&grid.cell_assignments)
        .filter_map(|(line, assignments)| {
            let text = line
                .cells
                .iter()
                .zip(assignments)
                .filter(|(_, assignment)| assignment.is_none())
                .map(|(cell, _)| cell.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then(|| escape_inline(&text))
        })
        .collect()
}

fn heading_level(line: &LayoutLine, body_height: f64) -> Option<usize> {
    if line.text.len() > 110 || line.words.len() > 14 {
        return None;
    }
    let ratio = line.height / body_height.max(1.0);
    if ratio >= 2.15 {
        return Some(1);
    }
    if ratio >= 1.65 {
        return Some(2);
    }
    let lower = line.text.to_lowercase();
    if ratio >= 1.35
        && ["rechnung", "gutschrift", "holzliste", "abrechnung"]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    {
        return Some(2);
    }
    None
}

fn render_line(line: &LayoutLine, body_height: f64) -> String {
    let text = escape_inline(&line.text);
    if let Some(level) = heading_level(line, body_height) {
        return format!("{} {text}", "#".repeat(level));
    }
    if line.text.len() >= 4 && line.text.chars().all(|character| "-_=".contains(character)) {
        return "---".to_owned();
    }
    if let Some(bullet) = line
        .text
        .strip_prefix("• ")
        .or_else(|| line.text.strip_prefix("· "))
        .or_else(|| line.text.strip_prefix("- "))
    {
        return format!("- {}", escape_inline(bullet));
    }
    let ordered_prefix = line.text.split_once(' ').and_then(|(prefix, content)| {
        let number = prefix.trim_end_matches(['.', ')']);
        (!number.is_empty()
            && number.chars().all(|character| character.is_ascii_digit())
            && (prefix.ends_with('.') || prefix.ends_with(')')))
        .then_some(content)
    });
    if let Some(content) = ordered_prefix {
        return format!("1. {}", escape_inline(content));
    }
    if line.cells.len() == 2 && line.cells[0].text.ends_with(':') {
        return format!(
            "**{}** {}",
            escape_inline(&line.cells[0].text),
            escape_inline(&line.cells[1].text)
        );
    }
    text
}

/// Convert already-parsed Tesseract pages into Markdown.
#[must_use]
pub fn tesseract_pages_to_markdown(
    pages: &[TesseractPage],
    options: &MarkdownOptions,
    malformed_row_count: usize,
) -> MarkdownResult {
    let table_min_rows = options.table_min_rows.max(2);
    let include_page_headings = options.include_page_headings.unwrap_or(pages.len() > 1);
    let mut output = Vec::new();
    let mut stats = MarkdownStats {
        page_count: pages.len(),
        malformed_row_count,
        ..MarkdownStats::default()
    };

    for (page_index, page) in pages.iter().enumerate() {
        let (lines, retained_words) = build_layout_lines(page, options.min_confidence);
        let body_height = median(
            retained_words.iter().map(|word| word.bbox.height).collect(),
            16.0,
        );
        let tables = find_tables(&lines, page, table_min_rows);
        let tables_by_start: HashMap<usize, &TableRun> = tables
            .iter()
            .filter(|table| table.grid.model.diagnostics.accepted)
            .map(|table| (table.from, table))
            .collect();
        let mut blocks = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            if let Some(table) = tables_by_start.get(&index) {
                blocks.extend(render_residual_cells(
                    &lines[table.from..table.to],
                    &table.grid,
                ));
                blocks.push(render_table(&table.grid));
                stats.table_count += 1;
                stats.unassigned_table_cell_count += table.grid.unassigned_cell_count;
                index = table.to;
            } else {
                blocks.push(render_line(&lines[index], body_height));
                index += 1;
            }
        }

        if include_page_headings {
            output.push(format!("## Page {}", page.page_number));
        }
        output.push(
            blocks
                .into_iter()
                .filter(|block| !block.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
        if page_index + 1 < pages.len() {
            output.push("---".to_owned());
        }
        stats.word_count += retained_words.len();
        stats.low_confidence_word_count += retained_words
            .iter()
            .filter(|word| word.confidence < 50.0)
            .count();
        stats.line_count += lines.len();
    }

    MarkdownResult {
        markdown: format!("{}\n", output.join("\n\n").trim()),
        stats,
    }
}

/// Analyze table regions without rendering Markdown.
#[must_use]
pub fn analyze_tesseract_pages(
    pages: &[TesseractPage],
    options: &MarkdownOptions,
) -> TesseractLayoutAnalysis {
    let table_min_rows = options.table_min_rows.max(2);
    let tables = pages
        .iter()
        .flat_map(|page| {
            let (lines, _) = build_layout_lines(page, options.min_confidence);
            find_tables(&lines, page, table_min_rows)
                .into_iter()
                .map(|table| table.grid.model)
                .collect::<Vec<_>>()
        })
        .collect();
    TesseractLayoutAnalysis { tables }
}

/// Parse Tesseract TSV and analyze table regions without rendering Markdown.
///
/// # Errors
///
/// Returns [`TsvError`] when the TSV header is invalid.
pub fn analyze_tesseract_tsv(
    tsv: &str,
    options: &MarkdownOptions,
) -> Result<TesseractLayoutAnalysis, TsvError> {
    let parsed = parse_tesseract_tsv(tsv)?;
    Ok(analyze_tesseract_pages(&parsed.pages, options))
}

/// Parse Tesseract TSV and convert it into Markdown.
///
/// # Errors
///
/// Returns [`TsvError`] when the TSV header is invalid.
pub fn tesseract_tsv_to_markdown(
    tsv: &str,
    options: &MarkdownOptions,
) -> Result<MarkdownResult, TsvError> {
    let parsed = parse_tesseract_tsv(tsv)?;
    Ok(tesseract_pages_to_markdown(
        &parsed.pages,
        options,
        parsed.malformed_row_count,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tsv_is_empty_markdown() {
        let result = tesseract_tsv_to_markdown("", &MarkdownOptions::default())
            .expect("empty input is valid");
        assert_eq!(result.markdown, "\n");
        assert_eq!(result.stats.page_count, 0);
    }

    #[test]
    fn incomplete_header_reports_columns() {
        let error =
            parse_tesseract_tsv("level\tpage_num\n1\t1\n").expect_err("header is incomplete");
        assert_eq!(
            error.to_string(),
            "invalid Tesseract TSV: missing columns block_num, par_num, line_num, word_num, left, top, width, height, conf, text"
        );
    }

    #[test]
    fn markdown_escaping_preserves_table_shape() {
        assert_eq!(escape_table_cell(r"Eiche|A [x]"), r"Eiche\|A \[x\]");
    }
}
