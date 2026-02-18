//! Prompt-Based Tool Calling Fallback
//!
//! For LLM providers that don't support native function/tool calling (e.g., Ollama),
//! this module injects tool descriptions into the system prompt and parses tool call
//! blocks from the LLM's text responses.

use serde::{Deserialize, Serialize};

use plan_cascade_llm::types::ToolDefinition;

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
/// - Pass 4: `<ToolName><param>value</param></ToolName>` direct XML blocks
/// - Pass 5: Bare function-call style `ToolName(args)`
/// - Pass 6: Bare JSON `{"tool": "Name", "arguments": {...}}`
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
    if calls.is_empty() {
        calls.extend(parse_direct_xml_tool_calls(text));
    }

    // Pass 5: Bare function-call style — ToolName(args) without any wrapper.
    if calls.is_empty() {
        calls.extend(parse_bare_function_calls(text));
    }

    // Pass 6: Bare JSON tool calls — {"tool": "Name", "arguments": {...}}
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
fn parse_direct_xml_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();
    let lower = text.to_lowercase();

    for tool in KNOWN_TOOLS {
        let lower_tool = tool.to_lowercase();
        let open_tag = format!("<{}", lower_tool);
        let close_tag = format!("</{}>", lower_tool);
        let mut search_pos = 0;

        while search_pos < lower.len() {
            let start = match lower[search_pos..].find(&open_tag) {
                Some(p) => search_pos + p,
                None => break,
            };

            let after_open = start + open_tag.len();
            if after_open >= text.len() {
                break;
            }
            let tag_end = match text[after_open..].find('>') {
                Some(p) => after_open + p + 1,
                None => break,
            };

            if let Some(close_rel) = lower[tag_end..].find(&close_tag) {
                let content = text[tag_end..tag_end + close_rel].trim();
                let raw_end = tag_end + close_rel + close_tag.len();

                let args = parse_nested_xml_args(content);

                let args = if args.as_object().map_or(true, |m| m.is_empty()) && !content.is_empty()
                {
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

    calls.sort_by_key(|c| text.find(&c.raw_text).unwrap_or(usize::MAX));

    calls
}

/// Parse bare function-call style tool calls: `ToolName(args)`.
///
/// Only matches when the tool call appears at the **start of a line**.
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
                break;
            }
        }
    }

    calls
}

