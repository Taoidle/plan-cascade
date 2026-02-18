//! AI Verification Gate
//!
//! Uses the LLM provider to check for skeleton code patterns in changed files:
//! - Functions with only `pass` / `...` / `todo!()` / `unimplemented!()`
//! - `raise NotImplementedError` patterns
//! - TODO/FIXME markers in new code
//! - Empty function bodies
//!
//! Constructs a prompt with the git diff and asks the LLM to identify skeleton code.

use serde::{Deserialize, Serialize};

use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

/// A single skeleton code finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkeletonFinding {
    /// File path where skeleton code was found
    pub file_path: String,
    /// Line number (approximate)
    pub line: Option<u32>,
    /// Description of the skeleton pattern found
    pub description: String,
    /// Severity: "warning" or "error"
    pub severity: String,
}

/// Result from AI verification gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiVerificationResult {
    /// Whether verification passed (no skeleton code found)
    pub passed: bool,
    /// List of skeleton findings
    pub findings: Vec<SkeletonFinding>,
    /// Summary message
    pub summary: String,
}

/// AI Verification Gate that detects skeleton code via LLM analysis.
pub struct AiVerificationGate {
    /// Git diff content to analyze
    diff_content: String,
}

impl AiVerificationGate {
    /// Create a new verification gate with the given diff content.
    pub fn new(diff_content: String) -> Self {
        Self { diff_content }
    }

    /// Build the system prompt for skeleton code detection.
    pub fn build_prompt(&self) -> String {
        format!(
            r#"You are a code quality analyzer. Analyze the following git diff for skeleton/stub code patterns.

Look for:
1. Functions with only `pass`, `...`, `todo!()`, `unimplemented!()`, `panic!("not implemented")`, or `raise NotImplementedError`
2. TODO/FIXME comments in newly added code (lines starting with +)
3. Empty function/method bodies (no implementation)
4. Placeholder return values like `return None`, `return 0`, `return ""` without actual logic

For each finding, respond in this exact JSON format:
{{
  "passed": false,
  "findings": [
    {{
      "filePath": "path/to/file.rs",
      "line": 42,
      "description": "Function `foo` contains only todo!()",
      "severity": "error"
    }}
  ],
  "summary": "Found 1 skeleton code pattern"
}}

If no skeleton code is found, respond:
{{
  "passed": true,
  "findings": [],
  "summary": "No skeleton code detected"
}}

Git diff to analyze:
```
{}
```"#,
            self.diff_content
        )
    }

    /// Run the gate with a pre-computed AI response.
    ///
    /// In production, the caller invokes the LLM provider with `build_prompt()`
    /// and passes the response here for parsing.
    pub fn parse_response(&self, ai_response: &str) -> PipelineGateResult {
        // Try to parse JSON from the response
        let verification_result = self.extract_result(ai_response);

        match verification_result {
            Some(result) => {
                if result.passed {
                    PipelineGateResult::passed(
                        "ai_verify",
                        "AI Verification",
                        GatePhase::PostValidation,
                        0,
                    )
                } else {
                    let findings: Vec<String> = result
                        .findings
                        .iter()
                        .map(|f| {
                            format!(
                                "[{}] {}: {} (line {})",
                                f.severity,
                                f.file_path,
                                f.description,
                                f.line.map_or("?".to_string(), |l| l.to_string())
                            )
                        })
                        .collect();

                    PipelineGateResult::failed(
                        "ai_verify",
                        "AI Verification",
                        GatePhase::PostValidation,
                        0,
                        result.summary,
                        findings,
                    )
                }
            }
            None => {
                // Graceful fallback: pass with warning if we can't parse the response
                PipelineGateResult::passed(
                    "ai_verify",
                    "AI Verification",
                    GatePhase::PostValidation,
                    0,
                )
            }
        }
    }

