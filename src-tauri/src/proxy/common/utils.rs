// ToolFunction

pub fn generate_random_id() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

/// according toModelNameSpeculateFunctionType
// Notice：此FunctionDeprecated，Please use instead mappers::common_utils::resolve_request_config
pub fn _deprecated_infer_quota_group(model: &str) -> String {
    if model.to_lowercase().starts_with("claude") {
        "claude".to_string()
    } else {
        "gemini".to_string()
    }
}
