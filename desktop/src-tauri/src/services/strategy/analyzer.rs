//! Strategy Analyzer
//!
//! Analyzes task descriptions across multiple dimensions to determine
//! the appropriate execution strategy. This is a Rust port of the
//! Python `strategy.py` module with added parallelization and risk
//! dimension scoring.
//!
//! ## Dimensions
//! - **Scope**: How many features/components are involved
//! - **Complexity**: Technical difficulty and architectural impact
//! - **Risk**: Potential for breaking changes and required safeguards
//! - **Parallelization**: Opportunity for parallel execution paths

use serde::{Deserialize, Serialize};

/// Execution strategy types matching the Plan Cascade modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    /// Simple task, execute directly without PRD breakdown
    Direct,
    /// Medium task, automatic PRD generation with story-based execution
    HybridAuto,
    /// Medium task with Git worktree isolation for each story
    HybridWorktree,
    /// Complex project, multi-feature orchestration with mega plan
    MegaPlan,
}

impl ExecutionStrategy {
    /// Human-readable label for the strategy.
    pub fn label(&self) -> &'static str {
        match self {
            ExecutionStrategy::Direct => "Direct",
            ExecutionStrategy::HybridAuto => "Hybrid Auto",
            ExecutionStrategy::HybridWorktree => "Hybrid Worktree",
            ExecutionStrategy::MegaPlan => "Mega Plan",
        }
    }

    /// Short description of the strategy.
    pub fn description(&self) -> &'static str {
        match self {
            ExecutionStrategy::Direct => {
                "Execute the task directly without PRD breakdown. Best for simple, single-step tasks."
            }
            ExecutionStrategy::HybridAuto => {
                "Automatic PRD generation with story-based execution in dependency order."
            }
            ExecutionStrategy::HybridWorktree => {
                "Story-based execution with Git worktree isolation for each story branch."
            }
            ExecutionStrategy::MegaPlan => {
                "Full project planning with multi-feature breakdown, worktrees, and parallel execution."
            }
        }
    }

    /// Return all available strategies.
    pub fn all() -> Vec<ExecutionStrategy> {
        vec![
            ExecutionStrategy::Direct,
            ExecutionStrategy::HybridAuto,
            ExecutionStrategy::HybridWorktree,
            ExecutionStrategy::MegaPlan,
        ]
    }
}

impl std::fmt::Display for ExecutionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStrategy::Direct => write!(f, "direct"),
            ExecutionStrategy::HybridAuto => write!(f, "hybrid_auto"),
            ExecutionStrategy::HybridWorktree => write!(f, "hybrid_worktree"),
            ExecutionStrategy::MegaPlan => write!(f, "mega_plan"),
        }
    }
}

/// Scores across each analysis dimension (0.0 - 1.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScores {
    /// How many features/components are involved (0 = single, 1 = many)
    pub scope: f64,
    /// Technical difficulty and architectural impact (0 = simple, 1 = complex)
    pub complexity: f64,
    /// Potential for breaking changes (0 = safe, 1 = high risk)
    pub risk: f64,
    /// Opportunity for parallel execution (0 = sequential, 1 = highly parallelizable)
    pub parallelization: f64,
}

/// Result of strategy analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDecision {
    /// Recommended execution strategy
    pub strategy: ExecutionStrategy,
    /// Confidence in the recommendation (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable reasoning for the recommendation
    pub reasoning: String,
    /// Estimated number of stories
    pub estimated_stories: u32,
    /// Estimated number of features (for mega plan)
    pub estimated_features: u32,
    /// Estimated duration in hours
    pub estimated_duration_hours: f64,
    /// Indicators that contributed to the decision
    pub complexity_indicators: Vec<String>,
    /// Actionable recommendations
    pub recommendations: Vec<String>,
    /// Dimension scores used for the decision
    pub dimension_scores: DimensionScores,
}

/// Optional context to refine analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisContext {
    /// Whether this is a greenfield (new) project
    #[serde(default)]
    pub is_greenfield: bool,
    /// Approximate lines of code in the existing codebase
    #[serde(default)]
    pub existing_codebase_size: u64,
    /// Whether the project uses Git worktrees already
    #[serde(default)]
    pub has_worktrees: bool,
}

/// Strategy analyzer that scores task descriptions across multiple dimensions.
pub struct StrategyAnalyzer;

impl StrategyAnalyzer {
    // ========================================================================
    // Keyword sets
    // ========================================================================

