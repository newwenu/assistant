use crate::base::ProviderError;

pub(crate) fn parse_retry_after_secs(text: &str) -> Option<u64> {
    let lower = text.to_lowercase();
    for prefix in &["retry after ", "retry_after:", "retry-after:"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim();
            if let Ok(secs) = rest.parse::<u64>() {
                return Some(secs);
            }
        }
    }
    if let Some(pos) = lower.find("retry after ") {
        let rest = lower[pos + 12..].trim();
        if let Ok(secs) = rest.parse::<u64>() {
            return Some(secs);
        }
    }
    None
}

pub(crate) fn extract_error_detail(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(error) = value.get("error") {
            if let Some(message) = error.get("message").and_then(|v| v.as_str()) {
                if !message.is_empty() {
                    return message.to_string();
                }
            }
            if let Some(s) = error.as_str() {
                return s.to_string();
            }
            let error_type = error.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let code = error.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let combined = format!("{} {}", error_type, code).trim().to_string();
            if !combined.is_empty() {
                return combined;
            }
        }
    }
    body.chars().take(300).collect()
}

#[allow(dead_code)]
pub(crate) struct ClassifiedError {
    pub error: ProviderError,
    pub http_status: u16,
    pub provider_code: Option<String>,
}

fn extract_provider_code(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_str())
                .map(String::from)
        })
}

pub(crate) fn classify_http_error(status: reqwest::StatusCode, body: &str) -> ClassifiedError {
    let retry_after_ms = parse_retry_after_secs(body).map(|s| s * 1000);
    let error_detail = extract_error_detail(body);
    let provider_code = extract_provider_code(body);
    let status_code = status.as_u16();
    let error = match status_code {
        401 | 403 => ProviderError::AuthFailed(error_detail),
        429 => ProviderError::RateLimited { retry_after_ms },
        404 => ProviderError::ModelNotFound(error_detail),
        400 => {
            let lower = error_detail.to_lowercase();
            if lower.contains("context") || lower.contains("token") && lower.contains("max") {
                ProviderError::ContextOverflow(error_detail)
            } else {
                ProviderError::RequestRejected(error_detail)
            }
        }
        408 | 504 => ProviderError::Timeout(error_detail),
        _ if status.is_server_error() => ProviderError::ServerError(error_detail),
        _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status_code, error_detail)),
    };
    ClassifiedError {
        error,
        http_status: status_code,
        provider_code,
    }
}

pub(crate) fn classify_request_error(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout(format!("HTTP request: {}", e))
    } else if e.is_connect() {
        ProviderError::NetworkError(format!("Connection: {}", e))
    } else {
        ProviderError::NetworkError(format!("HTTP request: {}", e))
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn parse_retry_after() {
        assert_eq!(parse_retry_after_secs("retry after 5"), Some(5));
        assert_eq!(parse_retry_after_secs("retry_after: 10"), Some(10));
        assert_eq!(parse_retry_after_secs("retry-after: 20"), Some(20));
        assert_eq!(parse_retry_after_secs("Retry After 3"), Some(3));
        assert_eq!(parse_retry_after_secs("no retry info"), None);
    }

    #[test]
    fn classify_http_status() {
        let classified =
            classify_http_error(reqwest::StatusCode::from_u16(401).unwrap(), "unauthorized");
        assert!(matches!(classified.error, ProviderError::AuthFailed(_)));
        assert_eq!(classified.http_status, 401);

        let classified =
            classify_http_error(reqwest::StatusCode::from_u16(429).unwrap(), "retry after 5");
        assert!(matches!(
            classified.error,
            ProviderError::RateLimited {
                retry_after_ms: Some(5000)
            }
        ));
        assert_eq!(classified.http_status, 429);

        let classified = classify_http_error(
            reqwest::StatusCode::from_u16(400).unwrap(),
            "context length exceeded max tokens",
        );
        assert!(matches!(
            classified.error,
            ProviderError::ContextOverflow(_)
        ));

        let classified = classify_http_error(
            reqwest::StatusCode::from_u16(500).unwrap(),
            "internal error",
        );
        assert!(matches!(classified.error, ProviderError::ServerError(_)));
    }

    #[test]
    fn extract_error_detail_formats() {
        let openai = r#"{"error":{"type":"invalid_request_error","code":"context_length_exceeded","message":"This model's maximum context length is 128000 tokens."}}"#;
        assert!(extract_error_detail(openai).contains("maximum context length"));

        let gemini = r#"{"error":{"message":"API key expired","code":401}}"#;
        assert_eq!(extract_error_detail(gemini), "API key expired");

        assert_eq!(extract_error_detail("not json"), "not json");
    }
}
