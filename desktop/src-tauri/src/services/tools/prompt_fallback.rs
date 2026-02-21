//! Prompt-Based Tool Calling Fallback
//!
//! For LLM providers that don't support native function/tool calling (e.g., Ollama),
//! this module injects tool descriptions into the system prompt and parses tool call
//! blocks from the LLM's text responses.

use serde::{Deserialize, Serialize};

use crate::services::llm::types::ToolDefinition;

/// A tool call parsed from the LLM's text response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedToolCall {
    /// The name of the tool to call
    pub tool_name: String,
    /// The arguments as a JSON value
    pub arguments: serde_json::Value,
    /// The raw text block that was parsed
    pub raw_text: String,
}

/// Build the prompt-based tool calling instructions to inject into the system prompt.
///
/// This instructs the LLM to output tool calls in a specific parseable format.
pub fn build_tool_call_instructions(tools: &[ToolDefinition]) -> String {
    let mut tool_descriptions = String::new();

    for tool in tools {
        tool_descriptions.push_str(&format!("### {}\n", tool.name));
        tool_descriptions.push_str(&format!("{}\n", tool.description));

        // Describe parameters
        if let Some(properties) = tool.input_schema.properties.as_ref() {
            if !properties.is_empty() {
                tool_descriptions.push_str("Parameters:\n");
                let required = tool
                    .input_schema
                    .required
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                for (name, schema) in properties {
                    let type_str = &schema.schema_type;
                    let req_marker = if required.contains(name) {
                        " (required)"
                    } else {
                        " (optional)"
                    };
                    let desc = schema.description.as_deref().unwrap_or("");
                    tool_descriptions.push_str(&format!(
                        "  - `{}` ({}{}): {}\n",
                        name, type_str, req_marker, desc
                    ));
                }
            }
        }
        tool_descriptions.push('\n');
    }

    format!(
        r#"## Tool Calling / 工具调用

You have access to the following tools. To use a tool, output a tool call block in this EXACT format:
请使用以下格式调用工具（必须严格遵守格式）：

```tool_call
{{"tool": "ToolName", "arguments": {{"param1": "value1", "param2": "value2"}}}}
```

IMPORTANT / 重要提示:
- The block MUST start with ```tool_call and end with ``` / 代码块必须以 ```tool_call 开头，以 ``` 结尾
- The JSON MUST be valid and on a single line or properly formatted / JSON 必须有效且格式正确
- You can make multiple tool calls in a single response / 可以在一次回复中调用多个工具
- After making tool calls, STOP and WAIT for the actual results before continuing / 调用工具后，必须停下来等待实际结果，然后再继续
- NEVER fabricate, predict, or describe tool results. Do NOT write "调用成功" or "returns..." — only use REAL results provided after tool execution / 绝对不要伪造、预测或描述工具结果。不要写"调用成功"或"返回..."——只使用工具执行后提供的真实结果
- Do NOT describe what you will do — just emit the tool call block / 不要描述你将要做什么——直接输出工具调用代码块
- Only use tools from the list below / 只使用下面列出的工具

## Available Tools / 可用工具

{tool_descriptions}## Example Tool Calls / 工具调用示例

读取文件 (Read a file):
```tool_call
{{"tool": "Read", "arguments": {{"file_path": "src/main.rs"}}}}
```

列出目录内容 (List directory contents):
```tool_call
{{"tool": "LS", "arguments": {{"path": "."}}}}
```

运行命令 (Run a command):
```tool_call
{{"tool": "Bash", "arguments": {{"command": "cargo test"}}}}
```

搜索代码 (Search code):
```tool_call
{{"tool": "Grep", "arguments": {{"pattern": "fn main", "path": "src/"}}}}
```

搜索符号和代码 (Search symbols and code with index):
```tool_call
{{"tool": "CodebaseSearch", "arguments": {{"query": "authenticate", "scope": "all"}}}}
```

When you receive a tool result, analyze it and decide whether to make more tool calls or provide your final response.
收到工具结果后，分析结果并决定是否需要继续调用工具或给出最终回答。"#,
        tool_descriptions = tool_descriptions,
    )
}

/// Parse tool call blocks from an LLM text response.
///
/// Handles multiple formats:
/// - Pass 1: `` ```tool_call ... ``` `` markdown blocks
/// - Pass 2: `<tool_call>...</tool_call>` XML blocks
/// - Pass 3: `[TOOL] ToolName(args)` bracket format
pub fn parse_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    // Pass 1: Match ```tool_call ... ``` markdown blocks
    let mut remaining = text;
    while let Some(start) = remaining.find("```tool_call") {
        let after_marker = &remaining[start + 12..]; // skip "```tool_call"

        // Find the closing ```
        if let Some(end) = after_marker.find("```") {
            let block_content = after_marker[..end].trim();

            if let Some(parsed) = parse_single_tool_call(block_content) {
                calls.push(ParsedToolCall {
                    tool_name: parsed.0,
                    arguments: parsed.1,
                    raw_text: format!("```tool_call\n{}\n```", block_content),
                });
            }

            remaining = &after_marker[end + 3..];
        } else {
            break;
        }
    }

    // Pass 2: Match <tool_call>...</tool_call> XML blocks (Qwen/GLM/DeepSeek text output)
    let mut remaining = text;
    while let Some(start) = remaining.find("<tool_call>") {
        let after_tag = &remaining[start + 11..]; // skip "<tool_call>"
        if let Some(end) = after_tag.find("</tool_call>") {
            let block_content = after_tag[..end].trim();
            if let Some(parsed) = parse_single_tool_call(block_content) {
                calls.push(ParsedToolCall {
                    tool_name: parsed.0,
                    arguments: parsed.1,
                    raw_text: format!("<tool_call>{}</tool_call>", block_content),
                });
            } else {
                // Not valid JSON — try lenient parsing (tool name with optional args)
                calls.extend(parse_lenient_tool_calls(block_content));
            }
            remaining = &after_tag[end + 12..]; // skip "</tool_call>" (12 chars)
        } else {
            // No closing tag — try to parse everything after <tool_call> until end or next '<'
            let block_content = after_tag.split('<').next().unwrap_or("").trim();
            if !block_content.is_empty() {
                if let Some(parsed) = parse_single_tool_call(block_content) {
                    calls.push(ParsedToolCall {
                        tool_name: parsed.0,
                        arguments: parsed.1,
                        raw_text: format!("<tool_call>{}</tool_call>", block_content),
                    });
                } else {
                    calls.extend(parse_lenient_tool_calls(block_content));
                }
            }
            break;
        }
    }

    // Pass 3: Match [TOOL] ToolName(args) bracket format
    // Only run if passes 1 and 2 found nothing (avoid double-parsing)
    if calls.is_empty() {
        calls.extend(parse_bracket_tool_calls(text));
    }

    // Pass 4: Match <ToolName><param>value</param></ToolName> direct XML blocks.
    // GLM/Chinese LLMs often generate tool calls as XML with the tool name as the
    // outer tag and parameters as nested tags, e.g.:
    //   <ls><path>.</path></ls>
    //   <Read><file_path>src/main.rs</file_path></Read>
    //   <grep><pattern>class.*</pattern></grep>
    // Case-insensitive matching maps to canonical tool names (ls→LS, grep→Grep).
    if calls.is_empty() {
        calls.extend(parse_direct_xml_tool_calls(text));
    }

    // Pass 5: Bare function-call style — ToolName(args) without any wrapper.
    // Some LLMs output tool calls as plain function calls without [TOOL] prefix or
    // XML wrapping, e.g.:
    //   LS(D:\VsCodeProjects\project)
    //   Read(src/main.rs)
    //   Grep("pattern", path="src/")
    // Case-insensitive matching. Requires '(' immediately after tool name and the
    // character before the tool name must be a word boundary (start of line, whitespace,
    // or punctuation) to avoid matching inside regular words.
    if calls.is_empty() {
        calls.extend(parse_bare_function_calls(text));
    }

    // Pass 6: Bare JSON tool calls — {"tool": "Name", "arguments": {...}}
    // Some LLMs output raw JSON without markdown code block wrapping, optionally
    // preceded by a "tool_call:" label, e.g.:
    //   tool_call:
    //   {"tool": "Grep", "arguments": {"pattern": "class"}}
    // or just:
    //   {"tool": "Read", "arguments": {"file_path": "src/main.rs"}}
    if calls.is_empty() {
        calls.extend(parse_bare_json_tool_calls(text));
    }

    // Normalize parameter names for each parsed tool call
    for call in &mut calls {
        normalize_tool_arguments(&call.tool_name.clone(), &mut call.arguments);
    }

    calls
}