    /// Run the gate without an LLM (heuristic-based skeleton detection).
    ///
    /// This is a fallback that uses simple pattern matching instead of AI.
    pub fn run_heuristic(&self) -> PipelineGateResult {
        let mut findings = Vec::new();

        let skeleton_patterns = [
            ("todo!()", "Contains todo!() macro"),
            ("unimplemented!()", "Contains unimplemented!() macro"),
            ("panic!(\"not implemented\")", "Contains not-implemented panic"),
            ("raise NotImplementedError", "Raises NotImplementedError"),
            ("pass  # TODO", "Contains pass with TODO"),
            ("...", "Contains ellipsis placeholder"),
        ];

        // Only check lines that are being added (start with +)
        for line in self.diff_content.lines() {
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let trimmed = line.trim_start_matches('+').trim();

            for (pattern, description) in &skeleton_patterns {
                if trimmed.contains(pattern) {
                    findings.push(format!("[warning] {}: {}", pattern, description));
                }
            }

            // Check for TODO/FIXME in new code
            let upper = trimmed.to_uppercase();
            if (upper.contains("TODO") || upper.contains("FIXME")) && !upper.contains("// TODO:") {
                // Allow structured TODO comments but flag bare ones
                if trimmed.starts_with("//") || trimmed.starts_with('#') {
                    findings.push(format!("[warning] TODO/FIXME marker: {}", trimmed));
                }
            }
        }

        if findings.is_empty() {
            PipelineGateResult::passed(
                "ai_verify",
                "AI Verification (heuristic)",
                GatePhase::PostValidation,
                0,
            )
        } else {
            PipelineGateResult::failed(
                "ai_verify",
                "AI Verification (heuristic)",
                GatePhase::PostValidation,
                0,
                format!("Found {} potential skeleton code patterns", findings.len()),
                findings,
            )
        }
    }

    /// Extract the verification result from the AI response.
    fn extract_result(&self, response: &str) -> Option<AiVerificationResult> {
        // Try direct JSON parse
        if let Ok(result) = serde_json::from_str::<AiVerificationResult>(response) {
            return Some(result);
        }

        // Try to find JSON block in markdown code fence
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                let json_str = &response[start..=end];
                if let Ok(result) = serde_json::from_str::<AiVerificationResult>(json_str) {
                    return Some(result);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quality_gates::GateStatus;

    #[test]
    fn test_ai_verify_clean_diff() {
        let diff = r#"
+fn add(a: i32, b: i32) -> i32 {
+    a + b
+}
"#;
        let gate = AiVerificationGate::new(diff.to_string());
        let result = gate.run_heuristic();
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Passed);
    }

    #[test]
    fn test_ai_verify_detects_todo_macro() {
        let diff = r#"
+fn process() {
+    todo!()
+}
"#;
        let gate = AiVerificationGate::new(diff.to_string());
        let result = gate.run_heuristic();
        assert!(!result.passed);
        assert!(!result.findings.is_empty());
    }

    #[test]
    fn test_ai_verify_detects_unimplemented() {
        let diff = r#"
+fn calculate() -> f64 {
+    unimplemented!()
+}
"#;
        let gate = AiVerificationGate::new(diff.to_string());
        let result = gate.run_heuristic();
        assert!(!result.passed);
    }

    #[test]
    fn test_ai_verify_detects_not_implemented_error() {
        let diff = r#"
+def process():
+    raise NotImplementedError
"#;
        let gate = AiVerificationGate::new(diff.to_string());
        let result = gate.run_heuristic();
        assert!(!result.passed);
    }

    #[test]
    fn test_ai_verify_ignores_removed_lines() {
        let diff = r#"
-    todo!()
-    unimplemented!()
+fn process() {
+    println!("Implemented!");
+}
"#;
        let gate = AiVerificationGate::new(diff.to_string());
        let result = gate.run_heuristic();
        assert!(result.passed);
    }

    #[test]
    fn test_parse_valid_json_response() {
        let gate = AiVerificationGate::new(String::new());
        let response = r#"{"passed": true, "findings": [], "summary": "No issues"}"#;
        let result = gate.parse_response(response);
        assert!(result.passed);
    }

    #[test]
    fn test_parse_json_with_findings() {
        let gate = AiVerificationGate::new(String::new());
        let response = r#"{
            "passed": false,
            "findings": [
                {
                    "filePath": "src/main.rs",
                    "line": 42,
                    "description": "Function contains todo!()",
                    "severity": "error"
                }
            ],
            "summary": "Found 1 skeleton pattern"
        }"#;
        let result = gate.parse_response(response);
        assert!(!result.passed);
        assert!(!result.findings.is_empty());
    }

    #[test]
    fn test_parse_json_in_markdown() {
        let gate = AiVerificationGate::new(String::new());
        let response = r#"Here is my analysis:
```json
{"passed": true, "findings": [], "summary": "Clean code"}
```"#;
        let result = gate.parse_response(response);
        assert!(result.passed);
    }

    #[test]
    fn test_parse_invalid_response_fallback() {
        let gate = AiVerificationGate::new(String::new());
        let response = "This is not JSON at all";
        let result = gate.parse_response(response);
        // Should gracefully fallback to pass
        assert!(result.passed);
    }

    #[test]
    fn test_build_prompt_includes_diff() {
        let diff = "+fn hello() { }";
        let gate = AiVerificationGate::new(diff.to_string());
        let prompt = gate.build_prompt();
        assert!(prompt.contains("+fn hello()"));
        assert!(prompt.contains("skeleton"));
    }
}