    const MEGA_KEYWORDS: &'static [&'static str] = &[
        "platform",
        "system",
        "architecture",
        "multiple features",
        "microservices",
        "complete solution",
        "full stack",
        "end to end",
        "e2e",
        "entire",
        "comprehensive",
        "multi-service",
        "distributed",
        "monorepo",
    ];

    const HYBRID_KEYWORDS: &'static [&'static str] = &[
        "implement",
        "create",
        "build",
        "develop",
        "add feature",
        "integration",
        "api",
        "authentication",
        "database",
        "workflow",
        "process",
        "multi-step",
        "migration",
        "refactor",
    ];

    const DIRECT_KEYWORDS: &'static [&'static str] = &[
        "fix bug",
        "update",
        "modify",
        "change",
        "tweak",
        "simple",
        "minor",
        "small",
        "quick",
        "single file",
        "typo",
        "rename",
        "bump version",
    ];

    const RISK_KEYWORDS: &'static [&'static str] = &[
        "breaking change",
        "migration",
        "security",
        "authentication",
        "authorization",
        "payment",
        "billing",
        "production",
        "deploy",
        "infrastructure",
        "database schema",
        "critical",
    ];

    const PARALLELIZATION_KEYWORDS: &'static [&'static str] = &[
        "independent",
        "parallel",
        "concurrent",
        "separate",
        "modular",
        "isolated",
        "decoupled",
        "multiple modules",
        "worktree",
    ];

    /// Analyze a task description and return a strategy recommendation.
    pub fn analyze(description: &str, context: Option<&AnalysisContext>) -> StrategyDecision {
        let description_lower = description.to_lowercase();
        let word_count = description.split_whitespace().count();

        let mut indicators: Vec<String> = Vec::new();
        let mut recommendations: Vec<String> = Vec::new();

        // ====================================================================
        // Score keywords
        // ====================================================================
        let mega_score = Self::count_keyword_matches(&description_lower, Self::MEGA_KEYWORDS);
        let hybrid_score = Self::count_keyword_matches(&description_lower, Self::HYBRID_KEYWORDS);
        let direct_score = Self::count_keyword_matches(&description_lower, Self::DIRECT_KEYWORDS);
        let risk_score = Self::count_keyword_matches(&description_lower, Self::RISK_KEYWORDS);
        let parallel_score =
            Self::count_keyword_matches(&description_lower, Self::PARALLELIZATION_KEYWORDS);

        // ====================================================================
        // Description length factor
        // ====================================================================
        let mut mega_adj: i32 = mega_score as i32;
        let mut hybrid_adj: i32 = hybrid_score as i32;
        let mut direct_adj: i32 = direct_score as i32;

        if word_count > 200 {
            mega_adj += 2;
            indicators.push("Long description suggests complex project".to_string());
        } else if word_count > 100 {
            hybrid_adj += 1;
            indicators.push("Medium description suggests multi-story task".to_string());
        } else if word_count < 30 {
            direct_adj += 1;
            indicators.push("Short description suggests simple task".to_string());
        }

        // ====================================================================
        // Bullet / numbered list heuristic
        // ====================================================================
        let bullet_count = Self::count_list_items(description);
        if bullet_count >= 5 {
            mega_adj += 2;
            indicators.push(format!(
                "Found {} list items suggesting multiple features",
                bullet_count
            ));
        } else if bullet_count >= 3 {
            hybrid_adj += 1;
            indicators.push(format!(
                "Found {} list items suggesting multiple stories",
                bullet_count
            ));
        }

        // ====================================================================
        // Context-based adjustments
        // ====================================================================
        if let Some(ctx) = context {
            if ctx.is_greenfield {
                mega_adj += 1;
                indicators.push("Greenfield project suggests comprehensive approach".to_string());
            }
            if ctx.existing_codebase_size > 10_000 {
                hybrid_adj += 1;
                indicators.push("Large codebase suggests careful multi-story approach".to_string());
            }
            if ctx.has_worktrees {
                // Slight boost toward worktree strategy
                hybrid_adj += 1;
                indicators.push("Project already uses worktrees".to_string());
            }
        }

        // ====================================================================
        // Determine strategy
        // ====================================================================
        let (strategy, confidence, estimated_features, estimated_stories, reasoning) =
            if mega_adj >= 3 && mega_adj > hybrid_adj {
                let conf = (0.5 + mega_adj as f64 * 0.1).min(0.9);
                let features = (mega_adj as u32).max(2);
                let stories = features * 3;
                recommendations.extend([
                    "Consider breaking into independent features with clear interfaces".to_string(),
                    "Use worktrees for parallel feature development".to_string(),
                    "Define feature dependencies carefully".to_string(),
                ]);
                (
                    ExecutionStrategy::MegaPlan,
                    conf,
                    features,
                    stories,
                    "Task complexity and scope suggest multi-feature architecture".to_string(),
                )
            } else if hybrid_adj >= 2 || (word_count > 50 && direct_adj < 2) {
                let conf = (0.5 + hybrid_adj as f64 * 0.1).min(0.9);
                let stories = (hybrid_adj as u32 + 1).max(2);

                // Decide between worktree and auto based on risk + parallel opportunity
                let use_worktree = risk_score >= 2 || parallel_score >= 2;
                let strat = if use_worktree {
                    recommendations.push(
                        "Using worktree isolation due to risk or parallelization opportunity"
                            .to_string(),
                    );
                    ExecutionStrategy::HybridWorktree
                } else {
                    ExecutionStrategy::HybridAuto
                };

                recommendations.extend([
                    "Generate PRD with clear story dependencies".to_string(),
                    "Consider quality gates between stories".to_string(),
                    "Use iteration loop for automatic progression".to_string(),
                ]);

                (
                    strat,
                    conf,
                    1,
                    stories,
                    "Task complexity suggests structured multi-story approach".to_string(),
                )
            } else {
                let conf = (0.5 + direct_adj as f64 * 0.1).min(0.9);
                recommendations.extend([
                    "Execute task directly without PRD generation".to_string(),
                    "Consider adding acceptance criteria for verification".to_string(),
                ]);
                (
                    ExecutionStrategy::Direct,
                    conf,
                    1,
                    1,
                    "Task appears simple enough for direct execution".to_string(),
                )
            };

        // ====================================================================
        // Estimate duration
        // ====================================================================
        let estimated_duration_hours = match strategy {
            ExecutionStrategy::MegaPlan => estimated_features as f64 * 4.0,
            ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree => {
                estimated_stories as f64 * 1.0
            }
            ExecutionStrategy::Direct => 0.5,
        };

        // ====================================================================
        // Dimension scores (normalized 0..1)
        // ====================================================================
        let max_keyword_hits = 5.0_f64; // normalizing constant
        let dimension_scores = DimensionScores {
            scope: ((mega_adj as f64 + bullet_count as f64 * 0.3) / max_keyword_hits).min(1.0),
            complexity: ((hybrid_adj as f64 + mega_adj as f64 * 0.5) / max_keyword_hits).min(1.0),
            risk: (risk_score as f64 / max_keyword_hits).min(1.0),
            parallelization: (parallel_score as f64 / max_keyword_hits).min(1.0),
        };

        StrategyDecision {
            strategy,
            confidence,
            reasoning,
            estimated_stories,
            estimated_features,
            estimated_duration_hours,
            complexity_indicators: indicators,
            recommendations,
            dimension_scores,
        }
    }

    /// Override a strategy decision (for expert mode).
    pub fn override_strategy(
        decision: &StrategyDecision,
        new_strategy: ExecutionStrategy,
        reason: &str,
    ) -> StrategyDecision {
        let mut indicators = decision.complexity_indicators.clone();
        indicators.push("User override applied".to_string());

        StrategyDecision {
            strategy: new_strategy,
            confidence: 1.0,
            reasoning: format!("User override: {}", reason),
            estimated_stories: decision.estimated_stories,
            estimated_features: decision.estimated_features,
            estimated_duration_hours: decision.estimated_duration_hours,
            complexity_indicators: indicators,
            recommendations: decision.recommendations.clone(),
            dimension_scores: decision.dimension_scores.clone(),
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn count_keyword_matches(text: &str, keywords: &[&str]) -> usize {
        keywords.iter().filter(|kw| text.contains(**kw)).count()
    }

    fn count_list_items(text: &str) -> usize {
        let dash_count = text.matches("- ").count();
        let star_count = text.matches("* ").count();
        let numbered_count = (0..10)
            .filter(|i| text.contains(&format!("{}.", i)))
            .count();
        dash_count + star_count + numbered_count
    }
}

/// Information about a single strategy option (for the UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyOption {
    /// Strategy identifier
    pub value: ExecutionStrategy,
    /// Human-readable label
    pub label: String,
    /// Short description
    pub description: String,
    /// Suggested minimum stories
    pub min_stories: u32,
    /// Suggested maximum stories (0 = unlimited)
    pub max_stories: u32,
}

