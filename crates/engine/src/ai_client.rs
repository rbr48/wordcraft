//! Multi-provider AI client for WordCraft Copilot.
//!
//! Connects to OpenRouter, DeepSeek, Google Gemini, Anthropic Claude,
//! and OpenAI-compatible endpoints with bounded timeouts and zero panics.

/// Queries an external LLM API provider synchronously.
#[cfg(not(target_arch = "wasm32"))]
pub fn query_llm_api(
    provider: &str,
    model: &str,
    api_key: &str,
    base_url: Option<&str>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("API key is empty".into());
    }

    let timeout = std::time::Duration::from_secs(60);
    let provider_lower = provider.trim().to_lowercase();

    if provider_lower == "claude" || provider_lower == "anthropic" {
        // Anthropic Messages API
        let url = base_url.unwrap_or("https://api.anthropic.com/v1/messages");
        let model_id = if model.trim().is_empty() { "claude-3-5-sonnet-20241022" } else { model.trim() };
        let body = serde_json::json!({
            "model": model_id,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_prompt}
            ]
        });

        let resp = ureq::post(url)
            .timeout(timeout)
            .set("x-api-key", api_key.trim())
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| format!("Anthropic API error: {e}"))?;

        let json: serde_json::Value = resp.into_json().map_err(|e| format!("Failed to parse JSON response: {e}"))?;
        if let Some(text) =
            json.get("content").and_then(|c| c.as_array()).and_then(|arr| arr.first()).and_then(|item| item.get("text")).and_then(|t| t.as_str())
        {
            return Ok(text.to_string());
        }
        return Err(format!("Unexpected response format from Anthropic: {json}"));
    }

    // OpenAI-compatible format (OpenRouter, DeepSeek, Gemini, Custom)
    let (url, default_model) = match provider_lower.as_str() {
        "deepseek" => ("https://api.deepseek.com/chat/completions", "deepseek-chat"),
        "gemini" | "google" => ("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", "gemini-2.0-flash"),
        "openrouter" => ("https://openrouter.ai/api/v1/chat/completions", "deepseek/deepseek-chat"),
        _ => (base_url.unwrap_or("https://openrouter.ai/api/v1/chat/completions"), "deepseek/deepseek-chat"),
    };

    let target_url = if let Some(u) = base_url.filter(|s| !s.trim().is_empty()) { u } else { url };
    let model_id = if model.trim().is_empty() { default_model } else { model.trim() };

    let body = serde_json::json!({
        "model": model_id,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.7
    });

    let mut req =
        ureq::post(target_url).timeout(timeout).set("Authorization", &format!("Bearer {}", api_key.trim())).set("Content-Type", "application/json");

    if provider_lower == "openrouter" || target_url.contains("openrouter.ai") {
        req = req.set("HTTP-Referer", "https://github.com/rbr48/wordcraft").set("X-Title", "WordCraft Word Processor");
    }

    let resp = req.send_json(body).map_err(|e| format!("{provider} API error: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| format!("Failed to parse JSON response: {e}"))?;

    if let Some(err) = json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
        return Err(format!("API returned error: {err}"));
    }

    if let Some(content) = json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|cnt| cnt.as_str())
    {
        return Ok(content.to_string());
    }

    Err(format!("Unexpected response structure from {provider}: {json}"))
}

/// Fallback for wasm32 target.
#[cfg(target_arch = "wasm32")]
pub fn query_llm_api(
    _provider: &str,
    _model: &str,
    _api_key: &str,
    _base_url: Option<&str>,
    _system_prompt: &str,
    _user_prompt: &str,
) -> Result<String, String> {
    Err("Direct HTTP API calls are not supported on wasm32; use offline mode.".into())
}
