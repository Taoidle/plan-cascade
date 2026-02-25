//! Interview Manager
//!
//! Manages multi-turn LLM-driven conversations that generate contextual
//! follow-up questions to elicit project requirements.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::services::persona::prompt_builder;
use crate::services::persona::registry::PersonaRegistry;
use crate::services::persona::types::PersonaRole;
use crate::utils::error::{AppError, AppResult};

use super::state::{InterviewStateManager, InterviewTurn, PersistedInterviewState};

/// Interview phase determines which section of the spec is being explored
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterviewPhase {
    /// Project overview: title, goal, success metrics
    Overview,
    /// Scope definition: in/out of scope, assumptions
    Scope,
    /// Requirements: functional and non-functional
    Requirements,
    /// Interfaces: APIs, data models
    Interfaces,
    /// Stories: task decomposition
    Stories,
    /// Open questions and review
    Review,
    /// Interview complete
    Complete,
}

impl InterviewPhase {
    /// Get the display label for this phase
    pub fn label(&self) -> &str {
        match self {
            Self::Overview => "Overview",
            Self::Scope => "Scope",
            Self::Requirements => "Requirements",
            Self::Interfaces => "Interfaces",
            Self::Stories => "Stories",
            Self::Review => "Review",
            Self::Complete => "Complete",
        }
    }

    /// Get the phase index (0-based) for progress calculation
    pub fn index(&self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Scope => 1,
            Self::Requirements => 2,
            Self::Interfaces => 3,
            Self::Stories => 4,
            Self::Review => 5,
            Self::Complete => 6,
        }
    }

    /// Total number of phases (excluding Complete)
    pub fn total_phases() -> usize {
        6
    }

    /// Get the next phase
    pub fn next(&self) -> Self {
        match self {
            Self::Overview => Self::Scope,
            Self::Scope => Self::Requirements,
            Self::Requirements => Self::Interfaces,
            Self::Interfaces => Self::Stories,
            Self::Stories => Self::Review,
            Self::Review => Self::Complete,
            Self::Complete => Self::Complete,
        }
    }

    /// Get the string form for database storage
    pub fn as_str(&self) -> &str {
        match self {
            Self::Overview => "overview",
            Self::Scope => "scope",
            Self::Requirements => "requirements",
            Self::Interfaces => "interfaces",
            Self::Stories => "stories",
            Self::Review => "review",
            Self::Complete => "complete",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "overview" => Self::Overview,
            "scope" => Self::Scope,
            "requirements" => Self::Requirements,
            "interfaces" => Self::Interfaces,
            "stories" => Self::Stories,
            "review" => Self::Review,
            "complete" => Self::Complete,
            _ => Self::Overview,
        }
    }
}

/// A question generated for the interview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewQuestion {
    /// Unique question ID
    pub id: String,
    /// The question text to display to the user
    pub question: String,
    /// Phase this question belongs to
    pub phase: InterviewPhase,
    /// Optional hint/placeholder for the input field
    pub hint: Option<String>,
    /// Whether the answer is required (vs optional)
    pub required: bool,
    /// Input type: "text", "textarea", "list", "boolean"
    pub input_type: String,
    /// Phase-specific field name (e.g. "title", "goal", "functional_requirements")
    pub field_name: String,
}

/// Configuration for starting a new interview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewConfig {
    /// Project description / initial intent
    pub description: String,
    /// Flow level: "quick", "standard", "full"
    #[serde(default = "default_flow_level")]
    pub flow_level: String,
    /// Maximum number of questions (soft cap)
    #[serde(default = "default_max_questions")]
    pub max_questions: i32,
    /// Whether to ask first-principles questions
    #[serde(default)]
    pub first_principles: bool,
    /// Optional project path for context
    pub project_path: Option<String>,
    /// Optional exploration result for BA context
    #[serde(default)]
    pub exploration_context: Option<String>,
}

fn default_flow_level() -> String {
    "standard".to_string()
}

fn default_max_questions() -> i32 {
    18
}

/// The interview manager orchestrating multi-turn conversations
pub struct InterviewManager {
    state_manager: InterviewStateManager,
}

impl InterviewManager {
    /// Create a new interview manager with the given state manager
    pub fn new(state_manager: InterviewStateManager) -> Self {
        Self { state_manager }
    }

