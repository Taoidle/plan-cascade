//! NotebookEdit
//!
//! Edit Jupyter notebook (.ipynb) cells: replace, insert, or delete.

use std::path::Path;

/// Edit a Jupyter notebook cell.
///
/// Operations:
/// - `"replace"`: Replace cell content at `cell_index`. Requires `new_source`.
/// - `"insert"`: Insert a new cell at `cell_index`. Requires `new_source` and `cell_type`.
/// - `"delete"`: Delete the cell at `cell_index`.
pub fn edit_notebook(
    path: &Path,
    cell_index: usize,
    operation: &str,
    cell_type: Option<&str>,
    new_source: Option<&str>,
) -> Result<String, String> {
    // Read and parse notebook
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read notebook: {}", e))?;

    let mut notebook: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse notebook JSON: {}", e))?;

    // Validate cells array exists
    if !notebook.get("cells").and_then(|c| c.as_array()).is_some() {
        return Err("Invalid notebook: missing 'cells' array".to_string());
    }

    let result_msg = match operation {
        "replace" => {
            let source =
                new_source.ok_or_else(|| "replace operation requires 'new_source'".to_string())?;

            let cells = notebook["cells"].as_array().unwrap();
            if cell_index >= cells.len() {
                return Err(format!(
                    "Cell index {} out of range (notebook has {} cells)",
                    cell_index,
                    cells.len()
                ));
            }

            // Build source lines
            let source_lines = source_to_lines(source);

            // Mutate through notebook directly
            notebook["cells"][cell_index]["source"] = serde_json::Value::Array(source_lines);

            // Optionally update cell type
            if let Some(ct) = cell_type {
                notebook["cells"][cell_index]["cell_type"] =
                    serde_json::Value::String(ct.to_string());

                // Clear outputs if changing to markdown
                if ct == "markdown" {
                    notebook["cells"][cell_index]
                        .as_object_mut()
                        .map(|obj| obj.remove("outputs"));
                    notebook["cells"][cell_index]
                        .as_object_mut()
                        .map(|obj| obj.remove("execution_count"));
                }
            }

            let ct = notebook["cells"][cell_index]
                .get("cell_type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();

            format!("Replaced cell {} ({})", cell_index, ct)
        }

        "insert" => {
            let source =
                new_source.ok_or_else(|| "insert operation requires 'new_source'".to_string())?;
            let ct = cell_type.ok_or_else(|| {
                "insert operation requires 'cell_type' (\"code\" or \"markdown\")".to_string()
            })?;

            let num_cells = notebook["cells"].as_array().unwrap().len();
            if cell_index > num_cells {
                return Err(format!(
                    "Cell index {} out of range for insert (notebook has {} cells, max insert index is {})",
                    cell_index, num_cells, num_cells
                ));
            }

            let source_lines = source_to_lines(source);

            let mut new_cell = serde_json::json!({
                "cell_type": ct,
                "metadata": {},
                "source": source_lines,
            });

            // Add code-specific fields
            if ct == "code" {
                new_cell["outputs"] = serde_json::json!([]);
                new_cell["execution_count"] = serde_json::Value::Null;
            }

            notebook["cells"]
                .as_array_mut()
                .unwrap()
                .insert(cell_index, new_cell);

            let total = notebook["cells"].as_array().unwrap().len();
            format!(
                "Inserted {} cell at index {} (notebook now has {} cells)",
                ct, cell_index, total
            )
        }

        "delete" => {
            let num_cells = notebook["cells"].as_array().unwrap().len();
            if cell_index >= num_cells {
                return Err(format!(
                    "Cell index {} out of range (notebook has {} cells)",
                    cell_index, num_cells
                ));
            }

            let ct = notebook["cells"][cell_index]
                .get("cell_type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string();

            notebook["cells"].as_array_mut().unwrap().remove(cell_index);

            let total = notebook["cells"].as_array().unwrap().len();
            format!(
                "Deleted cell {} ({}) (notebook now has {} cells)",
                cell_index, ct, total
            )
        }

        other => {
            return Err(format!(
                "Unknown operation: '{}'. Use \"replace\", \"insert\", or \"delete\".",
                other
            ))
        }
    };

    write_notebook(path, &notebook)?;
    Ok(result_msg)
}

/// Convert source text to Jupyter notebook source line format
fn source_to_lines(source: &str) -> Vec<serde_json::Value> {
    let lines: Vec<&str> = source.lines().collect();
    let total = lines.len();
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i < total.saturating_sub(1) {
                serde_json::Value::String(format!("{}\n", line))
            } else {
                serde_json::Value::String(line.to_string())
            }
        })
        .collect()
}

/// Write notebook JSON back to file
fn write_notebook(path: &Path, notebook: &serde_json::Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(notebook)
        .map_err(|e| format!("Failed to serialize notebook: {}", e))?;

    std::fs::write(path, json).map_err(|e| format!("Failed to write notebook: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_notebook(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("test.ipynb");
        let notebook = serde_json::json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "metadata": {},
                    "source": ["# Title"]
                },
                {
                    "cell_type": "code",
                    "metadata": {},
                    "source": ["print('hello')"],
                    "outputs": [],
                    "execution_count": null
                },
                {
                    "cell_type": "code",
                    "metadata": {},
                    "source": ["x = 42"],
                    "outputs": [],
                    "execution_count": null
                }
            ],
            "metadata": {
                "kernelspec": {
                    "display_name": "Python 3",
                    "language": "python",
                    "name": "python3"
                }
            },
            "nbformat": 4,
            "nbformat_minor": 5
        });
        std::fs::write(&path, serde_json::to_string_pretty(&notebook).unwrap()).unwrap();
        path
    }

    #[test]
    fn test_replace_cell() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        let result = edit_notebook(&path, 1, "replace", None, Some("print('world')"));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Replaced cell 1"));

        // Verify change
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let source = content["cells"][1]["source"][0].as_str().unwrap();
        assert_eq!(source, "print('world')");
    }

    #[test]
    fn test_insert_cell() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        let result = edit_notebook(&path, 1, "insert", Some("code"), Some("y = 100"));
        assert!(result.is_ok());
        assert!(result.unwrap().contains("4 cells"));

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["cells"].as_array().unwrap().len(), 4);
        assert_eq!(
            content["cells"][1]["source"][0].as_str().unwrap(),
            "y = 100"
        );
    }

    #[test]
    fn test_delete_cell() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        let result = edit_notebook(&path, 1, "delete", None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("2 cells"));

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["cells"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_out_of_range() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        assert!(edit_notebook(&path, 10, "replace", None, Some("foo")).is_err());
        assert!(edit_notebook(&path, 10, "delete", None, None).is_err());
    }

    #[test]
    fn test_missing_source() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        assert!(edit_notebook(&path, 0, "replace", None, None).is_err());
        assert!(edit_notebook(&path, 0, "insert", Some("code"), None).is_err());
    }

    #[test]
    fn test_insert_requires_cell_type() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = create_test_notebook(dir.path());

        assert!(edit_notebook(&path, 0, "insert", None, Some("foo")).is_err());
    }
}
