/*
 * Bifröst-Gate: Middleware para logging de API
 * Copyright (C) 2026 Estuardo Dardón.
 */

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::Arc;
use std::time::Instant;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct LoggingMiddlewareState {
    pub logger: Arc<crate::logger::Logger>,
}

#[derive(Clone)]
pub struct ApiKeyMiddlewareState {
    pub logger: Arc<crate::logger::Logger>,
    pub enabled: bool,
    pub pool: SqlitePool,
    pub header_name: String,
}

#[derive(Clone)]
pub struct DocsAuthMiddlewareState {
    pub logger: Arc<crate::logger::Logger>,
    pub pool: SqlitePool,
}

/// Middleware que registra todas las requests de API
pub async fn logging_middleware(
    State(state): State<LoggingMiddlewareState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    
    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed();
    
    let status = response.status().as_u16();
    let duration_ms = duration.as_millis();

    state
        .logger
        .log_api_request(&method, &path, status, duration_ms);

    response
}

/// Middleware que valida API key para endpoints protegidos.
pub async fn api_key_middleware(
    State(state): State<ApiKeyMiddlewareState>,
    request: Request,
    next: Next,
) -> Response {
    if !state.enabled {
        return next.run(request).await;
    }

    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let header_name = state.header_name.to_ascii_lowercase();

    let provided_key = request
        .headers()
        .iter()
        .find_map(|(name, value)| {
            if name.as_str().eq_ignore_ascii_case(&header_name) {
                value.to_str().ok()
            } else {
                None
            }
        });

    let provided_key = match provided_key {
        Some(value) => value,
        None => {
            state
                .logger
                .log_api_error(&method, &path, 401, "Missing or invalid API key");

            return (
                StatusCode::UNAUTHORIZED,
                [("content-type", "application/json")],
                "{\"error\":\"unauthorized\",\"message\":\"Missing or invalid API key\"}",
            )
                .into_response();
        }
    };

    let is_valid = match crate::db::is_valid_api_key(&state.pool, provided_key).await {
        Ok(valid) => valid,
        Err(err) => {
            state
                .logger
                .log_api_error(&method, &path, 500, &format!("API key DB error: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "application/json")],
                "{\"error\":\"internal_error\",\"message\":\"API key validation failed\"}",
            )
                .into_response();
        }
    };

    if !is_valid {
        state
            .logger
            .log_api_error(&method, &path, 401, "Missing or invalid API key");

        return (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            "{\"error\":\"unauthorized\",\"message\":\"Missing or invalid API key\"}",
        )
            .into_response();
    }

    next.run(request).await
}

/// Middleware de autenticación Basic para rutas de documentación.
pub async fn docs_basic_auth_middleware(
    State(state): State<DocsAuthMiddlewareState>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let unauthorized = || {
        let mut resp = (
            StatusCode::UNAUTHORIZED,
            [
                ("content-type", "application/json"),
            ],
            "{\"error\":\"unauthorized\",\"message\":\"Basic auth required\"}",
        )
            .into_response();
        resp.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Bifrost Docs\""),
        );
        resp
    };

    let Some((scheme, encoded)) = auth_header.split_once(' ') else {
        state
            .logger
            .log_api_error(&method, &path, 401, "Missing basic auth scheme");
        return unauthorized();
    };

    if !scheme.eq_ignore_ascii_case("Basic") {
        state
            .logger
            .log_api_error(&method, &path, 401, "Invalid auth scheme for docs");
        return unauthorized();
    }

    let encoded = encoded.trim();
    if encoded.is_empty() {
        state
            .logger
            .log_api_error(&method, &path, 401, "Empty basic auth payload");
        return unauthorized();
    }

    let decoded = match STANDARD.decode(encoded) {
        Ok(bytes) => bytes,
        Err(_) => {
            state
                .logger
                .log_api_error(&method, &path, 401, "Invalid basic auth encoding");
            return unauthorized();
        }
    };

    let credentials = String::from_utf8_lossy(&decoded);
    let Some((username, password)) = credentials.split_once(':') else {
        state
            .logger
            .log_api_error(&method, &path, 401, "Invalid basic auth payload");
        return unauthorized();
    };

    let is_valid = match crate::db::verify_docs_user_credentials(&state.pool, username, password).await {
        Ok(valid) => valid,
        Err(err) => {
            state
                .logger
                .log_api_error(&method, &path, 500, &format!("Docs auth DB error: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "application/json")],
                "{\"error\":\"internal_error\",\"message\":\"Docs auth failed\"}",
            )
                .into_response();
        }
    };

    if !is_valid {
        state
            .logger
            .log_api_error(&method, &path, 401, "Invalid docs credentials");
        return unauthorized();
    }

    next.run(request).await
}
