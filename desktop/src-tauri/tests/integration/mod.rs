//! Integration Tests Module
//!
//! This module contains comprehensive integration tests for Plan Cascade Desktop v5.0.
//! Tests cover quality gates, worktree management, standalone execution, Claude Code integration,
//! strategy analysis, spec interviews, design document generation/import, and recovery system.

// Quality gates detection and execution tests
mod quality_gates_test;

// Worktree lifecycle tests
mod worktree_test;

// Standalone LLM execution tests
mod standalone_test;

// Claude Code integration tests
mod claude_code_test;

// Strategy analyzer integration tests (story-010)
mod strategy_test;

// Spec interview service integration tests (story-010)
mod spec_interview_test;

// Design document generator/importer integration tests (story-010)
mod design_doc_test;

// Recovery system integration tests (story-010)
mod recovery_test;

// Tool calling integration tests (story-006)
mod tool_calling_test;

// Prompt fallback integration tests (story-006)
mod prompt_fallback_test;