/// Return metadata for all available strategies.
pub fn get_strategy_options() -> Vec<StrategyOption> {
    vec![
        StrategyOption {
            value: ExecutionStrategy::Direct,
            label: "Direct".to_string(),
            description: ExecutionStrategy::Direct.description().to_string(),
            min_stories: 0,
            max_stories: 1,
        },
        StrategyOption {
            value: ExecutionStrategy::HybridAuto,
            label: "Hybrid Auto".to_string(),
            description: ExecutionStrategy::HybridAuto.description().to_string(),
            min_stories: 2,
            max_stories: 10,
        },
        StrategyOption {
            value: ExecutionStrategy::HybridWorktree,
            label: "Hybrid Worktree".to_string(),
            description: ExecutionStrategy::HybridWorktree.description().to_string(),
            min_stories: 2,
            max_stories: 10,
        },
        StrategyOption {
            value: ExecutionStrategy::MegaPlan,
            label: "Mega Plan".to_string(),
            description: ExecutionStrategy::MegaPlan.description().to_string(),
            min_stories: 10,
            max_stories: 0,
        },
    ]
}

// ============================================================================
// Task Mode Analysis
// ============================================================================

/// Execution mode for the UI: Chat (simple) or Task (structured).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Simple chat-based execution for direct tasks
    Chat,
    /// Structured task mode with PRD, stories, and quality gates
    Task,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Chat => write!(f, "chat"),
            ExecutionMode::Task => write!(f, "task"),
        }
    }
}