/// Parse `[TOOL] ToolName(args)` or `[Tool] ToolName(args)` bracket patterns.
///
/// Handles:
/// - `[TOOL] Read(README.md)` → Read with inferred file_path
/// - `[TOOL] Read(README.md) (id: fallback_5)` → same, id ignored
/// - Multiple `[TOOL]` calls in same text
fn parse_bracket_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();
    let lower = text.to_lowercase();
    let mut search_from = 0;

    while let Some(rel_pos) = lower[search_from..].find("[tool]") {
        let bracket_pos = search_from + rel_pos;
        let after_bracket = &text[bracket_pos + 6..]; // skip "[TOOL]" or "[tool]"
        let after_marker = after_bracket.trim_start();
        // offset within `text` where after_marker starts
        let marker_offset = text.len() - after_marker.len();

        let mut found = false;
        for tool in KNOWN_TOOLS {
            if after_marker.starts_with(tool) {
                let after_tool = &after_marker[tool.len()..];

                if after_tool.starts_with('(') {
                    if let Some(close_paren) = after_tool.find(')') {
                        let inner = after_tool[1..close_paren].trim();
                        let arguments = infer_tool_arguments(tool, inner);
                        let end_offset = marker_offset + tool.len() + close_paren + 1;
                        calls.push(ParsedToolCall {
                            tool_name: tool.to_string(),
                            arguments,
                            raw_text: text[bracket_pos..end_offset].to_string(),
                        });
                        search_from = end_offset;
                        found = true;
                        break;
                    }
                } else {
                    let end_offset = marker_offset + tool.len();
                    calls.push(ParsedToolCall {
                        tool_name: tool.to_string(),
                        arguments: serde_json::Value::Object(serde_json::Map::new()),
                        raw_text: text[bracket_pos..end_offset].to_string(),
                    });
                    search_from = end_offset;
                    found = true;
                    break;
                }
            }
        }

        if !found {
            search_from = bracket_pos + 6;
        }
    }

    calls
}

/// Parse `<ToolName><param>value</param></ToolName>` direct XML tool calls.
///
/// GLM and other Chinese LLMs generate tool calls as XML where the outer tag is the
/// tool name and inner tags are parameter names, e.g.:
/// - `<ls><path>.</path></ls>`
/// - `<Read><file_path>src/main.rs</file_path></Read>`
/// - `<grep><pattern>class.*</pattern><path>src/</path></grep>`
///
/// Case-insensitive matching maps lowercase tags to canonical tool names (ls→LS, grep→Grep).
fn parse_direct_xml_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();
    let lower = text.to_lowercase();

    for tool in KNOWN_TOOLS {
        let lower_tool = tool.to_lowercase();
        let open_tag = format!("<{}", lower_tool);
        let close_tag = format!("</{}>", lower_tool);
        let mut search_pos = 0;

        while search_pos < lower.len() {
            // Find opening tag
            let start = match lower[search_pos..].find(&open_tag) {
                Some(p) => search_pos + p,
                None => break,
            };

            // Find end of opening tag (handle `<ls>` or `<ls attr="...">`)
            let after_open = start + open_tag.len();
            if after_open >= text.len() {
                break;
            }
            let tag_end = match text[after_open..].find('>') {
                Some(p) => after_open + p + 1,
                None => break,
            };

            // Find closing tag
            if let Some(close_rel) = lower[tag_end..].find(&close_tag) {
                let content = text[tag_end..tag_end + close_rel].trim();
                let raw_end = tag_end + close_rel + close_tag.len();

                // Parse nested <param_name>value</param_name> pairs from the content
                let args = parse_nested_xml_args(content);

                // If no nested tags found but content is non-empty, infer primary param
                let args = if args.as_object().map_or(true, |m| m.is_empty()) && !content.is_empty()
                {
                    // Content might be bare text (e.g. <ls>.</ls>)
                    let bare = content
                        .trim()
                        .trim_start_matches(|c: char| c == '<' || c == '/')
                        .trim();
                    if !bare.is_empty() && !bare.contains('<') {
                        infer_tool_arguments(tool, bare)
                    } else {
                        args
                    }
                } else {
                    args
                };

                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments: args,
                    raw_text: text[start..raw_end].to_string(),
                });
                search_pos = raw_end;
            } else {
                search_pos = tag_end;
            }
        }
    }

    // Sort by position in original text (tool iteration order may differ from text order)
    calls.sort_by_key(|c| text.find(&c.raw_text).unwrap_or(usize::MAX));

    calls
}

/// Parse bare function-call style tool calls: `ToolName(args)`.
///
/// Only matches when the tool call appears at the **start of a line** (after optional
/// whitespace). This prevents matching tool names embedded in natural language like
/// "I used LS(.) to explore" — which would cause infinite re-execution loops.
///
/// Matches patterns like:
///   `LS(D:\VsCodeProjects\path)` → LS with path (line starts with "LS(")
///   `Read(src/main.rs)` → Read with file_path
///   `Grep("pattern")` → Grep with pattern
///
/// Case-insensitive tool name matching.
fn parse_bare_function_calls(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        let lower_trimmed = trimmed.to_lowercase();

        for tool in KNOWN_TOOLS {
            let lower_tool = tool.to_lowercase();
            let pattern = format!("{}(", lower_tool);

            if lower_trimmed.starts_with(&pattern) {
                let paren_start = lower_tool.len();
                // Find closing ')' — use simple matching (no nesting)
                if let Some(close_rel) = trimmed[paren_start + 1..].find(')') {
                    let inner = trimmed[paren_start + 1..paren_start + 1 + close_rel].trim();
                    if !inner.is_empty() {
                        let arguments = infer_tool_arguments(tool, inner);
                        let raw_text = trimmed[..paren_start + 1 + close_rel + 1].to_string();
                        calls.push(ParsedToolCall {
                            tool_name: tool.to_string(),
                            arguments,
                            raw_text,
                        });
                    }
                }
                break; // Only one tool match per line
            }
        }
    }

    calls
}

/// Parse bare JSON tool calls: `{"tool": "Name", "arguments": {...}}`.
///
/// Matches patterns like:
///   `tool_call:\n{"tool": "Grep", "arguments": {"pattern": "class"}}` (labeled JSON)
///   `{"tool": "Read", "arguments": {"file_path": "src/main.rs"}}` (bare JSON)
///
/// Uses `parse_single_tool_call` for the JSON parsing itself.
fn parse_bare_json_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    if has_unclosed_tool_call_fence(text) {
        return Vec::new();
    }

    let mut calls = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Check for "tool_call:" prefix — JSON starts on the next line
        let json_start_line =
            if line.eq_ignore_ascii_case("tool_call:") || line.eq_ignore_ascii_case("tool_call") {
                i += 1;
                if i < lines.len() {
                    Some(i)
                } else {
                    None
                }
            } else if line.starts_with('{') && line.contains("\"tool\"") {
                // Bare JSON on this line
                Some(i)
            } else {
                None
            };

        if let Some(start_line) = json_start_line {
            let first = lines[start_line].trim();
            if first.starts_with('{') {
                // Collect lines until braces are balanced
                let mut json_str = first.to_string();
                let mut brace_count =
                    first.matches('{').count() as i32 - first.matches('}').count() as i32;
                let mut j = start_line + 1;
                while brace_count > 0 && j < lines.len() {
                    let next = lines[j].trim();
                    json_str.push('\n');
                    json_str.push_str(next);
                    brace_count +=
                        next.matches('{').count() as i32 - next.matches('}').count() as i32;
                    j += 1;
                }

                if let Some(parsed) = parse_single_tool_call(&json_str) {
                    calls.push(ParsedToolCall {
                        tool_name: parsed.0,
                        arguments: parsed.1,
                        raw_text: json_str,
                    });
                }
                i = j;
                continue;
            }
        }

        i += 1;
    }

    calls
}

fn has_unclosed_tool_call_fence(text: &str) -> bool {
    let mut remaining = text;
    while let Some(start) = remaining.find("```tool_call") {
        let after_marker = &remaining[start + 12..];
        if let Some(end) = after_marker.find("```") {
            remaining = &after_marker[end + 3..];
        } else {
            return true;
        }
    }
    false
}

