//! Default model IDs loaded from `default_models.json` at runtime.
//! Edit that JSON file to change them.
//!
//! At runtime each model is resolved from the first of these that is set: CLI flag, ENV var, config.toml, remote settings, these defaults.

use std::sync::LazyLock;

/// The raw JSON, embedded at compile time.
/// It is `pub` because `xai_grok_shell::models` re-exports it and `agent::config` reads it.
pub const DEFAULT_MODELS_JSON: &str = include_str!("../default_models.json");

#[derive(serde::Deserialize)]
struct DefaultModels {
    default: String,
    /// Falls back to `default` if not specified in JSON.
    web_search: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    image_description: Option<String>,
    /// Falls back to `default` if not specified in JSON.
    session_summary: Option<String>,
    models: Vec<DefaultModelEntry>,
}

#[derive(serde::Deserialize)]
struct DefaultModelEntry {
    model: String,
}

static DEFAULTS: LazyLock<DefaultModels> = LazyLock::new(|| {
    let defaults: DefaultModels = serde_json::from_str(DEFAULT_MODELS_JSON)
        .expect("default_models.json: invalid JSON or missing 'default' field");

    // Baked-in JSON: a mismatch here is a developer error
    let model_ids: Vec<&str> = defaults.models.iter().map(|m| m.model.as_str()).collect();
    assert!(
        model_ids.contains(&defaults.default.as_str()),
        "default_models.json: 'default' is '{}' but 'models' array only has {model_ids:?}",
        defaults.default,
    );

    defaults
});

/// Primary model for coding tasks and general fallback.
pub fn default_model() -> &'static str {
    &DEFAULTS.default
}

/// Model for web search tool synthesis. Falls back to default model.
pub fn default_web_search_model() -> &'static str {
    DEFAULTS.web_search.as_deref().unwrap_or(&DEFAULTS.default)
}

/// Model for image describe. Falls back to default model.
pub fn default_image_description_model() -> &'static str {
    DEFAULTS
        .image_description
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

/// Model for session title generation. Falls back to default model.
pub fn default_session_summary_model() -> &'static str {
    DEFAULTS
        .session_summary
        .as_deref()
        .unwrap_or(&DEFAULTS.default)
}

/// Whether a given model ID or display name represents a built-in Grok/SpaceXAI model.
pub fn is_builtin_model(id_or_name: &str) -> bool {
    let trimmed = id_or_name.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("grok") {
        return true;
    }
    let first_token = lower.split_whitespace().next().unwrap_or(&lower);
    if first_token.starts_with("grok") {
        return true;
    }
    DEFAULTS
        .models
        .iter()
        .any(|m| m.model.eq_ignore_ascii_case(trimmed) || m.model.eq_ignore_ascii_case(first_token))
}

/// Whether a given model ID or display name represents a custom (third-party or user-configured) model.
pub fn is_custom_model(id_or_name: &str) -> bool {
    !is_builtin_model(id_or_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_and_custom_models() {
        assert!(is_builtin_model("grok-4.6"));
        assert!(is_builtin_model("grok-4.5"));
        assert!(is_builtin_model("grok-3"));
        assert!(is_builtin_model("Grok 4.6"));
        assert!(is_builtin_model("grok-4.6 (high)"));
        assert!(!is_custom_model("grok-4.6"));

        assert!(!is_builtin_model("claude-3-7-sonnet"));
        assert!(is_custom_model("claude-3-7-sonnet"));
        assert!(is_custom_model("deepseek-chat"));
        assert!(is_custom_model("gpt-4o"));
        assert!(is_custom_model("my-custom-model"));
        assert!(is_custom_model(""));
    }
}