/// Risk level assessment for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Low risk - simple changes with minimal impact
    Low,
    /// Medium risk - moderate changes that need testing
    Medium,
    /// High risk - breaking changes, security, or production impact
    High,
}

/// Benefit assessment for parallelization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Benefit {
    /// No parallelization benefit
    None,
    /// Moderate parallelization benefit
    Moderate,
    /// Significant parallelization benefit
    Significant,
}

/// Comprehensive strategy analysis result for Chat/Task mode recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyAnalysis {
    /// Identified functional areas in the task
    pub functional_areas: Vec<String>,
    /// Estimated number of stories
    pub estimated_stories: u32,
    /// Whether stories have dependencies on each other
    pub has_dependencies: bool,
    /// Risk level assessment
    pub risk_level: RiskLevel,
    /// Parallelization benefit assessment
    pub parallelization_benefit: Benefit,
    /// Recommended execution mode
    pub recommended_mode: ExecutionMode,
    /// Confidence in the recommendation (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable reasoning for the recommendation
    pub reasoning: String,
    /// The underlying strategy decision
    pub strategy_decision: StrategyDecision,
}

/// Analyze a task description and return a Chat/Task mode recommendation.
///
/// Wraps `StrategyAnalyzer::analyze()` with additional heuristics:
/// - Direct strategy -> Chat mode
/// - HybridAuto/HybridWorktree/MegaPlan -> Task mode
pub fn analyze_task_for_mode(
    description: &str,
    context: Option<&AnalysisContext>,
) -> StrategyAnalysis {
    let decision = StrategyAnalyzer::analyze(description, context);

    let recommended_mode = match decision.strategy {
        ExecutionStrategy::Direct => ExecutionMode::Chat,
        ExecutionStrategy::HybridAuto
        | ExecutionStrategy::HybridWorktree
        | ExecutionStrategy::MegaPlan => ExecutionMode::Task,
    };

    let risk_level = if decision.dimension_scores.risk >= 0.6 {
        RiskLevel::High
    } else if decision.dimension_scores.risk >= 0.3 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    let parallelization_benefit = if decision.dimension_scores.parallelization >= 0.6 {
        Benefit::Significant
    } else if decision.dimension_scores.parallelization >= 0.3 {
        Benefit::Moderate
    } else {
        Benefit::None
    };

    let has_dependencies = decision.estimated_stories > 1
        && (decision.dimension_scores.scope >= 0.3
            || decision.dimension_scores.complexity >= 0.4);

    // Extract functional areas from complexity indicators
    let functional_areas = decision
        .complexity_indicators
        .iter()
        .filter(|ind| {
            !ind.contains("description suggests")
                && !ind.contains("list items")
                && !ind.contains("already uses")
        })
        .cloned()
        .collect::<Vec<_>>();

    let reasoning = match recommended_mode {
        ExecutionMode::Chat => format!(
            "Task is simple enough for direct chat execution. {}",
            decision.reasoning
        ),
        ExecutionMode::Task => format!(
            "Task complexity warrants structured task mode with PRD and quality gates. {}",
            decision.reasoning
        ),
    };

    StrategyAnalysis {
        functional_areas,
        estimated_stories: decision.estimated_stories,
        has_dependencies,
        risk_level,
        parallelization_benefit,
        recommended_mode,
        confidence: decision.confidence,
        reasoning,
        strategy_decision: decision,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // StrategyAnalysis / analyze_task_for_mode tests
    // ========================================================================

    #[test]
    fn test_simple_task_returns_chat_mode() {
        let analysis = analyze_task_for_mode("fix a typo in the readme", None);
        assert_eq!(analysis.recommended_mode, ExecutionMode::Chat);
        assert_eq!(analysis.estimated_stories, 1);
        assert!(analysis.confidence >= 0.5);
        assert_eq!(analysis.risk_level, RiskLevel::Low);
        assert!(analysis.reasoning.contains("chat execution"));
    }

    #[test]
    fn test_complex_task_returns_task_mode() {
        let analysis = analyze_task_for_mode(
            "Build a comprehensive e2e platform with multiple features: \
             1. User authentication system \
             2. Payment processing microservices \
             3. Full stack dashboard \
             4. Complete solution for analytics \
             5. End to end testing infrastructure",
            None,
        );
        assert_eq!(analysis.recommended_mode, ExecutionMode::Task);
        assert!(analysis.estimated_stories >= 2);
        assert!(analysis.has_dependencies);
        assert!(analysis.reasoning.contains("task mode"));
    }

    #[test]
    fn test_medium_task_returns_task_mode() {
        let analysis = analyze_task_for_mode(
            "implement authentication system with OAuth integration, create API endpoints for user management, and build database migration workflow",
            None,
        );
        assert_eq!(analysis.recommended_mode, ExecutionMode::Task);
    }

    #[test]
    fn test_high_risk_detection() {
        let analysis = analyze_task_for_mode(
            "implement payment billing system with database schema migration for production deploy with breaking changes",
            None,
        );
        assert!(matches!(analysis.risk_level, RiskLevel::Medium | RiskLevel::High));
    }

    #[test]
    fn test_parallelization_benefit() {
        let analysis = analyze_task_for_mode(
            "create independent parallel modular decoupled separate isolated modules",
            None,
        );
        assert!(matches!(
            analysis.parallelization_benefit,
            Benefit::Moderate | Benefit::Significant
        ));
    }

    #[test]
    fn test_execution_mode_serialization() {
        let chat = serde_json::to_string(&ExecutionMode::Chat).unwrap();
        assert_eq!(chat, "\"chat\"");
        let task = serde_json::to_string(&ExecutionMode::Task).unwrap();
        assert_eq!(task, "\"task\"");
    }

    #[test]
    fn test_risk_level_serialization() {
        let low = serde_json::to_string(&RiskLevel::Low).unwrap();
        assert_eq!(low, "\"low\"");
        let high = serde_json::to_string(&RiskLevel::High).unwrap();
        assert_eq!(high, "\"high\"");
    }

    #[test]
    fn test_benefit_serialization() {
        let none = serde_json::to_string(&Benefit::None).unwrap();
        assert_eq!(none, "\"none\"");
        let sig = serde_json::to_string(&Benefit::Significant).unwrap();
        assert_eq!(sig, "\"significant\"");
    }

    #[test]
    fn test_strategy_analysis_camel_case_serialization() {
        let analysis = analyze_task_for_mode("fix a typo", None);
        let json = serde_json::to_string(&analysis).unwrap();
        assert!(json.contains("\"recommendedMode\""));
        assert!(json.contains("\"estimatedStories\""));
        assert!(json.contains("\"hasDependencies\""));
        assert!(json.contains("\"riskLevel\""));
        assert!(json.contains("\"parallelizationBenefit\""));
        assert!(json.contains("\"functionalAreas\""));
        assert!(json.contains("\"strategyDecision\""));
    }

    #[test]
    fn test_boundary_confidence_scores() {
        // Simple task
        let simple = analyze_task_for_mode("rename variable", None);
        assert!(simple.confidence >= 0.5 && simple.confidence <= 1.0);

        // Complex task
        let complex = analyze_task_for_mode(
            "Build a comprehensive system with architecture and multiple features, microservices, complete solution, full stack, end to end",
            None,
        );
        assert!(complex.confidence >= 0.5 && complex.confidence <= 1.0);
    }

    // ========================================================================
    // Existing StrategyAnalyzer tests (unchanged)
    // ========================================================================

    #[test]
    fn test_simple_task_returns_direct() {
        let decision = StrategyAnalyzer::analyze("fix bug in login page", None);
        assert_eq!(decision.strategy, ExecutionStrategy::Direct);
        assert!(decision.confidence >= 0.5);
        assert_eq!(decision.estimated_stories, 1);
    }

    #[test]
    fn test_medium_task_returns_hybrid() {
        let decision = StrategyAnalyzer::analyze(
            "implement authentication system with OAuth integration, create API endpoints for user management, and build database migration workflow",
            None,
        );
        assert!(matches!(
            decision.strategy,
            ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree
        ));
        assert!(decision.estimated_stories >= 2);
    }

    #[test]
    fn test_complex_project_returns_mega() {
        let decision = StrategyAnalyzer::analyze(
            "Build a comprehensive e2e platform with multiple features: \
             1. User authentication system \
             2. Payment processing microservices \
             3. Full stack dashboard \
             4. Complete solution for analytics \
             5. End to end testing infrastructure",
            None,
        );
        assert_eq!(decision.strategy, ExecutionStrategy::MegaPlan);
        assert!(decision.estimated_features >= 2);
    }

    #[test]
    fn test_override_strategy() {
        let decision = StrategyAnalyzer::analyze("fix typo", None);
        assert_eq!(decision.strategy, ExecutionStrategy::Direct);

        let overridden = StrategyAnalyzer::override_strategy(
            &decision,
            ExecutionStrategy::HybridAuto,
            "I want stories",
        );
        assert_eq!(overridden.strategy, ExecutionStrategy::HybridAuto);
        assert!((overridden.confidence - 1.0).abs() < f64::EPSILON);
        assert!(overridden.reasoning.contains("User override"));
    }

    #[test]
    fn test_risk_keywords_boost_worktree() {
        let decision = StrategyAnalyzer::analyze(
            "implement payment billing system with database schema migration for production deploy",
            None,
        );
        // High risk keywords should push towards worktree isolation
        assert!(matches!(
            decision.strategy,
            ExecutionStrategy::HybridWorktree | ExecutionStrategy::HybridAuto
        ));
    }

    #[test]
    fn test_dimension_scores_bounded() {
        let decision = StrategyAnalyzer::analyze(
            "Build a comprehensive end to end platform with multiple features, \
             microservices architecture, complete solution, full stack, \
             independent parallel modular decoupled modules, \
             breaking change migration security authentication payment billing production deploy",
            None,
        );
        assert!(decision.dimension_scores.scope >= 0.0 && decision.dimension_scores.scope <= 1.0);
        assert!(
            decision.dimension_scores.complexity >= 0.0
                && decision.dimension_scores.complexity <= 1.0
        );
        assert!(decision.dimension_scores.risk >= 0.0 && decision.dimension_scores.risk <= 1.0);
        assert!(
            decision.dimension_scores.parallelization >= 0.0
                && decision.dimension_scores.parallelization <= 1.0
        );
    }

    #[test]
    fn test_greenfield_context_boosts_mega() {
        let ctx = AnalysisContext {
            is_greenfield: true,
            existing_codebase_size: 0,
            has_worktrees: false,
        };
        let without = StrategyAnalyzer::analyze(
            "Build a system with architecture and multiple features",
            None,
        );
        let with = StrategyAnalyzer::analyze(
            "Build a system with architecture and multiple features",
            Some(&ctx),
        );
        // With greenfield context the mega score should be equal or higher
        assert!(with.dimension_scores.scope >= without.dimension_scores.scope);
    }

    #[test]
    fn test_get_strategy_options_returns_all() {
        let options = get_strategy_options();
        assert_eq!(options.len(), 4);
        assert_eq!(options[0].value, ExecutionStrategy::Direct);
        assert_eq!(options[3].value, ExecutionStrategy::MegaPlan);
    }

    #[test]
    fn test_strategy_display() {
        assert_eq!(format!("{}", ExecutionStrategy::Direct), "direct");
        assert_eq!(format!("{}", ExecutionStrategy::HybridAuto), "hybrid_auto");
        assert_eq!(
            format!("{}", ExecutionStrategy::HybridWorktree),
            "hybrid_worktree"
        );
        assert_eq!(format!("{}", ExecutionStrategy::MegaPlan), "mega_plan");
    }
}
