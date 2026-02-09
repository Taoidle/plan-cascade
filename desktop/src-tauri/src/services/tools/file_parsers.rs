//! File Parsers
//!
//! Rich file parsing for PDF, Jupyter notebooks, DOCX, XLSX, and images.

use calamine::Reader;
use std::path::Path;

/// Maximum file size for document parsing (50MB)
const MAX_DOC_SIZE: u64 = 50 * 1024 * 1024;

/// Maximum file size for image base64 encoding (20MB)
const MAX_IMAGE_SIZE: u64 = 20 * 1024 * 1024;

/// Maximum pages per PDF request
const MAX_PDF_PAGES: usize = 20;

/// Maximum rows per XLSX sheet
const MAX_XLSX_ROWS: usize = 100;

/// Maximum columns per XLSX sheet
const MAX_XLSX_COLS: usize = 50;

/// Maximum sheets per XLSX file
const MAX_XLSX_SHEETS: usize = 5;

/// Check file size against a limit
fn check_file_size(path: &Path, max_size: u64) -> Result<u64, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    let size = metadata.len();
    if size > max_size {
        return Err(format!(
            "File too large: {:.1} MB (max {:.1} MB)",
            size as f64 / (1024.0 * 1024.0),
            max_size as f64 / (1024.0 * 1024.0)
        ));
    }
    Ok(size)
}

/// Parse a page range string like "1-5", "3", "10-20"
fn parse_page_range(pages: &str) -> Result<(usize, usize), String> {
    let pages = pages.trim();
    if let Some((start_str, end_str)) = pages.split_once('-') {
        let start: usize = start_str
            .trim()
            .parse()
            .map_err(|_| format!("Invalid page number: {}", start_str.trim()))?;
        let end: usize = end_str
            .trim()
            .parse()
            .map_err(|_| format!("Invalid page number: {}", end_str.trim()))?;
        if start == 0 || end == 0 {
            return Err("Page numbers must be >= 1".to_string());
        }
        if start > end {
            return Err(format!("Invalid page range: {}-{}", start, end));
        }
        if end - start + 1 > MAX_PDF_PAGES {
            return Err(format!(
                "Too many pages: {} (max {} per request)",
                end - start + 1,
                MAX_PDF_PAGES
            ));
        }
        Ok((start, end))
    } else {
        let page: usize = pages
            .parse()
            .map_err(|_| format!("Invalid page number: {}", pages))?;
        if page == 0 {
            return Err("Page numbers must be >= 1".to_string());
        }
        Ok((page, page))
    }
}

/// Parse a PDF file and extract text content.
///
/// Supports optional page range parameter.
pub fn parse_pdf(path: &Path, pages: Option<&str>) -> Result<String, String> {
    check_file_size(path, MAX_DOC_SIZE)?;

    let text = pdf_extract::extract_text(path)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))?;

    // Split into pages (PDF text extraction gives us the full text)
    // We split on form feeds (\x0c) which pdf-extract uses as page separators
    let all_pages: Vec<&str> = text.split('\x0c').collect();
    let total_pages = all_pages.len();

    if let Some(page_spec) = pages {
        let (start, end) = parse_page_range(page_spec)?;
        if start > total_pages {
            return Err(format!(
                "Page {} out of range (document has {} pages)",
                start, total_pages
            ));
        }
        let end = end.min(total_pages);

        let mut output = format!("PDF: {} ({} total pages, showing {}-{})\n\n", path.display(), total_pages, start, end);
        for i in (start - 1)..end {
            output.push_str(&format!("--- Page {} ---\n{}\n\n", i + 1, all_pages[i].trim()));
        }
        Ok(output)
    } else {
        if total_pages > 10 {
            return Err(format!(
                "PDF has {} pages. Please specify a page range (e.g., pages: \"1-5\"). Max {} pages per request.",
                total_pages, MAX_PDF_PAGES
            ));
        }

        let mut output = format!("PDF: {} ({} pages)\n\n", path.display(), total_pages);
        for (i, page) in all_pages.iter().enumerate() {
            let trimmed = page.trim();
            if !trimmed.is_empty() {
                output.push_str(&format!("--- Page {} ---\n{}\n\n", i + 1, trimmed));
            }
        }
        Ok(output)
    }
}

