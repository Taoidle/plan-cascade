//! Curated reliable model catalog shared across providers.
//!
//! This module centralizes which model IDs are considered "reliable" for
//! native tool-calling behavior. Providers can still accept custom model IDs,
//! but unknown models should be treated conservatively.

use crate::types::ProviderType;

pub const ANTHROPIC_MODELS: &[&str] = &[
    "claude-opus-4-6-20260219",
    "claude-sonnet-4-6-20260219",
    "claude-opus-4-5-20250929",
    "claude-sonnet-4-5-20250929",
];

pub const OPENAI_MODELS: &[&str] = &["gpt-5.1", "gpt-5.2", "gpt-5.3"];

pub const GLM_MODELS: &[&str] = &["glm-5", "glm-4.7", "glm-4.6", "glm-4.6v"];

pub const QWEN_MODELS: &[&str] = &["qwen3-max", "qwen3-plus", "qwen3.5-plus"];

pub const DEEPSEEK_MODELS: &[&str] = &["deepseek-chat", "deepseek-reasoner", "deepseek-r1"];

pub const MINIMAX_MODELS: &[&str] = &[
    "minimax-m2.5",
    "minimax-m2.5-highspeed",
    "minimax-m2.1",
    "minimax-m2.1-highspeed",
];

pub const OLLAMA_REFERENCE_MODELS: &[&str] = &["llama3.2", "deepseek-r1:14b", "qwq:32b"];

pub fn allowed_models(provider: ProviderType) -> &'static [&'static str] {
    match provider {
        ProviderType::Anthropic => ANTHROPIC_MODELS,
        ProviderType::OpenAI => OPENAI_MODELS,
        ProviderType::Glm => GLM_MODELS,
        ProviderType::Qwen => QWEN_MODELS,
        ProviderType::DeepSeek => DEEPSEEK_MODELS,
        ProviderType::Minimax => MINIMAX_MODELS,
        ProviderType::Ollama => OLLAMA_REFERENCE_MODELS,
    }
}

pub fn default_model(provider: ProviderType) -> &'static str {
    match provider {
        ProviderType::Anthropic => ANTHROPIC_MODELS[0],
        ProviderType::OpenAI => OPENAI_MODELS[0],
        ProviderType::Glm => GLM_MODELS[0],
        ProviderType::Qwen => QWEN_MODELS[0],
        ProviderType::DeepSeek => DEEPSEEK_MODELS[0],
        ProviderType::Minimax => MINIMAX_MODELS[0],
        ProviderType::Ollama => OLLAMA_REFERENCE_MODELS[0],
    }
}

pub fn is_reliable_model(provider: ProviderType, model: &str) -> bool {
    if provider == ProviderType::Ollama {
        return true;
    }

    let normalized = normalize_model_id(model);
    allowed_models(provider)
        .iter()
        .any(|candidate| normalize_model_id(candidate) == normalized)
}

fn normalize_model_id(model: &str) -> String {
    let mut normalized = model.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.starts_with("glm5") {
        normalized = normalized.replacen("glm5", "glm-5", 1);
    }
    if normalized.starts_with("gpt5") {
        normalized = normalized.replacen("gpt5", "gpt-5", 1);
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glm_alias_is_normalized() {
        assert!(is_reliable_model(ProviderType::Glm, "glm5"));
        assert!(is_reliable_model(ProviderType::Glm, "GLM-5"));
    }

    #[test]
    fn test_qwen_allowlist() {
        assert!(is_reliable_model(ProviderType::Qwen, "qwen3-plus"));
        assert!(!is_reliable_model(ProviderType::Qwen, "qwen-plus"));
    }

    #[test]
    fn test_ollama_is_always_reliable() {
        assert!(is_reliable_model(ProviderType::Ollama, "any-custom-model"));
    }
}
