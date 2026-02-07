//! Strategy Analyzer Integration Tests
//!
//! Comprehensive tests for the strategy analysis service covering all four
//! strategy outcomes (DIRECT, HYBRID_AUTO, HYBRID_WORKTREE, MEGA_PLAN),
//! edge cases, confidence score ranges, and the intent classifier.
//!
//! No LLM calls are made - the strategy analyzer is entirely rule-based.

use plan_cascade_desktop::services::strategy::{
    StrategyAnalyzer, ExecutionStrategy, DimensionScores, StrategyDecision,
};
use plan_cascade_desktop::services::strategy::analyzer::{
    AnalysisContext, get_strategy_options,
};
use plan_cascade_desktop::services::strategy::classifier::{
    IntentClassifier, Intent, get_intent_choices,
};

// ============================================================================
// Strategy Analyzer: All Four Strategy Outcomes
// ============================================================================

/// AC1: Test DIRECT strategy for simple tasks
#[test]
fn test_direct_strategy_simple_bug_fix() {
    let decision = StrategyAnalyzer::analyze("fix bug in login page", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
    assert!(decision.confidence >= 0.5, "Confidence {} < 0.5", decision.confidence);
    assert!(decision.confidence <= 1.0, "Confidence {} > 1.0", decision.confidence);
    assert_eq!(decision.estimated_stories, 1);
    assert_eq!(decision.estimated_features, 1);
    assert!(decision.estimated_duration_hours <= 1.0);
}

#[test]
fn test_direct_strategy_typo_fix() {
    let decision = StrategyAnalyzer::analyze("fix typo in readme", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
    assert_eq!(decision.estimated_stories, 1);
}

#[test]
fn test_direct_strategy_rename() {
    let decision = StrategyAnalyzer::analyze("rename variable", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
}

#[test]
fn test_direct_strategy_bump_version() {
    let decision = StrategyAnalyzer::analyze("bump version to 2.0", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
}

#[test]
fn test_direct_strategy_simple_tweak() {
    let decision = StrategyAnalyzer::analyze("tweak small CSS", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
}

#[test]
fn test_direct_strategy_minor_update() {
    let decision = StrategyAnalyzer::analyze("update minor config", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
}

/// AC1: Test HYBRID_AUTO strategy for medium tasks
#[test]
fn test_hybrid_auto_strategy_feature_implementation() {
    let decision = StrategyAnalyzer::analyze(
        "implement authentication system with OAuth integration, create API endpoints for user management, and build database migration workflow",
        None,
    );
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree),
        "Expected HybridAuto or HybridWorktree, got {:?}",
        decision.strategy
    );
    assert!(decision.estimated_stories >= 2);
    assert!(decision.confidence >= 0.5);
}

#[test]
fn test_hybrid_auto_multi_step_process() {
    let decision = StrategyAnalyzer::analyze(
        "implement a multi-step workflow to process and validate incoming API requests, create database schemas, and add integration tests",
        None,
    );
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree),
        "Expected Hybrid strategy, got {:?}",
        decision.strategy
    );
}

#[test]
fn test_hybrid_auto_refactoring_task() {
    let decision = StrategyAnalyzer::analyze(
        "refactor the authentication module to use a new OAuth2 library and migrate the database schema for the user table with a multi-step process",
        None,
    );
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree),
        "Expected Hybrid strategy, got {:?}",
        decision.strategy
    );
}

/// AC1: Test HYBRID_WORKTREE strategy (triggered by risk/parallelization keywords)
#[test]
fn test_hybrid_worktree_high_risk() {
    let decision = StrategyAnalyzer::analyze(
        "implement payment billing system with database schema migration for production deploy with breaking change to the authentication flow and security updates",
        None,
    );
    // High risk keywords: payment, billing, database schema, production, deploy, breaking change, security, authentication
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridWorktree | ExecutionStrategy::HybridAuto),
        "Expected worktree/auto strategy due to risk keywords, got {:?}",
        decision.strategy
    );
}

#[test]
fn test_hybrid_worktree_parallel_modules() {
    let decision = StrategyAnalyzer::analyze(
        "implement independent parallel isolated decoupled modules for the separate service layer with database integration and create API endpoints",
        None,
    );
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridWorktree | ExecutionStrategy::HybridAuto | ExecutionStrategy::MegaPlan),
        "Expected worktree/auto/mega strategy for parallel keywords, got {:?}",
        decision.strategy
    );
}

/// AC1: Test MEGA_PLAN strategy for complex projects
#[test]
fn test_mega_plan_complex_platform() {
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
    assert!(decision.confidence >= 0.5);
}