/// Parse nested `<param_name>value</param_name>` XML pairs from content.
///
/// Given content like `<path>.</path><pattern>*.rs</pattern>`, extracts
/// `{"path": ".", "pattern": "*.rs"}`.
fn parse_nested_xml_args(content: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut remaining = content;

    while let Some(open_start) = remaining.find('<') {
        let after_open = &remaining[open_start + 1..];

        // Skip closing tags (</...>)
        if after_open.starts_with('/') {
            if let Some(gt) = after_open.find('>') {
                remaining = &after_open[gt + 1..];
            } else {
                break;
            }
            continue;
        }

        // Find tag name
        if let Some(open_end) = after_open.find('>') {
            let tag_name = after_open[..open_end].trim();
            if tag_name.is_empty() {
                remaining = &after_open[open_end + 1..];
                continue;
            }

            // Find closing tag for this parameter
            let close_tag = format!("</{}>", tag_name);
            let search_area = &after_open[open_end + 1..];
            if let Some(close_pos) = search_area.find(&close_tag) {
                let value = search_area[..close_pos].trim();
                map.insert(
                    tag_name.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
                remaining = &search_area[close_pos + close_tag.len()..];
            } else {
                remaining = &after_open[open_end + 1..];
            }
        } else {
            break;
        }
    }

    serde_json::Value::Object(map)
}

/// Infer structured arguments from a bare value based on tool name.
///
/// When the model writes `Read(README.md)`, the `README.md` is clearly the
/// primary parameter. This maps tool names to their primary parameter.
fn infer_tool_arguments(tool_name: &str, bare_value: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    // First, try to parse as JSON
    if bare_value.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(bare_value) {
            return json;
        }
    }

    // Try key=value or key: value inside the parens
    if bare_value.contains('=') {
        if let Some(args) = parse_equals_args(bare_value) {
            return args;
        }
    }
    if bare_value.contains(':') {
        // Skip colon parsing for Windows drive paths like "D:\..." or "C:/..."
        let is_windows_drive = bare_value.len() >= 3
            && bare_value.as_bytes()[0].is_ascii_alphabetic()
            && bare_value.as_bytes()[1] == b':'
            && (bare_value.len() == 2
                || bare_value.as_bytes()[2] == b'\\'
                || bare_value.as_bytes()[2] == b'/');
        if !is_windows_drive {
            if let Some(args) = parse_colon_args(bare_value) {
                return args;
            }
        }
    }

    // Infer the primary parameter from tool name
    let primary_param = match tool_name {
        "Read" => Some("file_path"),
        "Write" => Some("file_path"),
        "Edit" => Some("file_path"),
        "LS" => Some("path"),
        "Glob" => Some("pattern"),
        "Grep" => Some("pattern"),
        "Bash" => Some("command"),
        "WebFetch" => Some("url"),
        "WebSearch" => Some("query"),
        "NotebookEdit" => Some("notebook_path"),
        _ => None,
    };

    if let Some(param) = primary_param {
        if !bare_value.is_empty() {
            map.insert(
                param.to_string(),
                serde_json::Value::String(bare_value.trim_matches('"').to_string()),
            );
        }
    }

    serde_json::Value::Object(map)
}

/// Normalize tool argument keys to canonical parameter names.
///
/// LLMs (especially in prompt-fallback mode) often use alternative parameter names.
/// For example, `path` instead of `file_path` for Read. This function renames
/// known aliases to the canonical names the executor expects.
fn normalize_tool_arguments(tool_name: &str, args: &mut serde_json::Value) {
    let map = match args.as_object_mut() {
        Some(m) => m,
        None => return,
    };

    // Define canonical renames: (tool, alias) → canonical_name
    // Only rename if the canonical name is NOT already present
    let renames: &[(&str, &str, &str)] = &[
        // Read: path → file_path, filepath → file_path
        ("Read", "path", "file_path"),
        ("Read", "filepath", "file_path"),
        ("Read", "file", "file_path"),
        // Write: path → file_path, filepath → file_path
        ("Write", "path", "file_path"),
        ("Write", "filepath", "file_path"),
        ("Write", "file", "file_path"),
        // Edit: path → file_path, filepath → file_path
        ("Edit", "path", "file_path"),
        ("Edit", "filepath", "file_path"),
        ("Edit", "file", "file_path"),
        // LS: directory → path, dir → path
        ("LS", "directory", "path"),
        ("LS", "dir", "path"),
        // Glob: glob → pattern
        ("Glob", "glob", "pattern"),
        // Grep: search → pattern, regex → pattern
        ("Grep", "search", "pattern"),
        ("Grep", "regex", "pattern"),
        // Bash: cmd → command
        ("Bash", "cmd", "command"),
        // WebFetch: link → url, address → url
        ("WebFetch", "link", "url"),
        ("WebFetch", "address", "url"),
        // WebSearch: search → query, q → query
        ("WebSearch", "search", "query"),
        ("WebSearch", "q", "query"),
        // Analyze: scope → mode, focus/path/file → path_hint
        ("Analyze", "scope", "mode"),
        ("Analyze", "focus", "path_hint"),
        ("Analyze", "path", "path_hint"),
        ("Analyze", "file", "path_hint"),
        // NotebookEdit: path → notebook_path, filepath → notebook_path
        ("NotebookEdit", "path", "notebook_path"),
        ("NotebookEdit", "filepath", "notebook_path"),
    ];

    for &(tool, alias, canonical) in renames {
        if tool_name == tool && map.contains_key(alias) && !map.contains_key(canonical) {
            if let Some(value) = map.remove(alias) {
                map.insert(canonical.to_string(), value);
            }
        }
    }
}

/// Parse a single tool call JSON block.
/// Returns (tool_name, arguments) or None if parsing fails.
fn parse_single_tool_call(content: &str) -> Option<(String, serde_json::Value)> {
    // Try to parse as JSON
    let json: serde_json::Value = serde_json::from_str(content).ok()?;

    let tool_name = json.get("tool")?.as_str()?.to_string();
    let arguments = json
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    Some((tool_name, arguments))
}

/// Known tool names for lenient parsing.
const KNOWN_TOOLS: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "Bash",
    "Glob",
    "Grep",
    "LS",
    "Cwd",
    "Analyze",
    "Task",
    "WebFetch",
    "WebSearch",
    "NotebookEdit",
];