/// Parse a Jupyter notebook (.ipynb) file.
///
/// Renders cells with their type, source, and text outputs.
pub fn parse_jupyter(path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read notebook: {}", e))?;

    let notebook: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse notebook JSON: {}", e))?;

    let cells = notebook
        .get("cells")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "Invalid notebook: missing 'cells' array".to_string())?;

    let mut output = format!("Jupyter Notebook: {} ({} cells)\n\n", path.display(), cells.len());

    for (i, cell) in cells.iter().enumerate() {
        let cell_type = cell
            .get("cell_type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        let source = cell
            .get("source")
            .map(|s| extract_notebook_text(s))
            .unwrap_or_default();

        match cell_type {
            "code" => {
                output.push_str(&format!("[Cell {} - code]:\n```python\n{}\n```\n", i + 1, source));

                // Extract text outputs
                if let Some(outputs) = cell.get("outputs").and_then(|o| o.as_array()) {
                    for out in outputs {
                        if let Some(text) = out.get("text") {
                            output.push_str(&format!("[Output]: {}\n", extract_notebook_text(text)));
                        } else if let Some(data) = out.get("data") {
                            if let Some(text) = data.get("text/plain") {
                                output.push_str(&format!("[Output]: {}\n", extract_notebook_text(text)));
                            } else {
                                // Skip binary outputs (image/png, etc.)
                                let keys: Vec<&str> = data
                                    .as_object()
                                    .map(|o| o.keys().map(|k| k.as_str()).collect())
                                    .unwrap_or_default();
                                output.push_str(&format!("[Output]: <{}>", keys.join(", ")));
                            }
                        }
                    }
                }
                output.push('\n');
            }
            "markdown" => {
                output.push_str(&format!("[Cell {} - markdown]:\n{}\n\n", i + 1, source));
            }
            "raw" => {
                output.push_str(&format!("[Cell {} - raw]:\n{}\n\n", i + 1, source));
            }
            _ => {
                output.push_str(&format!("[Cell {} - {}]:\n{}\n\n", i + 1, cell_type, source));
            }
        }
    }

    Ok(output)
}

