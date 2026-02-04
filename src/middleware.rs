/*
 * Bifröst-Gate: Middleware para logging de API
 * Copyright (C) 2026 Estuardo Dardón.
 */

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct LoggingMiddlewareState {
    pub logger: Arc<crate::logger::Logger>,
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
