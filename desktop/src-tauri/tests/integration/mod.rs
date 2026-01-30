//! Integration Tests Module
//!
//! This module contains comprehensive integration tests for Plan Cascade Desktop v5.0.
//! Tests cover quality gates, worktree management, standalone execution, and Claude Code integration.

// Quality gates detection and execution tests
mod quality_gates_test;

// Worktree lifecycle tests
mod worktree_test;

// Standalone LLM execution tests
mod standalone_test;

// Claude Code integration tests
mod claude_code_test;