/// Attempt to parse lenient tool calls from freeform text.
///
/// Handles many formats that Chinese LLMs output:
/// - `"ToolName"` — tool name only, no arguments
/// - `"ToolName {"arg": "val"}"` — tool name followed by JSON arguments
/// - `"LS Cwd"` — multiple tool names separated by whitespace
/// - `"LS<arg_key>path</arg_key><arg_value>.</arg_value>"` — XML arg pairs
/// - `"LS path=."` or `"LS path=\".\""` — equals-separated key=value
/// - `"LS path: ."` or `"LS (path: .)"` — colon-separated key: value
fn parse_lenient_tool_calls(content: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    for tool in KNOWN_TOOLS {
        if content.starts_with(tool) {
            let rest = content[tool.len()..].trim();

            // --- Pre-cleanup: normalize `rest` before trying parsers ---

            // A. Strip duplicate tool name prefix from rest (same tool repeated).
            //    e.g. "ReadRead file_path=..." → rest starts with "Read" (same tool), strip it.
            //    Only strip if it's the SAME tool name, not a different tool like "Cwd".
            let rest = if rest.starts_with(tool) {
                rest[tool.len()..].trim()
            } else {
                rest
            };

            // B. Strip `(id: ...)` prefix that LLMs copy from tool result formatting.
            //    e.g. "(id: story_fallback_6){"path": "..."}" → `{"path": "..."}`
            //    Only strip the `(id:...)` segment, preserve any trailing content after `)`.
            //    Must NOT consume `(id: x, path: ".")` — that's a colon-args pattern.
            let rest =
                if (rest.starts_with("(id:") || rest.starts_with("(id ")) && !rest.contains(',') {
                    if let Some(close_paren) = rest.find(')') {
                        rest[close_paren + 1..].trim()
                    } else {
                        rest
                    }
                } else {
                    rest
                };

            // 1. Tool name only, no arguments
            if rest.is_empty() {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments: serde_json::Value::Object(serde_json::Map::new()),
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 1.5 Chained tools: "Cwd LS {...}" or "LS Cwd"
            if let Some((next_tool, next_rest)) = parse_leading_known_tool(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments: serde_json::Value::Object(serde_json::Map::new()),
                    raw_text: format!("<tool_call>{}</tool_call>", tool),
                });
                let arguments = if next_rest.is_empty() {
                    serde_json::Value::Object(serde_json::Map::new())
                } else {
                    parse_lenient_arguments(next_tool, next_rest)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
                };
                calls.push(ParsedToolCall {
                    tool_name: next_tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 2. Tool name followed by JSON arguments: ToolName {"key": "val"}
            if rest.starts_with('{') {
                let arguments = serde_json::from_str(rest)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 3. XML arg pairs: <arg_key>k</arg_key><arg_value>v</arg_value>
            //    Also handles malformed: <tool_call>k</arg_key><arg_value>v</arg_value>
            if rest.contains("</arg_key>") {
                let arguments = parse_xml_arg_pairs(rest);
                if arguments.as_object().map_or(false, |m| !m.is_empty()) {
                    calls.push(ParsedToolCall {
                        tool_name: tool.to_string(),
                        arguments,
                        raw_text: format!("<tool_call>{}</tool_call>", content),
                    });
                    return calls;
                }
            }

            // 4. Multiple tool names: "LS Cwd"
            let words: Vec<&str> = content.split_whitespace().collect();
            if words.len() > 1 && words.iter().all(|w| KNOWN_TOOLS.contains(w)) {
                for word in &words {
                    calls.push(ParsedToolCall {
                        tool_name: word.to_string(),
                        arguments: serde_json::Value::Object(serde_json::Map::new()),
                        raw_text: format!("<tool_call>{}</tool_call>", word),
                    });
                }
                return calls;
            }

            // C. Function-call style: JunkWord("value") or just ("value")
            //    e.g. rest = `Return("pyproject.toml")` → extract "pyproject.toml"
            //    e.g. rest = `(".")` → extract "."
            if let Some(open) = rest.find('(') {
                if let Some(close) = rest[open..].find(')') {
                    let inner = rest[open + 1..open + close].trim().trim_matches('"');
                    if !inner.is_empty() {
                        let arguments = infer_tool_arguments(tool, inner);
                        if arguments.as_object().map_or(false, |m| !m.is_empty()) {
                            calls.push(ParsedToolCall {
                                tool_name: tool.to_string(),
                                arguments,
                                raw_text: format!("<tool_call>{}</tool_call>", content),
                            });
                            return calls;
                        }
                    }
                }
            }

            // 5. Equals-separated: key=value, key="value", key1=v1 key2=v2
            if let Some(arguments) = parse_equals_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 6. Colon-separated: "key: value" or "(key: value)"
            if let Some(arguments) = parse_colon_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 7. Degenerate XML: <arg_key>key=value</arg_value> (mixed tags, key=val inside)
            if let Some(arguments) = parse_degenerate_xml_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            // 8. Final fallback:
            // - For path-discovery tools, synthesize safe defaults.
            // - For tools with required arguments (Read/Grep/Bash/etc), skip instead
            //   of emitting an empty call that only creates noisy "missing parameter" retries.
            let synthesized = match *tool {
                "Cwd" => Some(serde_json::Value::Object(serde_json::Map::new())),
                "LS" => Some(serde_json::json!({ "path": "." })),
                "Glob" => Some(serde_json::json!({ "pattern": "**/*", "path": "." })),
                _ => None,
            };
            if let Some(arguments) = synthesized {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
            }
            return calls;
        }
    }

    calls
}

fn parse_leading_known_tool(rest: &str) -> Option<(&'static str, &str)> {
    for tool in KNOWN_TOOLS {
        if let Some(after) = rest.strip_prefix(tool) {
            let boundary_ok = after
                .chars()
                .next()
                .map(|c| c.is_whitespace() || c == '{' || c == '(' || c == '<')
                .unwrap_or(true);
            if boundary_ok {
                return Some((tool, after.trim_start()));
            }
        }
    }
    None
}

fn parse_lenient_arguments(tool: &str, rest: &str) -> Option<serde_json::Value> {
    if rest.starts_with('{') {
        return Some(
            serde_json::from_str(rest).unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        );
    }

    if rest.contains("</arg_key>") {
        let arguments = parse_xml_arg_pairs(rest);
        if arguments.as_object().map_or(false, |m| !m.is_empty()) {
            return Some(arguments);
        }
    }

    if let Some(open) = rest.find('(') {
        if let Some(close) = rest[open..].find(')') {
            let inner = rest[open + 1..open + close].trim().trim_matches('"');
            if !inner.is_empty() {
                let arguments = infer_tool_arguments(tool, inner);
                if arguments.as_object().map_or(false, |m| !m.is_empty()) {
                    return Some(arguments);
                }
            }
        }
    }

    if let Some(arguments) = parse_equals_args(rest) {
        return Some(arguments);
    }
    if let Some(arguments) = parse_colon_args(rest) {
        return Some(arguments);
    }
    if let Some(arguments) = parse_degenerate_xml_args(rest) {
        return Some(arguments);
    }

    None
}

/// Parse XML argument pairs into a JSON object.
///
/// Handles both correct and malformed opening tags:
/// - `<arg_key>name</arg_key><arg_value>val</arg_value>` (correct)
/// - `<tool_call>name</arg_key><arg_value>val</arg_value>` (model confuses tags)
/// - Any `<...>name</arg_key><arg_value>val</arg_value>` pattern
///
/// The key insight: we anchor on `</arg_key>` (always correct) and extract
/// the key text between the nearest preceding `>` and `</arg_key>`.
fn parse_xml_arg_pairs(text: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut remaining = text;

    while let Some(close_key_pos) = remaining.find("</arg_key>") {
        // Extract key: text between the last '>' before </arg_key> and the </arg_key> itself.
        // This handles <arg_key>name</arg_key>, <tool_call>name</arg_key>, etc.
        let before_close = &remaining[..close_key_pos];
        let key = if let Some(last_gt) = before_close.rfind('>') {
            before_close[last_gt + 1..].trim()
        } else {
            before_close.trim()
        };

        let after_close_key = &remaining[close_key_pos + 10..]; // skip "</arg_key>"

        if let Some(val_start) = after_close_key.find("<arg_value>") {
            let after_val_tag = &after_close_key[val_start + 11..]; // skip "<arg_value>"
            if let Some(val_end) = after_val_tag.find("</arg_value>") {
                let value = after_val_tag[..val_end].trim();
                if !key.is_empty() {
                    map.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
                remaining = &after_val_tag[val_end + 12..]; // skip "</arg_value>"
                continue;
            }
        }
        break;
    }

    serde_json::Value::Object(map)
}

/// Parse `key=value` style arguments.
///
/// Handles:
/// - `path=.` → `{"path": "."}`
/// - `path="."` → `{"path": "."}`
/// - `path=./src` → `{"path": "./src"}`
/// - `key1=val1 key2=val2` → `{"key1": "val1", "key2": "val2"}`
/// - `path={"path": "."}` → extracts JSON after `=`
fn parse_equals_args(text: &str) -> Option<serde_json::Value> {
    // Quick check: must contain at least one `=`
    if !text.contains('=') {
        return None;
    }

    // Special case: if there's `={` it might be key=JSON
    if let Some(eq_brace) = text.find("={") {
        let json_str = &text[eq_brace + 1..];
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            if json.is_object() {
                return Some(json);
            }
        }
    }

    let mut map = serde_json::Map::new();

    // Split on whitespace and parse each key=value pair
    // But be careful: values might contain spaces if quoted
    let mut remaining = text.trim();

    while !remaining.is_empty() {
        // Skip any XML-like tags (e.g. `<arg_key>`)
        if remaining.starts_with('<') {
            if let Some(gt) = remaining.find('>') {
                remaining = remaining[gt + 1..].trim();
                continue;
            }
            break;
        }

        // Find the next `=`
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => break,
        };

        let key = remaining[..eq_pos]
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        let after_eq = &remaining[eq_pos + 1..];

        if key.is_empty() {
            break;
        }

        // Extract value: either quoted "..." or until next boundary
        let (value, rest) = if after_eq.starts_with('"') {
            // Quoted value: find closing quote
            if let Some(end_quote) = after_eq[1..].find('"') {
                (
                    &after_eq[1..end_quote + 1],
                    after_eq[end_quote + 2..].trim(),
                )
            } else {
                (after_eq[1..].trim(), "")
            }
        } else {
            // Unquoted: take until next `<` (XML tag), space-then-key=, or end
            let mut end = after_eq.len();

            // Stop at XML tags (e.g. `.</arg_value>` → take only `.`)
            if let Some(lt_pos) = after_eq.find('<') {
                end = lt_pos;
            }

            // Also look for next key=value pair (word followed by =)
            let search_range = &after_eq[..end];
            for (i, _) in search_range.char_indices() {
                if i > 0 && search_range.as_bytes()[i] == b' ' {
                    let rest_after_space = search_range[i..].trim();
                    if rest_after_space.contains('=') {
                        let next_eq = rest_after_space.find('=').unwrap();
                        let potential_key = rest_after_space[..next_eq].trim();
                        if !potential_key.is_empty()
                            && !potential_key.contains(' ')
                            && potential_key
                                .chars()
                                .all(|c| c.is_alphanumeric() || c == '_')
                        {
                            end = i;
                            break;
                        }
                    }
                }
            }
            let value = after_eq[..end].trim();
            let rest = if end < after_eq.len() {
                after_eq[end..].trim()
            } else {
                ""
            };
            (value, rest)
        };

        // Clean value: strip quotes and trailing XML junk
        let clean_value = value.trim_matches('"').trim_end_matches("/>");

        if !key.is_empty() && !clean_value.is_empty() {
            map.insert(
                key.to_string(),
                serde_json::Value::String(clean_value.to_string()),
            );
        }

        remaining = rest;
    }

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// Try to parse "key: value" style arguments.
///
/// Handles:
/// - `path: .` → `{"path": "."}`
/// - `(path: .)` → `{"path": "."}`  (strips parentheses)
/// - `(id: x, path: ".")` → `{"path": "."}` (takes last valid key)
fn parse_colon_args(text: &str) -> Option<serde_json::Value> {
    // Strip surrounding parentheses
    let text = text.trim();
    let text = if text.starts_with('(') && text.ends_with(')') {
        &text[1..text.len() - 1]
    } else {
        text
    };

    let mut map = serde_json::Map::new();

    // Split by comma for multi-value patterns like "id: x, path: ."
    for segment in text.split(',') {
        let segment = segment.trim();
        if let Some(colon_pos) = segment.find(':') {
            let key = segment[..colon_pos].trim();
            let value = segment[colon_pos + 1..].trim().trim_matches('"');

            // Key should be a simple identifier
            if !key.is_empty()
                && !value.is_empty()
                && !key.contains(' ')
                && key.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                map.insert(
                    key.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// Parse degenerate XML arg patterns where key=value is inside a single tag.
///
/// Handles: `<arg_key>path=.</arg_value>` or `<arg_key>path=.</arg_key>`
fn parse_degenerate_xml_args(text: &str) -> Option<serde_json::Value> {
    // Look for content between any XML-like tags that contains `=`
    let mut remaining = text;
    let mut map = serde_json::Map::new();

    while let Some(gt_pos) = remaining.find('>') {
        let after_tag = &remaining[gt_pos + 1..];
        // Find the next closing tag
        let end = after_tag.find('<').unwrap_or(after_tag.len());
        let inner = after_tag[..end].trim();

        if let Some(eq_pos) = inner.find('=') {
            let key = inner[..eq_pos].trim();
            let value = inner[eq_pos + 1..].trim().trim_matches('"');
            if !key.is_empty()
                && !value.is_empty()
                && key.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                map.insert(
                    key.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }

        if end < after_tag.len() {
            remaining = &after_tag[end..];
        } else {
            break;
        }
    }

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// Extract the text parts from a response that also contains tool calls.
///
/// Returns the text with ```tool_call``` markdown blocks,
/// `<tool_call>...</tool_call>` XML blocks, and `[TOOL] ...` bracket
/// patterns all removed.
pub fn extract_text_without_tool_calls(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;

    // Pass 1: Remove ```tool_call ... ``` markdown blocks
    while let Some(start) = remaining.find("```tool_call") {
        result.push_str(&remaining[..start]);

        let after_marker = &remaining[start + 12..];
        if let Some(end) = after_marker.find("```") {
            remaining = &after_marker[end + 3..];
        } else {
            result.push_str(&remaining[start..]);
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);

    // Pass 2: Remove <tool_call>...</tool_call> XML blocks
    let mut cleaned = result;
    loop {
        if let Some(start) = cleaned.find("<tool_call>") {
            if let Some(end_offset) = cleaned[start..].find("</tool_call>") {
                let before = &cleaned[..start];
                let after = &cleaned[start + end_offset + 12..];
                cleaned = format!("{}{}", before, after);
            } else {
                cleaned = cleaned[..start].to_string();
                break;
            }
        } else {
            break;
        }
    }

    // Pass 3: Remove [TOOL] ToolName(...) bracket patterns
    loop {
        let lower = cleaned.to_lowercase();
        if let Some(start) = lower.find("[tool]") {
            // Find the end of this tool call: next [TOOL] or end of line/text
            let after = &cleaned[start + 6..];
            // Look for closing `)` followed by optional `(id: ...)` and then either
            // next [TOOL] or end of text
            let end = if let Some(next_tool) = after.to_lowercase().find("[tool]") {
                start + 6 + next_tool
            } else {
                // Take everything from [TOOL] to end-of-line or end-of-text
                let newline = after.find('\n').unwrap_or(after.len());
                start + 6 + newline
            };
            let before = &cleaned[..start];
            let after = &cleaned[end..];
            cleaned = format!("{}{}", before, after);
        } else {
            break;
        }
    }

    // Pass 4: Remove direct XML tool blocks (`<ls>...</ls>`, `<read>...</read>`, etc.)
    for call in parse_direct_xml_tool_calls(&cleaned) {
        cleaned = cleaned.replacen(&call.raw_text, "", 1);
    }

    // Pass 5: Remove bare function-call tool lines (`LS(.)`, `Read(src/main.rs)`).
    for call in parse_bare_function_calls(&cleaned) {
        cleaned = cleaned.replacen(&call.raw_text, "", 1);
    }

    // Pass 6: Remove bare JSON tool calls and leftover labels.
    for call in parse_bare_json_tool_calls(&cleaned) {
        cleaned = cleaned.replacen(&call.raw_text, "", 1);
    }
    cleaned = cleaned
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.eq_ignore_ascii_case("tool_call:")
                && !trimmed.eq_ignore_ascii_case("tool_call")
        })
        .collect::<Vec<_>>()
        .join("\n");

    while cleaned.contains("\n\n\n") {
        cleaned = cleaned.replace("\n\n\n", "\n\n");
    }

    // Pass 7: Deduplicate repeated paragraphs.
    // LLMs using FallbackToolFormatMode often repeat their reasoning text
    // before/between/after tool call blocks.  After tool calls are stripped
    // (passes 1-6) the duplicate text remains.  Split by blank lines and
    // remove consecutive identical paragraphs.
    let paragraphs: Vec<&str> = cleaned.split("\n\n").collect();
    let mut deduped: Vec<&str> = Vec::with_capacity(paragraphs.len());
    for para in &paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip if this paragraph is identical to the previous one
        if let Some(prev) = deduped.last() {
            if prev.trim() == trimmed {
                continue;
            }
        }
        deduped.push(para);
    }
    cleaned = deduped.join("\n\n");

    cleaned.trim().to_string()
}

/// Format a tool result for injection back into the conversation as a user message.
pub fn format_tool_result(tool_name: &str, tool_id: &str, result: &str, is_error: bool) -> String {
    if is_error {
        format!(
            "[Tool Result: {} (id: {})]\nError: {}",
            tool_name, tool_id, result
        )
    } else {
        format!("[Tool Result: {} (id: {})]\n{}", tool_name, tool_id, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tools::definitions::get_tool_definitions_from_registry;

    #[test]
    fn test_build_tool_call_instructions() {
        let tools = get_tool_definitions_from_registry();
        let instructions = build_tool_call_instructions(&tools);

        assert!(instructions.contains("```tool_call"));
        assert!(instructions.contains("Read"));
        assert!(instructions.contains("Write"));
        assert!(instructions.contains("LS"));
        assert!(instructions.contains("Cwd"));
        assert!(instructions.contains("Available Tools"));
        // CodebaseSearch example should be included
        assert!(
            instructions.contains("CodebaseSearch"),
            "Should include CodebaseSearch example"
        );
        assert!(
            instructions.contains("\"scope\": \"all\""),
            "Should include scope=all in CodebaseSearch example"
        );
    }

    #[test]
    fn test_parse_single_tool_call() {
        let text = r#"Let me read the file.

```tool_call
{"tool": "Read", "arguments": {"file_path": "src/main.rs"}}
```

I'll analyze the contents."#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_parse_multiple_tool_calls() {
        let text = r#"Let me explore the project.

```tool_call
{"tool": "LS", "arguments": {"path": "."}}
```

And also check the files:

```tool_call
{"tool": "Glob", "arguments": {"pattern": "**/*.rs"}}
```

I'll analyze the results."#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[1].tool_name, "Glob");
    }

    #[test]
    fn test_parse_no_tool_calls() {
        let text = "This is just a regular response with no tool calls.";
        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let text = r#"```tool_call
{invalid json here}
```"#;

        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_missing_tool_field() {
        let text = r#"```tool_call
{"arguments": {"path": "."}}
```"#;

        let calls = parse_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_tool_call_no_arguments() {
        let text = r#"```tool_call
{"tool": "Cwd", "arguments": {}}
```"#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Cwd");
    }

    #[test]
    fn test_extract_text_without_tool_calls() {
        let text = r#"Let me read this file.

```tool_call
{"tool": "Read", "arguments": {"file_path": "test.rs"}}
```

I'll analyze the contents."#;

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("Let me read this file."));
        assert!(clean.contains("I'll analyze the contents."));
        assert!(!clean.contains("tool_call"));
        assert!(!clean.contains("Read"));
    }

    #[test]
    fn test_format_tool_result_success() {
        let result = format_tool_result("Read", "call_1", "file content here", false);
        assert!(result.contains("Tool Result: Read"));
        assert!(result.contains("file content here"));
        assert!(!result.contains("Error"));
    }

    #[test]
    fn test_format_tool_result_error() {
        let result = format_tool_result("Read", "call_1", "file not found", true);
        assert!(result.contains("Tool Result: Read"));
        assert!(result.contains("Error: file not found"));
    }

    // --- XML <tool_call> format tests ---

    #[test]
    fn test_parse_xml_tool_call_standard() {
        let text = r#"Let me list the files.

<tool_call>{"tool": "LS", "arguments": {"path": "."}}</tool_call>

I'll check the results."#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_xml_tool_call_name_only() {
        let text = "<tool_call>LS</tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert!(calls[0].arguments.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_xml_tool_call_multiple_tools_no_closing() {
        let text = "I'll explore the project.\n\n<tool_call>LS Cwd";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[1].tool_name, "Cwd");
    }

    #[test]
    fn test_parse_xml_tool_call_with_json_args() {
        let text = r#"<tool_call>Read {"file_path": "src/main.rs"}</tool_call>"#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_parse_mixed_markdown_and_xml() {
        let text = r#"First tool:

```tool_call
{"tool": "Read", "arguments": {"file_path": "a.rs"}}
```

Second tool:

<tool_call>{"tool": "LS", "arguments": {"path": "."}}</tool_call>

Done."#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[1].tool_name, "LS");
    }

    #[test]
    fn test_parse_multiple_xml_tool_calls() {
        let text = r#"<tool_call>{"tool": "LS", "arguments": {"path": "."}}</tool_call>

<tool_call>{"tool": "Cwd", "arguments": {}}</tool_call>"#;

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[1].tool_name, "Cwd");
    }

    #[test]
    fn test_extract_text_without_xml_tool_calls() {
        let text = r#"Let me check.

<tool_call>{"tool": "LS", "arguments": {"path": "."}}</tool_call>

Here are the results."#;

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("Let me check."));
        assert!(clean.contains("Here are the results."));
        assert!(!clean.contains("tool_call"));
        assert!(!clean.contains("LS"));
    }

    #[test]
    fn test_extract_text_without_xml_no_closing_tag() {
        let text = "Some text before.\n\n<tool_call>LS Cwd";

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("Some text before."));
        assert!(!clean.contains("tool_call"));
        assert!(!clean.contains("LS"));
    }

    #[test]
    fn test_extract_text_mixed_formats() {
        let text = r#"Start.

```tool_call
{"tool": "Read", "arguments": {"file_path": "a.rs"}}
```

Middle.

<tool_call>{"tool": "LS", "arguments": {"path": "."}}</tool_call>

End."#;

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("Start."));
        assert!(clean.contains("Middle."));
        assert!(clean.contains("End."));
        assert!(!clean.contains("tool_call"));
    }

    // --- XML <arg_key>/<arg_value> format tests ---

    #[test]
    fn test_parse_xml_arg_pairs_single() {
        let text = "<tool_call>LS<arg_key>path</arg_key><arg_value>.</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_xml_arg_pairs_multiple_params() {
        let text = "<tool_call>Read<arg_key>file_path</arg_key><arg_value>src/main.rs</arg_value><arg_key>offset</arg_key><arg_value>10</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
        assert_eq!(calls[0].arguments["offset"].as_str(), Some("10"));
    }

    #[test]
    fn test_parse_xml_arg_pairs_glob() {
        let text =
            "<tool_call>Glob<arg_key>pattern</arg_key><arg_value>**/*</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Glob");
        assert_eq!(calls[0].arguments["pattern"].as_str(), Some("**/*"));
    }

    #[test]
    fn test_parse_xml_arg_pairs_bash() {
        let text =
            "<tool_call>Bash<arg_key>command</arg_key><arg_value>ls -la</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Bash");
        assert_eq!(calls[0].arguments["command"].as_str(), Some("ls -la"));
    }

    #[test]
    fn test_parse_xml_arg_pairs_with_surrounding_text() {
        let text = "让我先看看项目的整体结构。\n\n<tool_call>LS<arg_key>path</arg_key><arg_value>.</arg_value></tool_call>\n\n我来分析结果。";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_colon_args() {
        let text = "<tool_call>LS path: .</tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_extract_text_without_xml_arg_pairs() {
        let text = "让我查看。\n\n<tool_call>LS<arg_key>path</arg_key><arg_value>.</arg_value></tool_call>\n\n结果如下。";

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("让我查看。"));
        assert!(clean.contains("结果如下。"));
        assert!(!clean.contains("tool_call"));
        assert!(!clean.contains("arg_key"));
    }

    #[test]
    fn test_parse_xml_arg_pairs_fn() {
        let args = parse_xml_arg_pairs("<arg_key>path</arg_key><arg_value>.</arg_value>");
        assert_eq!(args["path"].as_str(), Some("."));

        let args = parse_xml_arg_pairs(
            "<arg_key>file_path</arg_key><arg_value>a.rs</arg_value><arg_key>offset</arg_key><arg_value>5</arg_value>",
        );
        assert_eq!(args["file_path"].as_str(), Some("a.rs"));
        assert_eq!(args["offset"].as_str(), Some("5"));
    }

    // --- Malformed tag tests (model uses <tool_call> instead of <arg_key>) ---

    #[test]
    fn test_parse_malformed_tool_call_as_arg_key() {
        // Real-world pattern: model outputs <tool_call> instead of <arg_key>
        let text = "<tool_call>LS<tool_call>path</arg_key><arg_value>.</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_malformed_read_tool_call_as_arg_key() {
        // Read with malformed opening tag
        let text = "<tool_call>Read<tool_call>file_path</arg_key><arg_value>src/main.rs</arg_value></tool_call>";

        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_parse_xml_arg_pairs_malformed_opening_tag() {
        // The parse_xml_arg_pairs function should handle <tool_call> instead of <arg_key>
        let args = parse_xml_arg_pairs("<tool_call>path</arg_key><arg_value>.</arg_value>");
        assert_eq!(args["path"].as_str(), Some("."));
    }

    #[test]
    fn test_extract_text_with_malformed_tags() {
        let text = "让我查看。\n\n<tool_call>LS<tool_call>path</arg_key><arg_value>.</arg_value></tool_call>\n\n结果。";

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("让我查看。"));
        assert!(clean.contains("结果。"));
        assert!(!clean.contains("tool_call"));
        assert!(!clean.contains("arg_key"));
    }

    // --- Equals-separated key=value tests (real model output patterns) ---

    #[test]
    fn test_parse_equals_simple() {
        // <tool_call>LS path=.</tool_call>
        let text = r#"<tool_call>LS path=.</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_equals_quoted() {
        // <tool_call>LS path="."</tool_call>
        let text = r#"<tool_call>LS path="."</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_equals_full_path() {
        // <tool_call>LS path=D:\VsCodeProjects\cc-sync</tool_call>
        let text = r#"<tool_call>LS path=D:\VsCodeProjects\cc-sync</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(
            calls[0].arguments["path"].as_str(),
            Some(r"D:\VsCodeProjects\cc-sync")
        );
    }

    #[test]
    fn test_parse_equals_multiple_params() {
        // <tool_call>LS verbosity=verbose path=.</tool_call>
        let text = r#"<tool_call>LS verbosity=verbose path=.</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_equals_json_value() {
        // <tool_call>LS path={"path": "."}</tool_call>
        let text = r#"<tool_call>LS path={"path": "."}</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_read_equals() {
        // <tool_call>Read file_path="README.md"</tool_call>
        let text = r#"<tool_call>Read file_path="README.md"</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].arguments["file_path"].as_str(), Some("README.md"));
    }

    // --- Colon-separated with parentheses ---

    #[test]
    fn test_parse_colon_parens() {
        // <tool_call>LS (path: .)</tool_call>
        let text = r#"<tool_call>LS (path: .)</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_colon_parens_with_path() {
        // <tool_call>LS (path: D:\VsCodeProjects\cc-sync)</tool_call>
        let text = r#"<tool_call>LS (path: D:\VsCodeProjects\cc-sync)</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(
            calls[0].arguments["path"].as_str(),
            Some(r"D:\VsCodeProjects\cc-sync")
        );
    }

    #[test]
    fn test_parse_colon_parens_multi() {
        // (id: fallback_9, path=".")
        let text = r#"<tool_call>LS (id: fallback_9, path: ".")</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    // --- Degenerate XML: <arg_key>key=value</arg_value> ---

    #[test]
    fn test_parse_degenerate_xml() {
        // <tool_call>LS<arg_key>path=.</arg_value></tool_call>
        let text = r#"<tool_call>LS<arg_key>path=.</arg_value></tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    // --- Self-closing XML style ---

    #[test]
    fn test_parse_self_closing_xml() {
        // <tool_call>Cwd cwd_id="1" /></tool_call>
        let text = r#"<tool_call>Cwd cwd_id="1" /></tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Cwd");
        // Cwd has no required params, so empty or with extra attrs is fine
    }

    // --- parse_equals_args unit tests ---

    #[test]
    fn test_parse_equals_args_fn() {
        let args = parse_equals_args("path=.").unwrap();
        assert_eq!(args["path"].as_str(), Some("."));

        let args = parse_equals_args(r#"path=".""#).unwrap();
        assert_eq!(args["path"].as_str(), Some("."));

        let args = parse_equals_args("file_path=src/main.rs").unwrap();
        assert_eq!(args["file_path"].as_str(), Some("src/main.rs"));

        assert!(parse_equals_args("no equals here").is_none());
    }

    #[test]
    fn test_parse_colon_args_fn() {
        let args = parse_colon_args("path: .").unwrap();
        assert_eq!(args["path"].as_str(), Some("."));

        let args = parse_colon_args("(path: .)").unwrap();
        assert_eq!(args["path"].as_str(), Some("."));

        let args = parse_colon_args(r#"(id: x, path: ".")"#).unwrap();
        assert_eq!(args["path"].as_str(), Some("."));
        assert_eq!(args["id"].as_str(), Some("x"));
    }

    // --- [TOOL] bracket format tests ---

    #[test]
    fn test_parse_bracket_single_read() {
        let text = "[TOOL] Read(README.md) (id: fallback_5)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].arguments["file_path"].as_str(), Some("README.md"));
    }

    #[test]
    fn test_parse_bracket_multiple_reads() {
        let text = "[TOOL] Read(README.zh-CN.md) (id: fallback_5) [TOOL] Read(README.md) (id: fallback_6) [TOOL] Read(task_plan.md) (id: fallback_7)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("README.zh-CN.md")
        );
        assert_eq!(calls[1].tool_name, "Read");
        assert_eq!(calls[1].arguments["file_path"].as_str(), Some("README.md"));
        assert_eq!(calls[2].tool_name, "Read");
        assert_eq!(
            calls[2].arguments["file_path"].as_str(),
            Some("task_plan.md")
        );
    }

    #[test]
    fn test_parse_bracket_ls() {
        let text = "[TOOL] LS(.)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_parse_bracket_glob() {
        let text = "[TOOL] Glob(*.rs)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Glob");
        assert_eq!(calls[0].arguments["pattern"].as_str(), Some("*.rs"));
    }

    #[test]
    fn test_parse_bracket_bash() {
        let text = "[TOOL] Bash(cargo test)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Bash");
        assert_eq!(calls[0].arguments["command"].as_str(), Some("cargo test"));
    }

    #[test]
    fn test_parse_bracket_with_surrounding_text() {
        let text = "让我读取文件来分析项目。\n\n[TOOL] Read(README.md) (id: fallback_5) [TOOL] Read(src/main.rs) (id: fallback_6)\n";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].arguments["file_path"].as_str(), Some("README.md"));
        assert_eq!(calls[1].tool_name, "Read");
        assert_eq!(
            calls[1].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_parse_bracket_case_insensitive() {
        let text = "[Tool] Read(test.rs)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
    }

    #[test]
    fn test_extract_text_without_bracket_tool_calls() {
        let text = "让我分析。\n\n[TOOL] Read(README.md) (id: fallback_5) [TOOL] Read(src/main.rs) (id: fallback_6)";
        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("让我分析。"));
        assert!(!clean.contains("[TOOL]"));
        assert!(!clean.contains("Read"));
    }

    #[test]
    fn test_infer_tool_arguments() {
        let args = infer_tool_arguments("Read", "test.rs");
        assert_eq!(args["file_path"].as_str(), Some("test.rs"));

        let args = infer_tool_arguments("LS", ".");
        assert_eq!(args["path"].as_str(), Some("."));

        let args = infer_tool_arguments("Glob", "*.rs");
        assert_eq!(args["pattern"].as_str(), Some("*.rs"));

        let args = infer_tool_arguments("Bash", "cargo build");
        assert_eq!(args["command"].as_str(), Some("cargo build"));

        let args = infer_tool_arguments("Cwd", "");
        assert!(args.as_object().unwrap().is_empty());
    }

    // --- normalize_tool_arguments tests ---

    #[test]
    fn test_normalize_read_path_to_file_path() {
        let mut args = serde_json::json!({"path": "README.md"});
        normalize_tool_arguments("Read", &mut args);
        assert_eq!(args["file_path"].as_str(), Some("README.md"));
        assert!(args.get("path").is_none());
    }

    #[test]
    fn test_normalize_does_not_overwrite_canonical() {
        // If file_path already exists, don't overwrite it with path's value
        let mut args = serde_json::json!({"file_path": "a.rs", "path": "b.rs"});
        normalize_tool_arguments("Read", &mut args);
        assert_eq!(args["file_path"].as_str(), Some("a.rs"));
    }

    #[test]
    fn test_normalize_ls_directory_to_path() {
        let mut args = serde_json::json!({"directory": "/src"});
        normalize_tool_arguments("LS", &mut args);
        assert_eq!(args["path"].as_str(), Some("/src"));
    }

    #[test]
    fn test_normalize_bash_cmd_to_command() {
        let mut args = serde_json::json!({"cmd": "ls -la"});
        normalize_tool_arguments("Bash", &mut args);
        assert_eq!(args["command"].as_str(), Some("ls -la"));
    }

    #[test]
    fn test_normalize_no_op_for_correct_params() {
        // When canonical names are already used, nothing changes
        let mut args = serde_json::json!({"file_path": "test.rs"});
        normalize_tool_arguments("Read", &mut args);
        assert_eq!(args["file_path"].as_str(), Some("test.rs"));
    }

    #[test]
    fn test_normalize_no_op_for_unknown_tool() {
        let mut args = serde_json::json!({"path": "test.rs"});
        normalize_tool_arguments("UnknownTool", &mut args);
        // Should not change anything
        assert_eq!(args["path"].as_str(), Some("test.rs"));
    }

    #[test]
    fn test_normalize_xml_parsed_read_with_path() {
        // Simulates the exact bug from the user's log:
        // <tool_call>Read<arg_key>path</arg_key><arg_value>README.zh-CN.md</arg_value></tool_call>
        let text = "<tool_call>Read<arg_key>path</arg_key><arg_value>README.zh-CN.md</arg_value></tool_call>";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        // After normalization, "path" should be renamed to "file_path"
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("README.zh-CN.md")
        );
    }

    #[test]
    fn test_normalize_write_filepath_variant() {
        let mut args = serde_json::json!({"filepath": "out.txt", "content": "hello"});
        normalize_tool_arguments("Write", &mut args);
        assert_eq!(args["file_path"].as_str(), Some("out.txt"));
        assert_eq!(args["content"].as_str(), Some("hello"));
    }

    // --- GLM garbled format tests ---

    #[test]
    fn test_glm_read_return_parens() {
        // GLM writes `ReadReturn("pyproject.toml")` — garbled tool name with function-call style args
        let text = r#"<tool_call>ReadReturn("pyproject.toml")</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("pyproject.toml")
        );
    }

    #[test]
    fn test_glm_duplicate_tool_name() {
        // GLM writes `ReadRead file_path="pyproject.toml"` — duplicated tool name
        let text = r#"<tool_call>ReadRead file_path="pyproject.toml"</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("pyproject.toml")
        );
    }

    #[test]
    fn test_glm_ls_with_copied_id_prefix() {
        // GLM copies result format: `LS (id: story_fallback_6){"path": "src"}`
        let text = r#"<tool_call>LS (id: story_fallback_6){"path": "src"}</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("src"));
    }

    #[test]
    fn test_glm_ls_with_id_colon_no_space() {
        // Variant: `(id:fallback_3){"path": "."}`
        let text = r#"<tool_call>LS (id:fallback_3){"path": "."}</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_glm_read_parens_bare() {
        // `Read("src/main.rs")` — function-call style without junk prefix
        let text = r#"<tool_call>Read("src/main.rs")</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_glm_ls_parens() {
        // `LS("src")` — function-call style
        let text = r#"<tool_call>LS("src")</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("src"));
    }

    #[test]
    fn test_glm_grep_parens_unquoted() {
        // `Grep(struct)` — function-call style without quotes
        let text = r#"<tool_call>Grep(struct)</tool_call>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Grep");
        assert_eq!(calls[0].arguments["pattern"].as_str(), Some("struct"));
    }

    // ---- Pass 4: Direct XML tool call tests ----

    #[test]
    fn test_direct_xml_ls_lowercase() {
        // GLM generates <ls><path>.</path></ls>
        let text = r#"I'll explore the project.

<ls>
<path>D:\VsCodeProjects\planning-with-files</path>
</ls>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1, "Should parse 1 tool call, got: {:?}", calls);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(
            calls[0].arguments["path"].as_str(),
            Some(r"D:\VsCodeProjects\planning-with-files")
        );
    }

    #[test]
    fn test_direct_xml_multiple_tools() {
        // Multiple direct XML tool calls in one response
        let text = r#"Let me explore.

<ls><path>.</path></ls>

<glob><pattern>**/{package.json,Cargo.toml,pyproject.toml}</pattern></glob>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(
            calls.len(),
            2,
            "Should parse 2 tool calls, got: {:?}",
            calls
        );
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
        assert_eq!(calls[1].tool_name, "Glob");
        assert_eq!(
            calls[1].arguments["pattern"].as_str(),
            Some("**/{package.json,Cargo.toml,pyproject.toml}")
        );
    }

    #[test]
    fn test_direct_xml_read_with_file_path() {
        let text = r#"<read><file_path>src/main.rs</file_path></read>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_direct_xml_grep_with_multiple_params() {
        let text = r#"<grep>
<pattern>class.*</pattern>
<path>src/</path>
</grep>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Grep");
        assert_eq!(calls[0].arguments["pattern"].as_str(), Some("class.*"));
        assert_eq!(calls[0].arguments["path"].as_str(), Some("src/"));
    }

    #[test]
    fn test_direct_xml_mixed_case() {
        // Tool names in various cases should all be recognized
        let text = r#"<Read><file_path>test.txt</file_path></Read>"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(calls[0].arguments["file_path"].as_str(), Some("test.txt"));
    }

    #[test]
    fn test_direct_xml_does_not_conflict_with_other_passes() {
        // If ```tool_call is present, pass 1 should handle it and pass 4 should not run
        let text =
            "```tool_call\n{\"tool\": \"Read\", \"arguments\": {\"file_path\": \"a.rs\"}}\n```";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
    }

    #[test]
    fn test_nested_xml_args_parser() {
        let content = r#"<file_path>src/main.rs</file_path><offset>10</offset>"#;
        let args = parse_nested_xml_args(content);
        assert_eq!(args["file_path"].as_str(), Some("src/main.rs"));
        assert_eq!(args["offset"].as_str(), Some("10"));
    }

    // ---- Pass 5: Bare function-call tests ----

    #[test]
    fn test_bare_function_ls_windows_path() {
        let text = r"LS(D:\VsCodeProjects\planning-with-files)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(
            calls[0].arguments["path"].as_str(),
            Some(r"D:\VsCodeProjects\planning-with-files")
        );
    }

    #[test]
    fn test_bare_function_read() {
        let text = "I need to check the file.\nRead(src/main.rs)\nDone.";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_bare_function_case_insensitive() {
        let text = "ls(.)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[0].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_bare_function_no_match_inside_word() {
        // "ReadFile(x)" should NOT match — "Read" is part of "ReadFile"
        let text = "ReadFile(something)";
        let calls = parse_bare_function_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_bare_function_no_match_mid_sentence() {
        // Tool call embedded in a sentence should NOT match (prevents re-execution loops)
        let text = "I used LS(.) to explore the project";
        let calls = parse_bare_function_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_bare_function_multiple_lines() {
        // Each tool call on its own line should match
        let text = "LS(.)\nRead(src/main.rs)";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "LS");
        assert_eq!(calls[1].tool_name, "Read");
    }

    // ---- Pass 6: Bare JSON tool call tests ----

    #[test]
    fn test_bare_json_tool_call() {
        let text = r#"{"tool": "Grep", "arguments": {"pattern": "class"}}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Grep");
        assert_eq!(calls[0].arguments["pattern"].as_str(), Some("class"));
    }

    #[test]
    fn test_bare_json_with_tool_call_prefix() {
        let text = "I'll search now.\ntool_call:\n{\"tool\": \"Grep\", \"arguments\": {\"pattern\": \"class |export\"}}";
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Grep");
        assert_eq!(
            calls[0].arguments["pattern"].as_str(),
            Some("class |export")
        );
    }

    #[test]
    fn test_bare_json_multiline() {
        let text = r#"tool_call:
{
  "tool": "Read",
  "arguments": {
    "file_path": "src/main.rs"
  }
}"#;
        let calls = parse_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "Read");
        assert_eq!(
            calls[0].arguments["file_path"].as_str(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn test_bare_json_no_false_positive() {
        // Regular JSON that doesn't have "tool" key should not match
        let text = r#"{"name": "test", "value": 42}"#;
        let calls = parse_bare_json_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_bare_json_skips_unclosed_tool_call_fence() {
        let text = r#"```tool_call
{"tool": "Read", "arguments": {"file_path": "test.txt"}}"#;
        let calls = parse_bare_json_tool_calls(text);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_lenient_chained_tools_with_json_args() {
        let text = r#"<tool_call>Cwd LS {"path": "."}</tool_call>"#;
        let calls = parse_tool_calls(text);

        assert_eq!(calls.len(), 2, "calls = {:?}", calls);
        assert_eq!(calls[0].tool_name, "Cwd");
        assert_eq!(calls[1].tool_name, "LS");
        assert_eq!(calls[1].arguments["path"].as_str(), Some("."));
    }

    #[test]
    fn test_extract_text_without_tool_calls_removes_bare_and_xml_variants() {
        let text = r#"I'll inspect first.
LS(.)
tool_call:
{"tool": "Read", "arguments": {"file_path": "src/main.rs"}}
<ls><path>.</path></ls>
Done."#;

        let clean = extract_text_without_tool_calls(text);
        assert!(clean.contains("I'll inspect first."));
        assert!(clean.contains("Done."));
        assert!(!clean.contains("LS(.)"));
        assert!(!clean.contains("\"tool\": \"Read\""));
        assert!(!clean.contains("<ls>"));
        assert!(!clean.contains("tool_call"));
    }
}
