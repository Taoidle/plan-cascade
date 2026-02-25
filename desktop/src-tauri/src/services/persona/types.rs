//! Persona Types
//!
//! Core types for the role-based AI persona system.
//! Each workflow phase is assigned a domain-expert persona with specialized prompts.

use serde::{Deserialize, Serialize};

/// AI persona roles assigned to workflow phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaRole {
    /// Tech Lead — analyzing/strategy phase
    TechLead,
    /// Senior Engineer — project exploration phase
    SeniorEngineer,
    /// Business Analyst — interviewing phase
    BusinessAnalyst,
    /// Product Manager — requirement analysis + PRD generation
    ProductManager,
    /// Software Architect — architecture review + design doc generation
    SoftwareArchitect,
    /// Developer — story execution phase
    Developer,
    /// QA Engineer — quality gates and code review
    QaEngineer,
}

impl PersonaRole {
    /// Human-readable display name for the persona.
    pub fn display_name(&self) -> &'static str {
        match self {
            PersonaRole::TechLead => "Tech Lead",
            PersonaRole::SeniorEngineer => "Senior Engineer",
            PersonaRole::BusinessAnalyst => "Business Analyst",
            PersonaRole::ProductManager => "Product Manager",
            PersonaRole::SoftwareArchitect => "Software Architect",
            PersonaRole::Developer => "Developer",
            PersonaRole::QaEngineer => "QA Engineer",
        }
    }

    /// Short identifier for the persona (used in logs and events).
    pub fn id(&self) -> &'static str {
        match self {
            PersonaRole::TechLead => "tech_lead",
            PersonaRole::SeniorEngineer => "senior_engineer",
            PersonaRole::BusinessAnalyst => "business_analyst",
            PersonaRole::ProductManager => "product_manager",
            PersonaRole::SoftwareArchitect => "software_architect",
            PersonaRole::Developer => "developer",
            PersonaRole::QaEngineer => "qa_engineer",
        }
    }
}

impl std::fmt::Display for PersonaRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// A persona definition with rich system prompt and configuration.
#[derive(Debug, Clone)]
pub struct Persona {
    /// The persona role
    pub role: PersonaRole,
    /// Rich role-specific identity prompt (injected as system prompt prefix)
    pub identity_prompt: String,
    /// Guides reasoning approach for expert step
    pub thinking_style: String,
    /// Domain expertise areas
    pub expertise: Vec<String>,
    /// Temperature for expert step (free-form reasoning, e.g. 0.7)
    pub expert_temperature: f32,
    /// Temperature for formatter step (JSON structuring, e.g. 0.1)
    pub formatter_temperature: f32,
}

/// Configuration overrides for the expert-formatter pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaConfig {
    /// Override model for expert step (use a stronger model for reasoning)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_model: Option<String>,
    /// Override model for formatter step (use a cheaper model for JSON structuring)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatter_model: Option<String>,
    /// If true, attempt to parse expert output directly (skip formatter step)
    #[serde(default)]
    pub skip_formatter: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_role_display() {
        assert_eq!(PersonaRole::TechLead.display_name(), "Tech Lead");
        assert_eq!(
            PersonaRole::ProductManager.display_name(),
            "Product Manager"
        );
        assert_eq!(PersonaRole::QaEngineer.display_name(), "QA Engineer");
    }

    #[test]
    fn test_persona_role_id() {
        assert_eq!(PersonaRole::TechLead.id(), "tech_lead");
        assert_eq!(PersonaRole::SoftwareArchitect.id(), "software_architect");
    }

    #[test]
    fn test_persona_role_serialization() {
        let role = PersonaRole::ProductManager;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"product_manager\"");

        let deserialized: PersonaRole = serde_json::from_str("\"tech_lead\"").unwrap();
        assert_eq!(deserialized, PersonaRole::TechLead);
    }

    #[test]
    fn test_persona_config_default() {
        let config = PersonaConfig::default();
        assert!(config.expert_model.is_none());
        assert!(config.formatter_model.is_none());
        assert!(!config.skip_formatter);
    }
}
