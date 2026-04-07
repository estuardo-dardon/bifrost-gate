use axum::http::HeaderMap;
use sqlx::SqlitePool;

pub const CODE_OK: i64 = 20000;
pub const CODE_API_KEY_REQUIRED: i64 = 30000;
pub const CODE_API_KEY_INVALID: i64 = 30001;
pub const CODE_API_KEY_DB_ERROR: i64 = 30002;
pub const CODE_FORBIDDEN: i64 = 40300;
pub const CODE_PEER_NAME_REQUIRED: i64 = 40000;
pub const CODE_PEER_PHASE_INVALID: i64 = 40001;
pub const CODE_PEER_IKE_FAILED: i64 = 41010;
pub const CODE_PEER_CHILD_FAILED: i64 = 41011;
pub const CODE_INVALID_INPUT: i64 = 40010;
pub const CODE_NOT_FOUND: i64 = 40400;
#[allow(dead_code)]
pub const CODE_OPERATION_FAILED: i64 = 40900;
pub const CODE_INTERNAL_ERROR: i64 = 50000;
#[allow(dead_code)]
pub const CODE_NOT_SUPPORTED: i64 = 50100;

pub fn resolve_requested_language(headers: &HeaderMap) -> String {
    if let Some(value) = headers.get("x-lang").and_then(|v| v.to_str().ok()) {
        return normalize_language_tag(value);
    }

    if let Some(value) = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
    {
        return normalize_language_tag(value);
    }

    "en".to_string()
}

pub fn normalize_language_tag(value: &str) -> String {
    let first = value
        .split(',')
        .next()
        .unwrap_or("en")
        .split(';')
        .next()
        .unwrap_or("en")
        .split('-')
        .next()
        .unwrap_or("en")
        .trim()
        .to_ascii_lowercase();

    if first.is_empty() {
        "en".to_string()
    } else {
        first
    }
}

pub async fn message_for_code(pool: &SqlitePool, code: i64, requested_lang: Option<&str>) -> String {
    let lang = requested_lang
        .map(normalize_language_tag)
        .unwrap_or_else(|| "en".to_string());

    match crate::db::get_localized_response_message(pool, code, &lang).await {
        Ok(Some(message)) => message,
        Ok(None) => "General Error".to_string(),
        Err(_) => "General Error".to_string(),
    }
}
