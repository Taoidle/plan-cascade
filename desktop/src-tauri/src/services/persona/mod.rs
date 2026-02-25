//! Persona Module
//!
//! Role-based AI persona system for the desktop task mode workflow.
//! Each workflow phase gets a domain-expert persona with specialized system prompts.
//!
//! ## Architecture
//!
//! The persona system uses an **Expert + Formatter** architecture:
//! - **Expert step**: Persona-guided free-form reasoning (natural language)
//! - **Formatter step**: Convert analysis into structured JSON output
//!
//! This separation improves reasoning quality by decoupling thinking from formatting.
//!
//! ## Persona Roles
//!
//! | Role | Phase |
//! |------|-------|
//! | TechLead | Strategy analysis |
//! | SeniorEngineer | Project exploration |
//! | BusinessAnalyst | Spec interview |
//! | ProductManager | Requirement analysis + PRD generation |
//! | SoftwareArchitect | Architecture review + Design doc generation |
//! | Developer | Story execution |
//! | QaEngineer | Quality gates + Code review |

pub mod expert_formatter;
pub mod prompt_builder;
pub mod registry;
pub mod types;

pub use expert_formatter::{run_expert_formatter, ExpertFormatterResult};
pub use prompt_builder::{
    build_expert_system_prompt, build_formatter_system_prompt, build_formatter_user_message,
};
pub use registry::PersonaRegistry;
pub use types::{Persona, PersonaConfig, PersonaRole};