    /// Start a new interview session (deterministic fallback)
    pub fn start_interview(&self, config: InterviewConfig) -> AppResult<InterviewSession> {
        let interview_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Store exploration context in conversation_context
        let conversation_context = if let Some(ref ctx) = config.exploration_context {
            serde_json::json!({ "exploration_context": ctx }).to_string()
        } else {
            "{}".to_string()
        };

        let state = PersistedInterviewState {
            id: interview_id.clone(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: config.flow_level.clone(),
            first_principles: config.first_principles,
            max_questions: config.max_questions,
            question_cursor: 0,
            description: config.description.clone(),
            project_path: config.project_path.clone(),
            spec_data: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now,
            conversation_context,
        };

        self.state_manager.create_interview(&state)?;

        let first_question = self.generate_next_question(&state)?;

        Ok(InterviewSession {
            id: interview_id,
            status: "in_progress".to_string(),
            phase: InterviewPhase::Overview,
            flow_level: config.flow_level,
            description: config.description,
            question_cursor: 0,
            max_questions: config.max_questions,
            current_question: Some(first_question),
            progress: 0.0,
            history: vec![],
        })
    }

    /// Start a new interview session with LLM-driven BA
    pub async fn start_interview_with_llm(
        &self,
        config: InterviewConfig,
        llm_provider: Arc<dyn LlmProvider>,
    ) -> AppResult<InterviewSession> {
        let interview_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let conversation_context = if let Some(ref ctx) = config.exploration_context {
            serde_json::json!({
                "exploration_context": ctx,
                "mode": "llm_driven"
            })
            .to_string()
        } else {
            serde_json::json!({ "mode": "llm_driven" }).to_string()
        };

        let state = PersistedInterviewState {
            id: interview_id.clone(),
            status: "in_progress".to_string(),
            phase: "overview".to_string(),
            flow_level: config.flow_level.clone(),
            first_principles: config.first_principles,
            max_questions: config.max_questions,
            question_cursor: 0,
            description: config.description.clone(),
            project_path: config.project_path.clone(),
            spec_data: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now,
            conversation_context,
        };

        self.state_manager.create_interview(&state)?;

        // Generate first question using LLM BA
        let first_question = self
            .generate_next_question_llm(&state, &[], &llm_provider)
            .await?;

        Ok(InterviewSession {
            id: interview_id,
            status: "in_progress".to_string(),
            phase: InterviewPhase::Overview,
            flow_level: config.flow_level,
            description: config.description,
            question_cursor: 0,
            max_questions: config.max_questions,
            current_question: Some(first_question),
            progress: 0.0,
            history: vec![],
        })
    }

    /// Submit an answer to the current question and get the next question
    pub fn submit_answer(&self, interview_id: &str, answer: &str) -> AppResult<InterviewSession> {
        let mut state = self
            .state_manager
            .get_interview(interview_id)?
            .ok_or_else(|| AppError::not_found(format!("Interview not found: {}", interview_id)))?;

        if state.status == "finalized" || state.status == "complete" {
            return Err(AppError::validation("Interview is already complete"));
        }

        let current_phase = InterviewPhase::from_str(&state.phase);

        // Record the turn
        let turn = InterviewTurn {
            id: Uuid::new_v4().to_string(),
            interview_id: interview_id.to_string(),
            turn_number: state.question_cursor + 1,
            phase: state.phase.clone(),
            question: self.get_question_text_for_phase(&current_phase, &state),
            answer: answer.to_string(),
            field_name: self.get_field_name_for_phase(&current_phase, &state),
            created_at: Utc::now().to_rfc3339(),
        };

        self.state_manager.add_turn(&turn)?;

        // Update the spec data with the answer
        let mut spec_data: serde_json::Value =
            serde_json::from_str(&state.spec_data).unwrap_or(serde_json::json!({}));
        self.apply_answer_to_spec(&mut spec_data, &current_phase, &state, answer);
        state.spec_data = serde_json::to_string(&spec_data).unwrap_or_else(|_| "{}".to_string());

        // Advance the question cursor
        state.question_cursor += 1;

        // Determine if we should advance the phase
        let (new_phase, phase_complete) =
            self.check_phase_transition(&current_phase, &state, &spec_data);

        if phase_complete {
            state.phase = new_phase.as_str().to_string();
        }

        let actual_phase = InterviewPhase::from_str(&state.phase);

        // Check if interview is complete
        if actual_phase == InterviewPhase::Complete {
            state.status = "finalized".to_string();
        }

        state.updated_at = Utc::now().to_rfc3339();
        self.state_manager.update_interview(&state)?;

        // Generate next question
        let next_question = if actual_phase != InterviewPhase::Complete {
            Some(self.generate_next_question(&state)?)
        } else {
            None
        };

        // Load history
        let turns = self.state_manager.get_turns(interview_id)?;
        let history: Vec<InterviewHistoryEntry> = turns
            .into_iter()
            .map(|t| InterviewHistoryEntry {
                turn_number: t.turn_number,
                phase: t.phase,
                question: t.question,
                answer: t.answer,
                timestamp: t.created_at,
            })
            .collect();

        let progress = self.calculate_progress(&actual_phase, &state);

        Ok(InterviewSession {
            id: interview_id.to_string(),
            status: state.status,
            phase: actual_phase,
            flow_level: state.flow_level,
            description: state.description,
            question_cursor: state.question_cursor,
            max_questions: state.max_questions,
            current_question: next_question,
            progress,
            history,
        })
    }

    /// Submit an answer with LLM-driven BA for next question generation
    pub async fn submit_answer_with_llm(
        &self,
        interview_id: &str,
        answer: &str,
        llm_provider: Arc<dyn LlmProvider>,
    ) -> AppResult<InterviewSession> {
        let mut state = self
            .state_manager
            .get_interview(interview_id)?
            .ok_or_else(|| AppError::not_found(format!("Interview not found: {}", interview_id)))?;

        if state.status == "finalized" || state.status == "complete" {
            return Err(AppError::validation("Interview is already complete"));
        }

        // Load existing turns to get the last question text
        let existing_turns = self.state_manager.get_turns(interview_id)?;
        let last_question = existing_turns
            .last()
            .map(|t| t.question.clone())
            .unwrap_or_else(|| "Initial question".to_string());

        // Record the turn
        let turn = InterviewTurn {
            id: Uuid::new_v4().to_string(),
            interview_id: interview_id.to_string(),
            turn_number: state.question_cursor + 1,
            phase: state.phase.clone(),
            question: last_question,
            answer: answer.to_string(),
            field_name: "ba_response".to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        self.state_manager.add_turn(&turn)?;
        state.question_cursor += 1;

        // Check if max questions reached
        let force_complete = state.question_cursor >= state.max_questions;

        // Load all turns for BA context
        let all_turns = self.state_manager.get_turns(interview_id)?;

        // Generate next question using LLM
        let ba_result = if force_complete {
            BaQuestionResult::Complete {
                summary: "Maximum number of questions reached. Finalizing interview.".to_string(),
            }
        } else {
            match self
                .generate_next_question_llm(&state, &all_turns, &llm_provider)
                .await
            {
                Ok(q) => BaQuestionResult::Question(q),
                Err(e) if e.to_string().contains("INTERVIEW_COMPLETE") => {
                    // BA determined interview has sufficient information
                    BaQuestionResult::Complete {
                        summary: "BA determined the interview has sufficient information."
                            .to_string(),
                    }
                }
                Err(e) => {
                    // Real error — fall back to deterministic question generation
                    debug!(error = %e, "LLM BA question generation failed, using deterministic fallback");
                    match self.generate_next_question(&state) {
                        Ok(q) => BaQuestionResult::Question(q),
                        Err(_) => BaQuestionResult::Complete {
                            summary: "Interview complete (fallback).".to_string(),
                        },
                    }
                }
            }
        };

        let (next_question, is_complete) = match ba_result {
            BaQuestionResult::Question(q) => (Some(q), false),
            BaQuestionResult::Complete { summary } => {
                // Extract structured spec_data from conversation
                if let Ok(spec_data) = self
                    .extract_spec_data_from_conversation(&state, &all_turns, &llm_provider)
                    .await
                {
                    state.spec_data =
                        serde_json::to_string(&spec_data).unwrap_or_else(|_| "{}".to_string());
                }

                // Store summary in conversation_context
                let mut ctx: serde_json::Value = serde_json::from_str(&state.conversation_context)
                    .unwrap_or(serde_json::json!({}));
                if let Some(o) = ctx.as_object_mut() {
                    o.insert(
                        "completion_summary".to_string(),
                        serde_json::Value::String(summary),
                    );
                }
                state.conversation_context =
                    serde_json::to_string(&ctx).unwrap_or_else(|_| "{}".to_string());

                (None, true)
            }
        };

        if is_complete {
            state.phase = InterviewPhase::Complete.as_str().to_string();
            state.status = "finalized".to_string();
        }

        state.updated_at = Utc::now().to_rfc3339();
        self.state_manager.update_interview(&state)?;

        // Build history
        let final_turns = self.state_manager.get_turns(interview_id)?;
        let history: Vec<InterviewHistoryEntry> = final_turns
            .into_iter()
            .map(|t| InterviewHistoryEntry {
                turn_number: t.turn_number,
                phase: t.phase,
                question: t.question,
                answer: t.answer,
                timestamp: t.created_at,
            })
            .collect();

        let actual_phase = InterviewPhase::from_str(&state.phase);
        let progress = self.calculate_progress(&actual_phase, &state);

        Ok(InterviewSession {
            id: interview_id.to_string(),
            status: state.status,
            phase: actual_phase,
            flow_level: state.flow_level,
            description: state.description,
            question_cursor: state.question_cursor,
            max_questions: state.max_questions,
            current_question: next_question,
            progress,
            history,
        })
    }

    /// Get the current interview state
    pub fn get_interview_state(&self, interview_id: &str) -> AppResult<InterviewSession> {
        let state = self
            .state_manager
            .get_interview(interview_id)?
            .ok_or_else(|| AppError::not_found(format!("Interview not found: {}", interview_id)))?;

        let phase = InterviewPhase::from_str(&state.phase);

        let next_question = if phase != InterviewPhase::Complete && state.status == "in_progress" {
            Some(self.generate_next_question(&state)?)
        } else {
            None
        };

        let turns = self.state_manager.get_turns(interview_id)?;
        let history: Vec<InterviewHistoryEntry> = turns
            .into_iter()
            .map(|t| InterviewHistoryEntry {
                turn_number: t.turn_number,
                phase: t.phase,
                question: t.question,
                answer: t.answer,
                timestamp: t.created_at,
            })
            .collect();

        let progress = self.calculate_progress(&phase, &state);

        Ok(InterviewSession {
            id: interview_id.to_string(),
            status: state.status,
            phase,
            flow_level: state.flow_level,
            description: state.description,
            question_cursor: state.question_cursor,
            max_questions: state.max_questions,
            current_question: next_question,
            progress,
            history,
        })
    }

    /// Get the raw spec data for compilation
    pub fn get_spec_data(&self, interview_id: &str) -> AppResult<serde_json::Value> {
        let state = self
            .state_manager
            .get_interview(interview_id)?
            .ok_or_else(|| AppError::not_found(format!("Interview not found: {}", interview_id)))?;

        serde_json::from_str(&state.spec_data)
            .map_err(|e| AppError::parse(format!("Failed to parse spec data: {}", e)))
    }

    // ========================================================================
    // Internal question generation logic
    // ========================================================================

    /// Generate the next question based on current phase and state
    fn generate_next_question(
        &self,
        state: &PersistedInterviewState,
    ) -> AppResult<InterviewQuestion> {
        let phase = InterviewPhase::from_str(&state.phase);
        let spec_data: serde_json::Value =
            serde_json::from_str(&state.spec_data).unwrap_or(serde_json::json!({}));

        let question = match phase {
            InterviewPhase::Overview => self.gen_overview_question(&spec_data, state),
            InterviewPhase::Scope => self.gen_scope_question(&spec_data, state),
            InterviewPhase::Requirements => self.gen_requirements_question(&spec_data, state),
            InterviewPhase::Interfaces => self.gen_interfaces_question(&spec_data, state),
            InterviewPhase::Stories => self.gen_stories_question(&spec_data, state),
            InterviewPhase::Review => self.gen_review_question(&spec_data, state),
            InterviewPhase::Complete => {
                return Err(AppError::validation("Interview is already complete"));
            }
        };

        Ok(question)
    }

    fn gen_overview_question(
        &self,
        spec: &serde_json::Value,
        state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let overview = spec
            .get("overview")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if overview
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .is_empty()
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What is the title of this project/feature?".to_string(),
                phase: InterviewPhase::Overview,
                hint: Some("e.g., User Authentication System".to_string()),
                required: true,
                input_type: "text".to_string(),
                field_name: "title".to_string(),
            };
        }

        if state.first_principles
            && overview
                .get("problem")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What is the core problem you are trying to solve? (first principles)"
                    .to_string(),
                phase: InterviewPhase::Overview,
                hint: Some(
                    "Describe the fundamental problem without assuming a solution".to_string(),
                ),
                required: true,
                input_type: "textarea".to_string(),
                field_name: "problem".to_string(),
            };
        }

        if overview
            .get("goal")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .is_empty()
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What is the primary goal? (one sentence)".to_string(),
                phase: InterviewPhase::Overview,
                hint: Some(state.description.clone()),
                required: true,
                input_type: "textarea".to_string(),
                field_name: "goal".to_string(),
            };
        }

        if overview
            .get("success_metrics")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What are the success metrics for this project?".to_string(),
                phase: InterviewPhase::Overview,
                hint: Some("Comma-separated list of measurable outcomes".to_string()),
                required: state.flow_level == "full",
                input_type: "list".to_string(),
                field_name: "success_metrics".to_string(),
            };
        }

        if overview
            .get("non_goals")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What are the non-goals / out of scope items?".to_string(),
                phase: InterviewPhase::Overview,
                hint: Some("Things explicitly NOT included in this work".to_string()),
                required: state.flow_level == "full",
                input_type: "list".to_string(),
                field_name: "non_goals".to_string(),
            };
        }

        // Phase complete - generate a transition question
        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Overview looks good. Anything else to add before moving to Scope?"
                .to_string(),
            phase: InterviewPhase::Overview,
            hint: Some("Type 'next' to continue or add additional context".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_transition".to_string(),
        }
    }

    fn gen_scope_question(
        &self,
        spec: &serde_json::Value,
        state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let scope = spec.get("scope").cloned().unwrap_or(serde_json::json!({}));

        if scope
            .get("in_scope")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What is in scope for this project?".to_string(),
                phase: InterviewPhase::Scope,
                hint: Some("List the key deliverables and areas of work".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "in_scope".to_string(),
            };
        }

        if scope
            .get("out_of_scope")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What is explicitly out of scope?".to_string(),
                phase: InterviewPhase::Scope,
                hint: Some("Items that will NOT be addressed".to_string()),
                required: state.flow_level == "full",
                input_type: "list".to_string(),
                field_name: "out_of_scope".to_string(),
            };
        }

        if scope
            .get("do_not_touch")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question:
                    "Are there any modules, files, or components that should NOT be modified?"
                        .to_string(),
                phase: InterviewPhase::Scope,
                hint: Some("Files/modules to preserve as-is".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "do_not_touch".to_string(),
            };
        }

        if scope
            .get("assumptions")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What assumptions are being made?".to_string(),
                phase: InterviewPhase::Scope,
                hint: Some("e.g., Node.js 20+ available, database already configured".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "assumptions".to_string(),
            };
        }

        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Scope looks good. Anything else before moving to Requirements?".to_string(),
            phase: InterviewPhase::Scope,
            hint: Some("Type 'next' to continue".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_transition".to_string(),
        }
    }

    fn gen_requirements_question(
        &self,
        spec: &serde_json::Value,
        state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let reqs = spec
            .get("requirements")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if reqs
            .get("functional")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What are the functional requirements?".to_string(),
                phase: InterviewPhase::Requirements,
                hint: Some("List the key features/behaviors the system must implement".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "functional".to_string(),
            };
        }

        let nfr = reqs
            .get("non_functional")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if nfr
            .get("performance_targets")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "Any performance targets or constraints?".to_string(),
                phase: InterviewPhase::Requirements,
                hint: Some(
                    "e.g., API response time < 200ms, support 1000 concurrent users".to_string(),
                ),
                required: false,
                input_type: "list".to_string(),
                field_name: "performance_targets".to_string(),
            };
        }

        if nfr
            .get("security")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "Any security requirements?".to_string(),
                phase: InterviewPhase::Requirements,
                hint: Some("e.g., authentication required, data encryption at rest".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "security".to_string(),
            };
        }

        // For quick flow, skip remaining NFR
        if state.flow_level != "quick" {
            if nfr
                .get("reliability")
                .and_then(|v| v.as_array())
                .map_or(true, |a| a.is_empty())
            {
                return InterviewQuestion {
                    id: Uuid::new_v4().to_string(),
                    question: "Any reliability expectations?".to_string(),
                    phase: InterviewPhase::Requirements,
                    hint: Some("e.g., 99.9% uptime, graceful degradation".to_string()),
                    required: false,
                    input_type: "list".to_string(),
                    field_name: "reliability".to_string(),
                };
            }

            if nfr
                .get("scalability")
                .and_then(|v| v.as_array())
                .map_or(true, |a| a.is_empty())
            {
                return InterviewQuestion {
                    id: Uuid::new_v4().to_string(),
                    question: "Any scalability expectations?".to_string(),
                    phase: InterviewPhase::Requirements,
                    hint: Some("e.g., horizontal scaling, handle 10x traffic growth".to_string()),
                    required: false,
                    input_type: "list".to_string(),
                    field_name: "scalability".to_string(),
                };
            }

            if nfr
                .get("accessibility")
                .and_then(|v| v.as_array())
                .map_or(true, |a| a.is_empty())
            {
                return InterviewQuestion {
                    id: Uuid::new_v4().to_string(),
                    question: "Any accessibility requirements?".to_string(),
                    phase: InterviewPhase::Requirements,
                    hint: Some("e.g., WCAG 2.1 AA compliance, keyboard navigation".to_string()),
                    required: false,
                    input_type: "list".to_string(),
                    field_name: "accessibility".to_string(),
                };
            }
        }

        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Requirements captured. Anything else before moving to Interfaces?"
                .to_string(),
            phase: InterviewPhase::Requirements,
            hint: Some("Type 'next' to continue".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_transition".to_string(),
        }
    }

    fn gen_interfaces_question(
        &self,
        spec: &serde_json::Value,
        _state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let interfaces = spec
            .get("interfaces")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        if interfaces
            .get("api")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What API endpoints or interfaces does this project expose?".to_string(),
                phase: InterviewPhase::Interfaces,
                hint: Some("e.g., POST /api/auth/login - User authentication".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "api".to_string(),
            };
        }

        if interfaces
            .get("data_models")
            .and_then(|v| v.as_array())
            .map_or(true, |a| a.is_empty())
        {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "What are the key data models?".to_string(),
                phase: InterviewPhase::Interfaces,
                hint: Some("e.g., User: id, email, name, created_at".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "data_models".to_string(),
            };
        }

        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Interfaces defined. Ready to decompose into stories?".to_string(),
            phase: InterviewPhase::Interfaces,
            hint: Some("Type 'next' to continue".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_transition".to_string(),
        }
    }

    fn gen_stories_question(
        &self,
        spec: &serde_json::Value,
        _state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let stories = spec.get("stories").and_then(|v| v.as_array());

        if stories.map_or(true, |a| a.is_empty()) {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "How many implementation stories should this project have?".to_string(),
                phase: InterviewPhase::Stories,
                hint: Some("Recommended: 3-7 stories for standard flow".to_string()),
                required: true,
                input_type: "text".to_string(),
                field_name: "story_count".to_string(),
            };
        }

        // Check if all stories have titles
        let arr = stories.unwrap();
        let incomplete_idx = arr.iter().position(|s| {
            s.get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .is_empty()
        });

        if let Some(idx) = incomplete_idx {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: format!(
                    "Define story {} - What is the title and description?",
                    idx + 1
                ),
                phase: InterviewPhase::Stories,
                hint: Some("Format: Title | Brief description of the story".to_string()),
                required: true,
                input_type: "textarea".to_string(),
                field_name: format!("story_{}", idx),
            };
        }

        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Stories defined. Ready to review the complete specification?".to_string(),
            phase: InterviewPhase::Stories,
            hint: Some("Type 'next' to continue to review".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_transition".to_string(),
        }
    }

    fn gen_review_question(
        &self,
        spec: &serde_json::Value,
        _state: &PersistedInterviewState,
    ) -> InterviewQuestion {
        let open_questions = spec.get("open_questions").and_then(|v| v.as_array());

        if open_questions.map_or(true, |a| a.is_empty()) {
            return InterviewQuestion {
                id: Uuid::new_v4().to_string(),
                question: "Any open questions or concerns about this specification?".to_string(),
                phase: InterviewPhase::Review,
                hint: Some("Leave empty or type 'done' to finalize".to_string()),
                required: false,
                input_type: "list".to_string(),
                field_name: "open_questions".to_string(),
            };
        }

        InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: "Specification review complete. Type 'done' to finalize.".to_string(),
            phase: InterviewPhase::Review,
            hint: Some("Type 'done' to compile spec, or add more details".to_string()),
            required: false,
            input_type: "text".to_string(),
            field_name: "_finalize".to_string(),
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn get_question_text_for_phase(
        &self,
        phase: &InterviewPhase,
        state: &PersistedInterviewState,
    ) -> String {
        // Best effort: regenerate the question to get its text
        if let Ok(q) = self.generate_next_question(state) {
            q.question
        } else {
            format!("{} question", phase.label())
        }
    }

    fn get_field_name_for_phase(
        &self,
        phase: &InterviewPhase,
        state: &PersistedInterviewState,
    ) -> String {
        if let Ok(q) = self.generate_next_question(state) {
            q.field_name
        } else {
            "_unknown".to_string()
        }
    }

    /// Apply the user's answer to the spec data structure
    fn apply_answer_to_spec(
        &self,
        spec: &mut serde_json::Value,
        phase: &InterviewPhase,
        state: &PersistedInterviewState,
        answer: &str,
    ) {
        let answer = answer.trim();

        // Skip transition answers
        if answer.eq_ignore_ascii_case("next")
            || answer.eq_ignore_ascii_case("done")
            || answer.is_empty()
        {
            return;
        }

        // Determine the field being answered by regenerating the question
        let field_name = if let Ok(q) = self.generate_next_question(state) {
            q.field_name
        } else {
            return;
        };

        if field_name.starts_with('_') {
            return; // Transition questions
        }

        match phase {
            InterviewPhase::Overview => {
                let overview = spec.get_mut("overview").and_then(|v| v.as_object_mut());
                let overview = if overview.is_none() {
                    spec.as_object_mut()
                        .unwrap()
                        .insert("overview".to_string(), serde_json::json!({}));
                    spec.get_mut("overview").unwrap().as_object_mut().unwrap()
                } else {
                    overview.unwrap()
                };

                match field_name.as_str() {
                    "title" | "goal" | "problem" => {
                        overview.insert(field_name, serde_json::Value::String(answer.to_string()));
                    }
                    "success_metrics" | "non_goals" => {
                        let items: Vec<serde_json::Value> = parse_list_answer(answer)
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect();
                        overview.insert(field_name, serde_json::Value::Array(items));
                    }
                    _ => {}
                }
            }
            InterviewPhase::Scope => {
                let scope = ensure_object(spec, "scope");
                let items: Vec<serde_json::Value> = parse_list_answer(answer)
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
                scope.insert(field_name, serde_json::Value::Array(items));
            }
            InterviewPhase::Requirements => match field_name.as_str() {
                "functional" => {
                    let reqs = ensure_object(spec, "requirements");
                    let items: Vec<serde_json::Value> = parse_list_answer(answer)
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect();
                    reqs.insert(field_name, serde_json::Value::Array(items));
                }
                "performance_targets"
                | "security"
                | "reliability"
                | "scalability"
                | "accessibility" => {
                    let reqs = ensure_object(spec, "requirements");
                    let nfr = if reqs
                        .get("non_functional")
                        .and_then(|v| v.as_object())
                        .is_none()
                    {
                        reqs.insert("non_functional".to_string(), serde_json::json!({}));
                        reqs.get_mut("non_functional")
                            .unwrap()
                            .as_object_mut()
                            .unwrap()
                    } else {
                        reqs.get_mut("non_functional")
                            .unwrap()
                            .as_object_mut()
                            .unwrap()
                    };
                    let items: Vec<serde_json::Value> = parse_list_answer(answer)
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect();
                    nfr.insert(field_name, serde_json::Value::Array(items));
                }
                _ => {}
            },
            InterviewPhase::Interfaces => {
                let interfaces = ensure_object(spec, "interfaces");
                match field_name.as_str() {
                    "api" => {
                        let items: Vec<serde_json::Value> = parse_list_answer(answer)
                            .into_iter()
                            .map(|item| {
                                if let Some((name, notes)) = item.split_once(" - ") {
                                    serde_json::json!({ "name": name.trim(), "notes": notes.trim() })
                                } else {
                                    serde_json::json!({ "name": item.trim(), "notes": "" })
                                }
                            })
                            .collect();
                        interfaces.insert(field_name, serde_json::Value::Array(items));
                    }
                    "data_models" => {
                        let items: Vec<serde_json::Value> = parse_list_answer(answer)
                            .into_iter()
                            .map(|item| {
                                if let Some((name, fields_str)) = item.split_once(':') {
                                    let fields: Vec<serde_json::Value> = fields_str
                                        .split(',')
                                        .map(|f| serde_json::Value::String(f.trim().to_string()))
                                        .filter(|f| !f.as_str().unwrap_or("").is_empty())
                                        .collect();
                                    serde_json::json!({ "name": name.trim(), "fields": fields })
                                } else {
                                    serde_json::json!({ "name": item.trim(), "fields": [] })
                                }
                            })
                            .collect();
                        interfaces.insert(field_name, serde_json::Value::Array(items));
                    }
                    _ => {}
                }
            }
            InterviewPhase::Stories => {
                match field_name.as_str() {
                    "story_count" => {
                        let count: usize = answer.parse().unwrap_or(3);
                        let stories: Vec<serde_json::Value> = (1..=count)
                            .map(|i| {
                                serde_json::json!({
                                    "id": format!("story-{:03}", i),
                                    "category": "core",
                                    "title": "",
                                    "description": "",
                                    "acceptance_criteria": [],
                                    "verification": { "commands": [], "manual_steps": [] },
                                    "dependencies": [],
                                    "context_estimate": "medium"
                                })
                            })
                            .collect();
                        spec.as_object_mut()
                            .unwrap()
                            .insert("stories".to_string(), serde_json::Value::Array(stories));
                    }
                    f if f.starts_with("story_") => {
                        let idx: usize = f.trim_start_matches("story_").parse().unwrap_or(0);
                        if let Some(stories) =
                            spec.get_mut("stories").and_then(|v| v.as_array_mut())
                        {
                            if let Some(story) = stories.get_mut(idx) {
                                // Parse "Title | Description" format
                                let (title, description) =
                                    if let Some((t, d)) = answer.split_once('|') {
                                        (t.trim().to_string(), d.trim().to_string())
                                    } else {
                                        (answer.to_string(), answer.to_string())
                                    };
                                story.as_object_mut().map(|s| {
                                    s.insert("title".to_string(), serde_json::Value::String(title));
                                    s.insert(
                                        "description".to_string(),
                                        serde_json::Value::String(description),
                                    );
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            InterviewPhase::Review => {
                if field_name == "open_questions" {
                    let items: Vec<serde_json::Value> = parse_list_answer(answer)
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect();
                    spec.as_object_mut().unwrap().insert(
                        "open_questions".to_string(),
                        serde_json::Value::Array(items),
                    );
                }
            }
            InterviewPhase::Complete => {}
        }
    }

    /// Check if the current phase should transition to the next
    fn check_phase_transition(
        &self,
        current_phase: &InterviewPhase,
        state: &PersistedInterviewState,
        spec: &serde_json::Value,
    ) -> (InterviewPhase, bool) {
        // Check if the last answer was a transition signal
        let turns = self.state_manager.get_turns(&state.id).unwrap_or_default();
        let last_answer = turns
            .last()
            .map(|t| t.answer.trim().to_lowercase())
            .unwrap_or_default();

        let is_transition =
            last_answer == "next" || last_answer == "done" || last_answer.is_empty();

        // Also check if the phase fields are sufficiently filled
        let phase_filled = match current_phase {
            InterviewPhase::Overview => {
                let overview = spec
                    .get("overview")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                !overview
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty()
                    && !overview
                        .get("goal")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .is_empty()
            }
            InterviewPhase::Scope => {
                let scope = spec.get("scope").cloned().unwrap_or(serde_json::json!({}));
                scope
                    .get("in_scope")
                    .and_then(|v| v.as_array())
                    .map_or(false, |a| !a.is_empty())
            }
            InterviewPhase::Requirements => {
                let reqs = spec
                    .get("requirements")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                reqs.get("functional")
                    .and_then(|v| v.as_array())
                    .map_or(false, |a| !a.is_empty())
            }
            InterviewPhase::Interfaces => true, // Interfaces are optional
            InterviewPhase::Stories => {
                let stories = spec.get("stories").and_then(|v| v.as_array());
                stories.map_or(false, |arr| {
                    !arr.is_empty()
                        && arr.iter().all(|s| {
                            !s.get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .is_empty()
                        })
                })
            }
            InterviewPhase::Review => is_transition || last_answer == "done",
            InterviewPhase::Complete => true,
        };

        if is_transition && phase_filled {
            let next = current_phase.next();
            (next, true)
        } else if phase_filled && !is_transition {
            // Check if the question would be a transition question
            if let Ok(q) = self.generate_next_question(state) {
                if q.field_name.starts_with('_') {
                    // The next question is a transition - auto advance
                    let next = current_phase.next();
                    return (next, true);
                }
            }
            (current_phase.clone(), false)
        } else {
            (current_phase.clone(), false)
        }
    }

    /// Calculate overall interview progress as a percentage
    fn calculate_progress(&self, phase: &InterviewPhase, state: &PersistedInterviewState) -> f64 {
        let phase_progress = phase.index() as f64 / InterviewPhase::total_phases() as f64;
        // Weight by questions within phase
        let q_weight =
            (state.question_cursor as f64 / state.max_questions.max(1) as f64).min(1.0) * 0.1;
        ((phase_progress + q_weight) * 100.0).min(100.0)
    }

    // ========================================================================
    // LLM-driven BA methods
    // ========================================================================

    /// Generate the next question using LLM-driven Business Analyst persona.
    ///
    /// Builds a full conversation history and asks the BA to generate the next
    /// contextual follow-up question. Returns `[INTERVIEW_COMPLETE]` detection
    /// as the BA deciding the interview has sufficient information.
    async fn generate_next_question_llm(
        &self,
        state: &PersistedInterviewState,
        turns: &[InterviewTurn],
        provider: &Arc<dyn LlmProvider>,
    ) -> AppResult<InterviewQuestion> {
        let persona = PersonaRegistry::get(PersonaRole::BusinessAnalyst);

        // Extract exploration context from conversation_context
        let conv_ctx: serde_json::Value =
            serde_json::from_str(&state.conversation_context).unwrap_or(serde_json::json!({}));
        let exploration_context = conv_ctx
            .get("exploration_context")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Build BA instructions
        let phase_instructions = format!(
            r#"You are conducting a requirements interview for the following project:
"{description}"

Flow level: {flow_level} (quick = fewer questions, standard = balanced, full = comprehensive)
Questions answered so far: {cursor} / {max}

Based on the conversation so far, ask the next most valuable question to better understand the project requirements. Your question should:
- Build on previous answers and not repeat what's already been discussed
- Explore areas that haven't been covered yet
- Be specific and actionable
- Help create a complete project specification covering: goals, scope, requirements, interfaces, and stories

When you have gathered enough information for a complete specification, respond with EXACTLY:
[INTERVIEW_COMPLETE]
Followed by a brief summary of the key findings.

Otherwise, respond with ONLY your next question. No formatting, no preamble — just the question text."#,
            description = state.description,
            flow_level = state.flow_level,
            cursor = state.question_cursor,
            max = state.max_questions,
        );

        let system_prompt = prompt_builder::build_expert_system_prompt(
            &persona,
            &phase_instructions,
            exploration_context.as_deref(),
        );

        // Build conversation messages from turns
        let mut messages = Vec::new();

        // Initial context message
        messages.push(Message::user(format!(
            "I want to build: {}. Please begin the requirements interview.",
            state.description
        )));

        // Add conversation history as alternating assistant/user messages
        for turn in turns {
            messages.push(Message::assistant(&turn.question));
            messages.push(Message::user(&turn.answer));
        }

        let options = LlmRequestOptions {
            temperature_override: Some(persona.expert_temperature),
            ..Default::default()
        };

        debug!(
            interview_id = %state.id,
            turn_count = turns.len(),
            "BA: generating next question via LLM"
        );

        let response = provider
            .send_message(messages, Some(system_prompt), vec![], options)
            .await
            .map_err(|e| AppError::command(format!("BA LLM call failed: {}", e)))?;

        let response_text = response
            .content
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(response.thinking.as_deref())
            .ok_or_else(|| AppError::parse("BA returned empty response".to_string()))?;

        // Check for completion signal
        if response_text.contains("[INTERVIEW_COMPLETE]") {
            return Err(AppError::validation("INTERVIEW_COMPLETE"));
        }

        // Extract question text (strip any markdown formatting)
        let question_text = response_text
            .trim()
            .trim_start_matches('#')
            .trim_start_matches("**Question:**")
            .trim_start_matches("Question:")
            .trim()
            .to_string();

        if question_text.is_empty() {
            return Err(AppError::parse("BA returned empty question".to_string()));
        }

        Ok(InterviewQuestion {
            id: Uuid::new_v4().to_string(),
            question: question_text,
            phase: InterviewPhase::from_str(&state.phase),
            hint: None,
            required: false,
            input_type: "textarea".to_string(),
            field_name: "ba_response".to_string(),
        })
    }

    /// Extract structured spec_data from the full conversation using LLM formatter.
    ///
    /// Called when the BA signals [INTERVIEW_COMPLETE]. Converts free-form
    /// conversation into the standard spec JSON structure.
    async fn extract_spec_data_from_conversation(
        &self,
        state: &PersistedInterviewState,
        turns: &[InterviewTurn],
        provider: &Arc<dyn LlmProvider>,
    ) -> AppResult<serde_json::Value> {
        let target_schema = r#"{
  "overview": { "title": "string", "goal": "string", "problem": "string (optional)", "success_metrics": ["string"], "non_goals": ["string"] },
  "scope": { "in_scope": ["string"], "out_of_scope": ["string"], "do_not_touch": ["string"], "assumptions": ["string"] },
  "requirements": {
    "functional": ["string"],
    "non_functional": { "performance_targets": ["string"], "security": ["string"], "reliability": ["string"], "scalability": ["string"], "accessibility": ["string"] }
  },
  "interfaces": { "api": [{"name": "string", "notes": "string"}], "data_models": [{"name": "string", "fields": ["string"]}] },
  "stories": [{ "id": "story-NNN", "title": "string", "category": "setup|core|integration|polish|test", "description": "string", "acceptance_criteria": ["string"], "verification": {"commands": ["string"], "manual_steps": ["string"]}, "dependencies": ["story-NNN"], "context_estimate": "small|medium|large" }],
  "open_questions": ["string"]
}"#;

        let formatter_system = prompt_builder::build_formatter_system_prompt(target_schema);

        // Build conversation summary for the formatter
        let mut conversation_text = format!(
            "Project Description: {}\n\nInterview Conversation:\n",
            state.description
        );
        for turn in turns {
            conversation_text.push_str(&format!("\nQ: {}\nA: {}\n", turn.question, turn.answer));
        }

        let formatter_user = prompt_builder::build_formatter_user_message(&conversation_text);

        let options = LlmRequestOptions {
            temperature_override: Some(0.1),
            ..Default::default()
        };

        debug!(
            interview_id = %state.id,
            turn_count = turns.len(),
            "BA: extracting structured spec_data from conversation"
        );

        let response = provider
            .send_message(
                vec![Message::user(&formatter_user)],
                Some(formatter_system.clone()),
                vec![],
                options.clone(),
            )
            .await
            .map_err(|e| AppError::command(format!("Spec extraction failed: {}", e)))?;

        let response_text = response
            .content
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                AppError::parse("Spec extraction returned empty response".to_string())
            })?;

        let json_str = extract_json_from_response(response_text);

        // Try parsing, with one retry on failure
        match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(value) => Ok(value),
            Err(first_error) => {
                debug!(error = %first_error, "Spec extraction parse failed, retrying");

                let repair_msg = format!(
                    "Your previous response could not be parsed as valid JSON.\n\
                     Parse error: {}\n\n\
                     Please respond with ONLY valid JSON matching the schema.",
                    first_error
                );

                let retry_response = provider
                    .send_message(
                        vec![
                            Message::user(&formatter_user),
                            Message::assistant(response_text),
                            Message::user(&repair_msg),
                        ],
                        Some(formatter_system),
                        vec![],
                        options,
                    )
                    .await
                    .map_err(|e| {
                        AppError::command(format!("Spec extraction retry failed: {}", e))
                    })?;

                let retry_text = retry_response
                    .content
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| {
                        AppError::parse("Spec extraction retry returned empty".to_string())
                    })?;

                let retry_json = extract_json_from_response(retry_text);
                serde_json::from_str(&retry_json).map_err(|e| {
                    AppError::parse(format!("Spec extraction failed after retry: {}", e))
                })
            }
        }
    }
}

/// Result from BA question generation
enum BaQuestionResult {
    /// BA generated a follow-up question
    Question(InterviewQuestion),
    /// BA determined interview is complete
    Complete { summary: String },
}

/// Extract JSON from an LLM response string, handling markdown fences.
fn extract_json_from_response(text: &str) -> String {
    let trimmed = text.trim();

    // Try markdown code fences
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        let content_start = if let Some(nl) = after_fence.find('\n') {
            nl + 1
        } else {
            0
        };
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }

    // Try JSON object { ... }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

// ============================================================================
// Response types
// ============================================================================

/// Full interview session state returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewSession {
    /// Interview ID
    pub id: String,
    /// Status: "in_progress", "finalized"
    pub status: String,
    /// Current phase
    pub phase: InterviewPhase,
    /// Flow level
    pub flow_level: String,
    /// Initial description
    pub description: String,
    /// Number of questions answered
    pub question_cursor: i32,
    /// Max questions (soft cap)
    pub max_questions: i32,
    /// The current question to display
    pub current_question: Option<InterviewQuestion>,
    /// Progress percentage (0-100)
    pub progress: f64,
    /// Conversation history
    pub history: Vec<InterviewHistoryEntry>,
}

/// A single entry in the interview history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterviewHistoryEntry {
    pub turn_number: i32,
    pub phase: String,
    pub question: String,
    pub answer: String,
    pub timestamp: String,
}

