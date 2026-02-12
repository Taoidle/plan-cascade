//! Adaptive analysis scope classification helpers.
//!
//! Keep intent/scope heuristics out of the main orchestrator service to improve
//! readability and maintainability.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdaptiveAnalysisScope {
    None,
    Local,
    Global,
}

pub(super) fn detect_adaptive_analysis_scope(
    message: &str,
    has_path_target: bool,
) -> AdaptiveAnalysisScope {
    if is_project_purpose_question(message) {
        return AdaptiveAnalysisScope::Global;
    }

    if is_exploration_task(message) {
        return AdaptiveAnalysisScope::Global;
    }

    let lower = message.to_lowercase();
    let has_execution_intent = [
        "implement",
        "fix",
        "refactor",
        "add",
        "change",
        "update",
        "\u{5b9e}\u{73b0}",
        "\u{4fee}\u{590d}",
        "\u{91cd}\u{6784}",
        "\u{65b0}\u{589e}",
        "\u{4fee}\u{6539}",
    ]
    .iter()
    .any(|kw| lower.contains(kw));
    if !has_execution_intent {
        return AdaptiveAnalysisScope::None;
    }

    let has_global_scope = [
        "whole project",
        "entire project",
        "whole repo",
        "across the repo",
        "across modules",
        "project-wide",
        "global",
        "\u{6574}\u{4e2a}\u{9879}\u{76ee}",
        "\u{5168}\u{5c40}",
        "\u{8de8}\u{6a21}\u{5757}",
        "\u{6574}\u{4f53}",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if has_global_scope {
        return AdaptiveAnalysisScope::Global;
    }

    if has_path_target {
        return AdaptiveAnalysisScope::Local;
    }

    let has_local_scope_words = [
        "file",
        "module",
        "function",
        "class",
        "endpoint",
        "test",
        "\u{6587}\u{4ef6}",
        "\u{6a21}\u{5757}",
        "\u{51fd}\u{6570}",
        "\u{7c7b}",
        "\u{63a5}\u{53e3}",
        "\u{6d4b}\u{8bd5}",
    ]
    .iter()
    .any(|kw| lower.contains(kw));

    if has_local_scope_words {
        AdaptiveAnalysisScope::Local
    } else {
        // For code-change intents without explicit scope, run at least a local pre-analysis
        // so the agent can gather concrete context before editing.
        AdaptiveAnalysisScope::Local
    }
}

pub(super) fn is_project_purpose_question(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();

    const EN_PATTERNS: &[&str] = &[
        "what is this project",
        "what's this project",
        "what does this project do",
        "what is this project for",
        "what is this repo",
        "what's this repo",
        "what does this repo do",
        "what is this repository",
        "what does this repository do",
        "what is this codebase",
        "what does this codebase do",
        "what is this code for",
        "purpose of this project",
        "purpose of this repo",
        "about this project",
        "about this repository",
    ];

    if EN_PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }

    let has_en_context = [
        "project",
        "repo",
        "repository",
        "codebase",
        "workspace",
        "directory",
        "folder",
    ]
    .iter()
    .any(|kw| lower.contains(kw));
    let has_en_intent = (lower.contains("what") && lower.contains("for"))
        || lower.contains("what does")
        || lower.contains("purpose");
    if has_en_context && has_en_intent {
        return true;
    }

    const ZH_PATTERNS: &[&str] = &[
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{662f}\u{5e72}\u{4ec0}\u{4e48}\u{7684}",
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{662f}\u{505a}\u{4ec0}\u{4e48}\u{7684}",
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{62ff}\u{6765}\u{5e72}\u{4ec0}\u{4e48}",
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{7528}\u{6765}\u{5e72}\u{4ec0}\u{4e48}",
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{6709}\u{4ec0}\u{4e48}\u{7528}",
        "\u{8fd9}\u{4e2a}\u{4ed3}\u{5e93}\u{662f}\u{5e72}\u{4ec0}\u{4e48}\u{7684}",
        "\u{8fd9}\u{4e2a}\u{4ed3}\u{5e93}\u{662f}\u{505a}\u{4ec0}\u{4e48}\u{7684}",
        "\u{8fd9}\u{4e2a}\u{4ee3}\u{7801}\u{5e93}\u{662f}\u{5e72}\u{4ec0}\u{4e48}\u{7684}",
        "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{662f}\u{4ec0}\u{4e48}",
        "\u{4ecb}\u{7ecd}\u{4e00}\u{4e0b}\u{8fd9}\u{4e2a}\u{9879}\u{76ee}",
    ];

    if ZH_PATTERNS.iter().any(|p| message.contains(p)) {
        return true;
    }

    let has_zh_context = [
        "\u{9879}\u{76ee}",
        "\u{4ed3}\u{5e93}",
        "\u{4ee3}\u{7801}\u{5e93}",
    ]
    .iter()
    .any(|kw| message.contains(kw));
    let has_zh_intent = [
        "\u{5e72}\u{4ec0}\u{4e48}",
        "\u{505a}\u{4ec0}\u{4e48}",
        "\u{7528}\u{6765}",
        "\u{4ec0}\u{4e48}\u{7528}",
        "\u{4ecb}\u{7ecd}",
    ]
    .iter()
    .any(|kw| message.contains(kw));
    has_zh_context && has_zh_intent
}