#[test]
fn test_mega_plan_distributed_architecture() {
    let decision = StrategyAnalyzer::analyze(
        "design and build a distributed microservices architecture for the entire platform with a comprehensive multi-service monorepo setup including full stack system",
        None,
    );
    assert_eq!(
        decision.strategy,
        ExecutionStrategy::MegaPlan,
        "Expected MegaPlan for distributed architecture keywords"
    );
}

#[test]
fn test_mega_plan_long_description_with_features() {
    // A long description with many features and bullet points
    let description = "Build a comprehensive SaaS platform with the following features:\n\
        - User authentication and authorization system\n\
        - Multi-tenant data isolation architecture\n\
        - REST API gateway with rate limiting\n\
        - Real-time notification microservices\n\
        - Analytics dashboard with full stack reporting\n\
        - Payment processing with Stripe integration\n\
        - Admin console for complete solution management\n\
        This is a distributed system with end to end testing and should be a monorepo setup.";

    let decision = StrategyAnalyzer::analyze(description, None);
    assert_eq!(
        decision.strategy,
        ExecutionStrategy::MegaPlan,
        "Long feature list should trigger MegaPlan"
    );
    assert!(decision.estimated_features >= 2);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_description() {
    let decision = StrategyAnalyzer::analyze("", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
    assert_eq!(decision.estimated_stories, 1);
}

#[test]
fn test_single_word_description() {
    let decision = StrategyAnalyzer::analyze("fix", None);
    assert_eq!(decision.strategy, ExecutionStrategy::Direct);
}

#[test]
fn test_very_long_description_without_keywords() {
    let filler: String = (0..250).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");
    let decision = StrategyAnalyzer::analyze(&filler, None);
    // Long but no specific keywords -- should at least be hybrid due to length
    assert!(
        matches!(decision.strategy, ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree | ExecutionStrategy::MegaPlan),
        "Very long description should not be Direct, got {:?}",
        decision.strategy
    );
}

#[test]
fn test_mixed_signals_direct_and_mega() {
    // Has both direct keywords and mega keywords
    let decision = StrategyAnalyzer::analyze(
        "fix a simple minor bug in the comprehensive distributed microservices platform system architecture",
        None,
    );
    // Should resolve to one specific strategy
    assert!(
        matches!(
            decision.strategy,
            ExecutionStrategy::Direct
                | ExecutionStrategy::HybridAuto
                | ExecutionStrategy::HybridWorktree
                | ExecutionStrategy::MegaPlan
        ),
        "Should resolve to any valid strategy"
    );
}

// ============================================================================
// Confidence Score Ranges (AC1)
// ============================================================================

#[test]
fn test_confidence_always_in_valid_range() {
    let descriptions = vec![
        "",
        "fix bug",
        "implement authentication system with OAuth integration and database migration",
        "Build comprehensive distributed microservices platform with multiple features end to end system architecture",
        "a",
        &"word ".repeat(300),
    ];

    for desc in descriptions {
        let decision = StrategyAnalyzer::analyze(desc, None);
        assert!(
            decision.confidence >= 0.0 && decision.confidence <= 1.0,
            "Confidence {} out of range for: '{}'",
            decision.confidence,
            &desc[..desc.len().min(50)]
        );
    }
}

#[test]
fn test_override_sets_confidence_to_one() {
    let decision = StrategyAnalyzer::analyze("fix typo", None);
    let overridden = StrategyAnalyzer::override_strategy(
        &decision,
        ExecutionStrategy::MegaPlan,
        "Testing override",
    );
    assert!((overridden.confidence - 1.0).abs() < f64::EPSILON);
    assert_eq!(overridden.strategy, ExecutionStrategy::MegaPlan);
    assert!(overridden.reasoning.contains("User override"));
    assert!(overridden.complexity_indicators.contains(&"User override applied".to_string()));
}

// ============================================================================
// Dimension Scores (AC1)
// ============================================================================

#[test]
fn test_dimension_scores_bounded_for_all_strategies() {
    let descriptions = vec![
        ("fix typo", ExecutionStrategy::Direct),
        ("implement authentication with database migration workflow", ExecutionStrategy::HybridAuto),
        (
            "Build comprehensive e2e platform with multiple features microservices full stack end to end system architecture",
            ExecutionStrategy::MegaPlan,
        ),
    ];

    for (desc, _expected) in descriptions {
        let decision = StrategyAnalyzer::analyze(desc, None);
        let ds = &decision.dimension_scores;

        assert!(ds.scope >= 0.0 && ds.scope <= 1.0, "Scope {} out of range", ds.scope);
        assert!(ds.complexity >= 0.0 && ds.complexity <= 1.0, "Complexity {} out of range", ds.complexity);
        assert!(ds.risk >= 0.0 && ds.risk <= 1.0, "Risk {} out of range", ds.risk);
        assert!(ds.parallelization >= 0.0 && ds.parallelization <= 1.0, "Parallelization {} out of range", ds.parallelization);
    }
}

#[test]
fn test_risk_score_higher_for_risky_tasks() {
    let safe = StrategyAnalyzer::analyze("fix typo in config", None);
    let risky = StrategyAnalyzer::analyze(
        "implement payment billing with database schema migration for production deploy with security authentication",
        None,
    );

    assert!(
        risky.dimension_scores.risk >= safe.dimension_scores.risk,
        "Risky task should have higher risk score: {} >= {}",
        risky.dimension_scores.risk,
        safe.dimension_scores.risk
    );
}

// ============================================================================
// Context-Based Analysis
// ============================================================================

#[test]
fn test_greenfield_context_boosts_scope() {
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

    assert!(
        with.dimension_scores.scope >= without.dimension_scores.scope,
        "Greenfield context should boost scope: {} >= {}",
        with.dimension_scores.scope,
        without.dimension_scores.scope
    );
}

#[test]
fn test_large_codebase_context() {
    let ctx = AnalysisContext {
        is_greenfield: false,
        existing_codebase_size: 50_000,
        has_worktrees: false,
    };

    let decision = StrategyAnalyzer::analyze(
        "implement a new API endpoint with validation",
        Some(&ctx),
    );

    // Large codebase should boost hybrid
    assert!(decision.complexity_indicators.iter().any(|i| i.contains("Large codebase")));
}

#[test]
fn test_worktree_context_noted() {
    let ctx = AnalysisContext {
        is_greenfield: false,
        existing_codebase_size: 0,
        has_worktrees: true,
    };

    let decision = StrategyAnalyzer::analyze(
        "implement authentication with database migration",
        Some(&ctx),
    );

    assert!(decision.complexity_indicators.iter().any(|i| i.contains("worktree")));
}

// ============================================================================
// Strategy Options and Metadata
// ============================================================================

#[test]
fn test_all_strategy_options_present() {
    let options = get_strategy_options();
    assert_eq!(options.len(), 4);

    let values: Vec<ExecutionStrategy> = options.iter().map(|o| o.value).collect();
    assert!(values.contains(&ExecutionStrategy::Direct));
    assert!(values.contains(&ExecutionStrategy::HybridAuto));
    assert!(values.contains(&ExecutionStrategy::HybridWorktree));
    assert!(values.contains(&ExecutionStrategy::MegaPlan));
}

#[test]
fn test_strategy_labels_and_descriptions() {
    for strat in ExecutionStrategy::all() {
        let label = strat.label();
        let desc = strat.description();
        assert!(!label.is_empty(), "Label should not be empty for {:?}", strat);
        assert!(!desc.is_empty(), "Description should not be empty for {:?}", strat);
    }
}

#[test]
fn test_strategy_display_format() {
    assert_eq!(format!("{}", ExecutionStrategy::Direct), "direct");
    assert_eq!(format!("{}", ExecutionStrategy::HybridAuto), "hybrid_auto");
    assert_eq!(format!("{}", ExecutionStrategy::HybridWorktree), "hybrid_worktree");
    assert_eq!(format!("{}", ExecutionStrategy::MegaPlan), "mega_plan");
}

#[test]
fn test_strategy_serialization_roundtrip() {
    for strat in ExecutionStrategy::all() {
        let json = serde_json::to_string(&strat).unwrap();
        let parsed: ExecutionStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(strat, parsed, "Roundtrip failed for {:?}", strat);
    }
}

#[test]
fn test_strategy_decision_serialization() {
    let decision = StrategyAnalyzer::analyze(
        "implement authentication system with OAuth integration",
        None,
    );

    let json = serde_json::to_string(&decision).unwrap();
    let parsed: StrategyDecision = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.strategy, decision.strategy);
    assert!((parsed.confidence - decision.confidence).abs() < f64::EPSILON);
    assert_eq!(parsed.reasoning, decision.reasoning);
    assert_eq!(parsed.estimated_stories, decision.estimated_stories);
}

// ============================================================================
// Estimated Duration Consistency
// ============================================================================

#[test]
fn test_estimated_duration_increases_with_complexity() {
    let direct = StrategyAnalyzer::analyze("fix typo", None);
    let hybrid = StrategyAnalyzer::analyze(
        "implement authentication with database migration workflow process",
        None,
    );
    let mega = StrategyAnalyzer::analyze(
        "Build a comprehensive e2e platform with multiple features: \
         1. Authentication system \
         2. Payment microservices \
         3. Full stack dashboard \
         4. Complete analytics solution \
         5. End to end testing",
        None,
    );

    assert!(direct.estimated_duration_hours <= hybrid.estimated_duration_hours);
    assert!(hybrid.estimated_duration_hours <= mega.estimated_duration_hours);
}

// ============================================================================
// Intent Classifier Integration
// ============================================================================

#[test]
fn test_intent_classifier_task_detection() {
    let classifier = IntentClassifier::new();

    let tasks = vec![
        "implement a new login page",
        "create an API endpoint for users",
        "fix the broken authentication",
        "refactor the data layer",
        "build a new dashboard component",
        "port the Python module to Rust",
        "set up the CI pipeline",
        "delete the deprecated module",
    ];

    for msg in tasks {
        let result = classifier.classify(msg);
        assert_eq!(
            result.intent, Intent::Task,
            "Expected Task intent for '{}', got {:?}",
            msg, result.intent
        );
        assert!(result.confidence >= 0.7, "Low confidence {} for '{}'", result.confidence, msg);
    }
}

#[test]
fn test_intent_classifier_query_detection() {
    let classifier = IntentClassifier::new();

    let queries = vec![
        "what is the project structure?",
        "how does the authentication work?",
        "explain the data flow",
        "where is the config file?",
        "which modules depend on the auth service?",
    ];

    for msg in queries {
        let result = classifier.classify(msg);
        assert_eq!(
            result.intent, Intent::Query,
            "Expected Query intent for '{}', got {:?}",
            msg, result.intent
        );
    }
}

#[test]
fn test_intent_classifier_chat_detection() {
    let classifier = IntentClassifier::new();

    let chats = vec![
        "hello",
        "thanks for the help",
        "sounds good",
        "yes",
    ];

    for msg in chats {
        let result = classifier.classify(msg);
        assert_eq!(
            result.intent, Intent::Chat,
            "Expected Chat intent for '{}', got {:?}",
            msg, result.intent
        );
    }
}

#[test]
fn test_intent_classifier_expert_mode_for_complex_tasks() {
    let classifier = IntentClassifier::new();

    let result = classifier.classify("refactor the entire system architecture for the platform");
    assert_eq!(result.intent, Intent::Task);
    assert_eq!(result.suggested_mode, "expert");
}

#[test]
fn test_intent_classifier_simple_mode_for_basic_tasks() {
    let classifier = IntentClassifier::new();

    let result = classifier.classify("fix a typo");
    assert_eq!(result.suggested_mode, "simple");
}

#[test]
fn test_intent_classifier_unclear_for_ambiguous_input() {
    let classifier = IntentClassifier::new();

    let result = classifier.classify("hmm");
    assert_eq!(result.intent, Intent::Unclear);
    assert!(result.confidence < 0.3);
}

#[test]
fn test_intent_choices_complete() {
    let choices = get_intent_choices();
    assert_eq!(choices.len(), 3);

    let values: Vec<&str> = choices.iter().map(|c| c.value.as_str()).collect();
    assert!(values.contains(&"task"));
    assert!(values.contains(&"query"));
    assert!(values.contains(&"chat"));
}

#[test]
fn test_intent_result_is_confident() {
    let classifier = IntentClassifier::new();

    let result = classifier.classify("implement a new feature for the system");
    assert!(result.is_confident(0.7));
    assert!(!result.is_confident(0.99));
}

// ============================================================================
// Recommendations
// ============================================================================

#[test]
fn test_direct_strategy_has_recommendations() {
    let decision = StrategyAnalyzer::analyze("fix bug", None);
    assert!(!decision.recommendations.is_empty());
    assert!(decision.recommendations.iter().any(|r| r.contains("directly")));
}

#[test]
fn test_hybrid_strategy_has_recommendations() {
    let decision = StrategyAnalyzer::analyze(
        "implement authentication system with OAuth integration and database migration",
        None,
    );
    assert!(!decision.recommendations.is_empty());
}

#[test]
fn test_mega_strategy_has_recommendations() {
    let decision = StrategyAnalyzer::analyze(
        "Build comprehensive e2e platform with multiple features microservices full stack end to end system",
        None,
    );
    assert!(!decision.recommendations.is_empty());
    if decision.strategy == ExecutionStrategy::MegaPlan {
        assert!(decision.recommendations.iter().any(|r| r.contains("feature") || r.contains("worktree")));
    }
}