/// Extract text from a notebook source field (can be string or array of strings)
fn extract_notebook_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Parse a DOCX file by extracting text from the XML inside the ZIP archive.
pub fn parse_docx(path: &Path) -> Result<String, String> {
    check_file_size(path, MAX_DOC_SIZE)?;

    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open DOCX: {}", e))?;

    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read DOCX as ZIP: {}", e))?;

    // Read word/document.xml
    let mut doc_xml = String::new();
    {
        let mut doc_entry = archive
            .by_name("word/document.xml")
            .map_err(|_| "Invalid DOCX: missing word/document.xml".to_string())?;
        std::io::Read::read_to_string(&mut doc_entry, &mut doc_xml)
            .map_err(|e| format!("Failed to read document.xml: {}", e))?;
    }

    // Parse XML and extract text from <w:t> elements
    let mut reader = quick_xml::Reader::from_str(&doc_xml);
    let mut output = format!("DOCX: {}\n\n", path.display());
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();
    let mut in_text_element = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) | Ok(quick_xml::events::Event::Empty(ref e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "p" {
                    in_paragraph = true;
                    paragraph_text.clear();
                } else if name == "t" {
                    in_text_element = true;
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");
                if name == "p" {
                    if in_paragraph && !paragraph_text.is_empty() {
                        output.push_str(&paragraph_text);
                        output.push_str("\n\n");
                    }
                    in_paragraph = false;
                } else if name == "t" {
                    in_text_element = false;
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_text_element {
                    if let Ok(text) = e.unescape() {
                        paragraph_text.push_str(&text);
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(output)
}

/// Parse an XLSX/XLS/ODS spreadsheet file.
pub fn parse_xlsx(path: &Path) -> Result<String, String> {
    check_file_size(path, MAX_DOC_SIZE)?;

    let mut workbook = calamine::open_workbook_auto(path)
        .map_err(|e| format!("Failed to open spreadsheet: {}", e))?;

    let sheet_names: Vec<String> = workbook.sheet_names().iter().map(|s| s.to_string()).collect();
    let num_sheets = sheet_names.len().min(MAX_XLSX_SHEETS);

    let mut output = format!(
        "Spreadsheet: {} ({} sheets{})\n\n",
        path.display(),
        sheet_names.len(),
        if sheet_names.len() > MAX_XLSX_SHEETS {
            format!(", showing first {}", MAX_XLSX_SHEETS)
        } else {
            String::new()
        }
    );

    for sheet_name in sheet_names.iter().take(num_sheets) {
        let range = match workbook.worksheet_range(sheet_name) {
            Ok(r) => r,
            Err(e) => {
                output.push_str(&format!("## Sheet: {} (error: {})\n\n", sheet_name, e));
                continue;
            }
        };

        output.push_str(&format!("## Sheet: {}\n\n", sheet_name));

        let rows: Vec<Vec<String>> = range
            .rows()
            .take(MAX_XLSX_ROWS + 1) // +1 for header
            .map(|row| {
                row.iter()
                    .take(MAX_XLSX_COLS)
                    .map(|cell| {
                        // calamine 0.26 rows() returns DataRef references
                        cell.to_string()
                    })
                    .collect()
            })
            .collect();

        if rows.is_empty() {
            output.push_str("(empty sheet)\n\n");
            continue;
        }

        // Calculate column widths
        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![3usize; num_cols]; // minimum width 3
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }

        // Render as markdown table
        for (row_idx, row) in rows.iter().enumerate() {
            output.push('|');
            for (i, width) in col_widths.iter().enumerate() {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                output.push_str(&format!(" {:width$} |", cell, width = width));
            }
            output.push('\n');

            // Header separator after first row
            if row_idx == 0 {
                output.push('|');
                for width in &col_widths {
                    output.push_str(&format!(" {} |", "-".repeat(*width)));
                }
                output.push('\n');
            }
        }

        let total_rows = range.rows().count();
        if total_rows > MAX_XLSX_ROWS + 1 {
            output.push_str(&format!(
                "\n... ({} more rows not shown)\n",
                total_rows - MAX_XLSX_ROWS - 1
            ));
        }
        output.push('\n');
    }

    Ok(output)
}

/// Read image metadata (dimensions, format, file size).
pub fn read_image_metadata(path: &Path) -> Result<String, String> {
    let file_size = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?
        .len();

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase();

    match image::image_dimensions(path) {
        Ok((width, height)) => Ok(format!(
            "Image: {} {}x{}, {}",
            ext.to_uppercase(),
            width,
            height,
            format_file_size(file_size)
        )),
        Err(_) => {
            // For SVG or unsupported formats, just report what we know
            Ok(format!(
                "Image: {} (dimensions unknown), {}",
                ext.to_uppercase(),
                format_file_size(file_size)
            ))
        }
    }
}

/// Encode an image file as base64.
///
/// Returns (mime_type, base64_data).
pub fn encode_image_base64(path: &Path) -> Result<(String, String), String> {
    check_file_size(path, MAX_IMAGE_SIZE)?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mime_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return Err(format!("Unsupported image format: {}", ext)),
    };

    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok((mime_type.to_string(), b64))
}

/// Format a file size in human-readable form
fn format_file_size(size: u64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}

/// Detect if a file extension indicates a parseable rich format
pub fn is_rich_format(ext: &str) -> bool {
    matches!(
        ext,
        "pdf" | "ipynb" | "docx" | "xlsx" | "xls" | "ods"
            | "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_page_range_single() {
        let (start, end) = parse_page_range("3").unwrap();
        assert_eq!(start, 3);
        assert_eq!(end, 3);
    }

    #[test]
    fn test_parse_page_range_range() {
        let (start, end) = parse_page_range("1-5").unwrap();
        assert_eq!(start, 1);
        assert_eq!(end, 5);
    }

    #[test]
    fn test_parse_page_range_invalid() {
        assert!(parse_page_range("0").is_err());
        assert!(parse_page_range("5-3").is_err());
        assert!(parse_page_range("abc").is_err());
    }

    #[test]
    fn test_parse_page_range_too_many() {
        assert!(parse_page_range("1-25").is_err());
    }

    #[test]
    fn test_parse_jupyter_simple() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.ipynb");
        let notebook = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": ["# Hello World"],
                    "metadata": {}
                },
                {
                    "cell_type": "code",
                    "source": ["print('hello')"],
                    "metadata": {},
                    "outputs": [
                        {"text": ["hello\n"]}
                    ]
                }
            ],
            "metadata": {},
            "nbformat": 4,
            "nbformat_minor": 2
        });
        std::fs::write(&path, serde_json::to_string(&notebook).unwrap()).unwrap();

        let result = parse_jupyter(&path).unwrap();
        assert!(result.contains("# Hello World"));
        assert!(result.contains("print('hello')"));
        assert!(result.contains("[Output]: hello"));
    }

    #[test]
    fn test_extract_notebook_text_string() {
        let val = serde_json::json!("hello");
        assert_eq!(extract_notebook_text(&val), "hello");
    }

    #[test]
    fn test_extract_notebook_text_array() {
        let val = serde_json::json!(["line1\n", "line2"]);
        assert_eq!(extract_notebook_text(&val), "line1\nline2");
    }

    #[test]
    fn test_is_rich_format() {
        assert!(is_rich_format("pdf"));
        assert!(is_rich_format("ipynb"));
        assert!(is_rich_format("docx"));
        assert!(is_rich_format("xlsx"));
        assert!(is_rich_format("png"));
        assert!(!is_rich_format("rs"));
        assert!(!is_rich_format("txt"));
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(100), "100 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
    }
}