pub(super) fn is_exploration_task(message: &str) -> bool {
    if is_project_purpose_question(message) {
        return true;
    }

    let lower = message.to_lowercase();

    const EN_ANALYSIS: &[&str] = &[
        "analyze",
        "analyse",
        "explore",
        "review",
        "investigate",
        "understand",
        "overview",
        "summarize",
        "summarise",
        "architecture",
        "codebase",
        "repository",
        "repo",
    ];
    const EN_CONTEXT: &[&str] = &[
        "project",
        "codebase",
        "repository",
        "repo",
        "workspace",
        "folder",
        "directory",
    ];
    const EN_EXECUTION: &[&str] = &[
        "implement",
        "fix",
        "build",
        "write",
        "create",
        "add",
        "remove",
        "refactor",
    ];

    const ZH_ANALYSIS: &[&str] = &[
        "\u{5206}\u{6790}",
        "\u{63a2}\u{7d22}",
        "\u{4e86}\u{89e3}",
        "\u{603b}\u{7ed3}",
        "\u{67e5}\u{770b}",
        "\u{67b6}\u{6784}",
    ];
    const ZH_CONTEXT: &[&str] = &[
        "\u{9879}\u{76ee}",
        "\u{4ee3}\u{7801}\u{5e93}",
        "\u{4ed3}\u{5e93}",
        "\u{5de5}\u{4f5c}\u{533a}",
        "\u{76ee}\u{5f55}",
    ];
    const ZH_EXECUTION: &[&str] = &[
        "\u{5b9e}\u{73b0}",
        "\u{4fee}\u{590d}",
        "\u{65b0}\u{589e}",
        "\u{7f16}\u{5199}",
        "\u{91cd}\u{6784}",
    ];

    let has_zh_analysis = ZH_ANALYSIS.iter().any(|kw| message.contains(kw));
    let has_zh_context = ZH_CONTEXT.iter().any(|kw| message.contains(kw));
    let has_zh_execution = ZH_EXECUTION.iter().any(|kw| message.contains(kw));

    if has_zh_analysis && has_zh_context {
        return true;
    }
    if has_zh_execution && !has_zh_analysis {
        return false;
    }

    let has_en_analysis = EN_ANALYSIS.iter().any(|kw| lower.contains(kw));
    let has_en_context = EN_CONTEXT.iter().any(|kw| lower.contains(kw));
    let has_en_execution = EN_EXECUTION.iter().any(|kw| lower.contains(kw));

    if has_en_execution && !has_en_analysis {
        return false;
    }

    has_en_analysis && has_en_context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_exploration_task_chinese() {
        assert!(is_exploration_task(
            "\u{5206}\u{6790}\u{8fd9}\u{4e2a}\u{9879}\u{76ee}"
        ));
        assert!(is_exploration_task(
            "\u{5e2e}\u{6211}\u{4e86}\u{89e3}\u{8fd9}\u{4e2a}\u{4ee3}\u{7801}\u{4ed3}\u{5e93}"
        ));
        assert!(is_exploration_task(
            "\u{603b}\u{7ed3}\u{4e00}\u{4e0b}\u{9879}\u{76ee}\u{67b6}\u{6784}"
        ));
        assert!(is_exploration_task(
            "\u{63a2}\u{7d22}\u{4ee3}\u{7801}\u{76ee}\u{5f55}"
        ));
    }

    #[test]
    fn test_is_exploration_task_english() {
        assert!(is_exploration_task("analyze this project"));
        assert!(is_exploration_task("Explore the codebase"));
        assert!(is_exploration_task(
            "give me an overview of this repository"
        ));
        assert!(is_exploration_task("summarize the repository architecture"));
        assert!(is_exploration_task("help me understand this codebase"));
        assert!(is_exploration_task("what is this project for?"));
    }

    #[test]
    fn test_is_project_purpose_question_variants() {
        assert!(is_project_purpose_question("what does this project do?"));
        assert!(is_project_purpose_question(
            "Can you tell me what this repository is for?"
        ));
        assert!(is_project_purpose_question(
            "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{662f}\u{62ff}\u{6765}\u{5e72}\u{4ec0}\u{4e48}\u{7684}\u{ff1f}"
        ));
        assert!(!is_project_purpose_question("fix login timeout bug"));
    }

    #[test]
    fn test_is_exploration_task_false_positives() {
        assert!(!is_exploration_task("explain how JWT works"));
        assert!(!is_exploration_task("analyze the market trends"));
        assert!(!is_exploration_task("summarize this conversation"));
        assert!(!is_exploration_task(
            "\u{603b}\u{7ed3}\u{8fd9}\u{6b21}\u{5bf9}\u{8bdd}"
        ));
        assert!(!is_exploration_task("add a login button"));
        assert!(!is_exploration_task("fix the bug in checkout"));
        assert!(!is_exploration_task("write a test for the API"));
        assert!(!is_exploration_task("implement this endpoint"));
        assert!(!is_exploration_task(
            "\u{4fee}\u{590d}\u{767b}\u{5f55}\u{6309}\u{94ae}"
        ));
    }

    #[test]
    fn test_detect_adaptive_analysis_scope_global() {
        assert_eq!(
            detect_adaptive_analysis_scope("analyze this project architecture", false),
            AdaptiveAnalysisScope::Global
        );
        assert_eq!(
            detect_adaptive_analysis_scope("what is this project for?", false),
            AdaptiveAnalysisScope::Global
        );
        assert_eq!(
            detect_adaptive_analysis_scope(
                "\u{8fd9}\u{4e2a}\u{9879}\u{76ee}\u{662f}\u{62ff}\u{6765}\u{5e72}\u{4ec0}\u{4e48}\u{7684}\u{ff1f}",
                false
            ),
            AdaptiveAnalysisScope::Global
        );
        assert_eq!(
            detect_adaptive_analysis_scope(
                "\u{91cd}\u{6784}\u{6574}\u{4e2a}\u{9879}\u{76ee}\u{7684}\u{67b6}\u{6784}",
                false
            ),
            AdaptiveAnalysisScope::Global
        );
    }

    #[test]
    fn test_detect_adaptive_analysis_scope_local() {
        assert_eq!(
            detect_adaptive_analysis_scope(
                "implement retry logic in src/plan_cascade/core/orchestrator.py",
                true
            ),
            AdaptiveAnalysisScope::Local
        );
        assert_eq!(
            detect_adaptive_analysis_scope(
                "\u{4fee}\u{590d} desktop/src/App.tsx \u{4e2d}\u{7684} \u{5d29}\u{6e83}",
                true
            ),
            AdaptiveAnalysisScope::Local
        );
    }

    #[test]
    fn test_detect_adaptive_analysis_scope_none() {
        assert_eq!(
            detect_adaptive_analysis_scope("hello there", false),
            AdaptiveAnalysisScope::None
        );
    }

    #[test]
    fn test_detect_adaptive_analysis_scope_defaults_to_local_for_execution_intent() {
        assert_eq!(
            detect_adaptive_analysis_scope(
                "implement the auth flow with proper error handling",
                false
            ),
            AdaptiveAnalysisScope::Local
        );
    }
}