/// Parse bare JSON tool calls: `{"tool": "Name", "arguments": {...}}`.
fn parse_bare_json_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    if has_unclosed_tool_call_fence(text) {
        return Vec::new();
    }

    let mut calls = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        let json_start_line =
            if line.eq_ignore_ascii_case("tool_call:") || line.eq_ignore_ascii_case("tool_call") {
                i += 1;
                if i < lines.len() {
                    Some(i)
                } else {
                    None
                }
            } else if line.starts_with('{') && line.contains("\"tool\"") {
                Some(i)
            } else {
                None
            };

        if let Some(start_line) = json_start_line {
            let first = lines[start_line].trim();
            if first.starts_with('{') {
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
fn parse_nested_xml_args(content: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut remaining = content;

    while let Some(open_start) = remaining.find('<') {
        let after_open = &remaining[open_start + 1..];

        if after_open.starts_with('/') {
            if let Some(gt) = after_open.find('>') {
                remaining = &after_open[gt + 1..];
            } else {
                break;
            }
            continue;
        }

        if let Some(open_end) = after_open.find('>') {
            let tag_name = after_open[..open_end].trim();
            if tag_name.is_empty() {
                remaining = &after_open[open_end + 1..];
                continue;
            }

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
fn infer_tool_arguments(tool_name: &str, bare_value: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    if bare_value.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(bare_value) {
            return json;
        }
    }

    if bare_value.contains('=') {
        if let Some(args) = parse_equals_args(bare_value) {
            return args;
        }
    }
    if bare_value.contains(':') {
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
fn normalize_tool_arguments(tool_name: &str, args: &mut serde_json::Value) {
    let map = match args.as_object_mut() {
        Some(m) => m,
        None => return,
    };

    let renames: &[(&str, &str, &str)] = &[
        ("Read", "path", "file_path"),
        ("Read", "filepath", "file_path"),
        ("Read", "file", "file_path"),
        ("Write", "path", "file_path"),
        ("Write", "filepath", "file_path"),
        ("Write", "file", "file_path"),
        ("Edit", "path", "file_path"),
        ("Edit", "filepath", "file_path"),
        ("Edit", "file", "file_path"),
        ("LS", "directory", "path"),
        ("LS", "dir", "path"),
        ("Glob", "glob", "pattern"),
        ("Grep", "search", "pattern"),
        ("Grep", "regex", "pattern"),
        ("Bash", "cmd", "command"),
        ("WebFetch", "link", "url"),
        ("WebFetch", "address", "url"),
        ("WebSearch", "search", "query"),
        ("WebSearch", "q", "query"),
        ("Analyze", "scope", "mode"),
        ("Analyze", "focus", "path_hint"),
        ("Analyze", "path", "path_hint"),
        ("Analyze", "file", "path_hint"),
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
fn parse_single_tool_call(content: &str) -> Option<(String, serde_json::Value)> {
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
fn parse_lenient_tool_calls(content: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    for tool in KNOWN_TOOLS {
        if content.starts_with(tool) {
            let rest = content[tool.len()..].trim();

            let rest = if rest.starts_with(tool) {
                rest[tool.len()..].trim()
            } else {
                rest
            };

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

            if rest.is_empty() {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments: serde_json::Value::Object(serde_json::Map::new()),
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

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

            if let Some(arguments) = parse_equals_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            if let Some(arguments) = parse_colon_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

            if let Some(arguments) = parse_degenerate_xml_args(rest) {
                calls.push(ParsedToolCall {
                    tool_name: tool.to_string(),
                    arguments,
                    raw_text: format!("<tool_call>{}</tool_call>", content),
                });
                return calls;
            }

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
fn parse_xml_arg_pairs(text: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut remaining = text;

    while let Some(close_key_pos) = remaining.find("</arg_key>") {
        let before_close = &remaining[..close_key_pos];
        let key = if let Some(last_gt) = before_close.rfind('>') {
            before_close[last_gt + 1..].trim()
        } else {
            before_close.trim()
        };

        let after_close_key = &remaining[close_key_pos + 10..];

        if let Some(val_start) = after_close_key.find("<arg_value>") {
            let after_val_tag = &after_close_key[val_start + 11..];
            if let Some(val_end) = after_val_tag.find("</arg_value>") {
                let value = after_val_tag[..val_end].trim();
                if !key.is_empty() {
                    map.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
                remaining = &after_val_tag[val_end + 12..];
                continue;
            }
        }
        break;
    }

    serde_json::Value::Object(map)
}

/// Parse `key=value` style arguments.
fn parse_equals_args(text: &str) -> Option<serde_json::Value> {
    if !text.contains('=') {
        return None;
    }

    if let Some(eq_brace) = text.find("={") {
        let json_str = &text[eq_brace + 1..];
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
            if json.is_object() {
                return Some(json);
            }
        }
    }

    let mut map = serde_json::Map::new();
    let mut remaining = text.trim();

    while !remaining.is_empty() {
        if remaining.starts_with('<') {
            if let Some(gt) = remaining.find('>') {
                remaining = remaining[gt + 1..].trim();
                continue;
            }
            break;
        }

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

        let (value, rest) = if after_eq.starts_with('"') {
            if let Some(end_quote) = after_eq[1..].find('"') {
                (
                    &after_eq[1..end_quote + 1],
                    after_eq[end_quote + 2..].trim(),
                )
            } else {
                (after_eq[1..].trim(), "")
            }
        } else {
            let mut end = after_eq.len();

            if let Some(lt_pos) = after_eq.find('<') {
                end = lt_pos;
            }

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
fn parse_colon_args(text: &str) -> Option<serde_json::Value> {
    let text = text.trim();
    let text = if text.starts_with('(') && text.ends_with(')') {
        &text[1..text.len() - 1]
    } else {
        text
    };

    let mut map = serde_json::Map::new();

    for segment in text.split(',') {
        let segment = segment.trim();
        if let Some(colon_pos) = segment.find(':') {
            let key = segment[..colon_pos].trim();
            let value = segment[colon_pos + 1..].trim().trim_matches('"');

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
fn parse_degenerate_xml_args(text: &str) -> Option<serde_json::Value> {
    let mut remaining = text;
    let mut map = serde_json::Map::new();

    while let Some(gt_pos) = remaining.find('>') {
        let after_tag = &remaining[gt_pos + 1..];
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
/// Returns the text with tool call blocks removed.
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
            let after = &cleaned[start + 6..];
            let end = if let Some(next_tool) = after.to_lowercase().find("[tool]") {
                start + 6 + next_tool
            } else {
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

    // Pass 4: Remove direct XML tool blocks
    for call in parse_direct_xml_tool_calls(&cleaned) {
        cleaned = cleaned.replacen(&call.raw_text, "", 1);
    }

    // Pass 5: Remove bare function-call tool lines
    for call in parse_bare_function_calls(&cleaned) {
        cleaned = cleaned.replacen(&call.raw_text, "", 1);
    }

    // Pass 6: Remove bare JSON tool calls and leftover labels
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

    // Pass 7: Deduplicate repeated paragraphs
    let paragraphs: Vec<&str> = cleaned.split("\n\n").collect();
    let mut deduped: Vec<&str> = Vec::with_capacity(paragraphs.len());
    for para in &paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
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
    fn test_normalize_read_path_alias() {
        let mut args = serde_json::json!({"path": "src/main.rs"});
        normalize_tool_arguments("Read", &mut args);
        assert_eq!(args["file_path"].as_str(), Some("src/main.rs"));
        assert!(args.get("path").is_none());
    }

    #[test]
    fn test_normalize_no_conflict() {
        let mut args = serde_json::json!({"file_path": "existing.rs", "path": "should_remain"});
        normalize_tool_arguments("Read", &mut args);
        // file_path already present, so path should NOT be renamed
        assert_eq!(args["file_path"].as_str(), Some("existing.rs"));
    }
}
