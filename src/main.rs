use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use tesseract_to_markdown::{MarkdownOptions, MarkdownStats, tesseract_tsv_to_markdown};

fn usage() -> &'static str {
    "Usage: tesseract-to-markdown [options] <input.tsv> [...]

Inputs:
  -                         Read TSV from stdin

Options:
  --min-confidence N        Drop OCR words below confidence N
  --table-min-rows N        Minimum rows required for a table (default: 3)
  --page-headings           Include explicit page headings
  --stats                   Print conversion statistics to stderr
  -h, --help                Show this help"
}

fn parse_value<T: std::str::FromStr>(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    value
        .parse()
        .map_err(|_| format!("invalid value for {option}: {value}"))
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .map_err(|error| format!("failed to read stdin: {error}"))?;
        Ok(input)
    } else {
        fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
    }
}

fn print_stats(path: &str, stats: &MarkdownStats) {
    eprintln!(
        "{path}: pages={} words={} low_confidence_words={} lines={} tables={} unassigned_table_cells={} malformed_rows={}",
        stats.page_count,
        stats.word_count,
        stats.low_confidence_word_count,
        stats.line_count,
        stats.table_count,
        stats.unassigned_table_cell_count,
        stats.malformed_row_count
    );
}

fn run() -> Result<(), String> {
    let mut options = MarkdownOptions::default();
    let mut show_stats = false;
    let mut paths = Vec::new();
    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(());
            }
            "--min-confidence" => {
                options.min_confidence = parse_value(&mut arguments, "--min-confidence")?;
            }
            "--table-min-rows" => {
                options.table_min_rows = parse_value(&mut arguments, "--table-min-rows")?;
            }
            "--page-headings" => options.include_page_headings = Some(true),
            "--stats" => show_stats = true,
            value if value.starts_with('-') && value != "-" => {
                return Err(format!("unknown option: {value}\n\n{}", usage()));
            }
            path => paths.push(path.to_owned()),
        }
    }

    if paths.is_empty() {
        return Err(usage().to_owned());
    }

    let multiple = paths.len() > 1;
    let mut sections = Vec::new();
    for path in &paths {
        let input = read_input(path)?;
        let result = tesseract_tsv_to_markdown(&input, &options)
            .map_err(|error| format!("{path}: {error}"))?;
        if show_stats {
            print_stats(path, &result.stats);
        }
        if multiple {
            let label = if path == "-" {
                "stdin".to_owned()
            } else {
                Path::new(path)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or(path)
                    .to_owned()
            };
            sections.push(format!("# {label}\n\n{}", result.markdown.trim()));
        } else {
            sections.push(result.markdown.trim().to_owned());
        }
    }

    println!("{}", sections.join("\n\n---\n\n"));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