// ============================================================================
// Utility functions
// ============================================================================

/// Parse a comma/newline separated answer into a list of strings
fn parse_list_answer(answer: &str) -> Vec<String> {
    let answer = answer.trim();
    if answer.is_empty()
        || answer.eq_ignore_ascii_case("next")
        || answer.eq_ignore_ascii_case("done")
    {
        return vec![];
    }

    answer
        .split(|c: char| c == ',' || c == '\n')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Ensure a top-level object exists in the spec and return a mutable reference
fn ensure_object<'a>(
    spec: &'a mut serde_json::Value,
    key: &str,
) -> &'a mut serde_json::Map<String, serde_json::Value> {
    if !spec.get(key).map_or(false, |v| v.is_object()) {
        spec.as_object_mut()
            .unwrap()
            .insert(key.to_string(), serde_json::json!({}));
    }
    spec.get_mut(key).unwrap().as_object_mut().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_answer() {
        assert_eq!(parse_list_answer("a, b, c"), vec!["a", "b", "c"]);
        assert_eq!(parse_list_answer("a\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(parse_list_answer(""), Vec::<String>::new());
        assert_eq!(parse_list_answer("next"), Vec::<String>::new());
    }

    #[test]
    fn test_interview_phase_order() {
        let phase = InterviewPhase::Overview;
        assert_eq!(phase.next(), InterviewPhase::Scope);
        assert_eq!(phase.next().next(), InterviewPhase::Requirements);
        assert_eq!(InterviewPhase::Complete.next(), InterviewPhase::Complete);
    }

    #[test]
    fn test_interview_phase_index() {
        assert_eq!(InterviewPhase::Overview.index(), 0);
        assert_eq!(InterviewPhase::Complete.index(), 6);
        assert_eq!(InterviewPhase::total_phases(), 6);
    }

    #[test]
    fn test_interview_phase_roundtrip() {
        let phases = vec![
            InterviewPhase::Overview,
            InterviewPhase::Scope,
            InterviewPhase::Requirements,
            InterviewPhase::Interfaces,
            InterviewPhase::Stories,
            InterviewPhase::Review,
            InterviewPhase::Complete,
        ];
        for phase in phases {
            assert_eq!(InterviewPhase::from_str(phase.as_str()), phase);
        }
    }
}
