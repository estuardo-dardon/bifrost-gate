use axum::{extract::State, http::HeaderMap, response::IntoResponse};

/// Inicia el servicio de StrongSwan en el host.
#[utoipa::path(
    post,
    path = "/api/strongswan/start",
    responses(
        (status = 200, description = "Servicio StrongSwan iniciado", body = crate::api::types::ServiceControlResponse),
        (status = 400, description = "No se pudo iniciar", body = crate::api::types::ServiceControlResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ServiceControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = crate::api::types::ServiceControlResponse)
    )
)]
pub async fn strongswan_start_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::strongswan::strongswan_control_handler(state, "start", Some(lang)).await
}

/// Detiene el servicio de StrongSwan en el host.
#[utoipa::path(
    post,
    path = "/api/strongswan/stop",
    responses(
        (status = 200, description = "Servicio StrongSwan detenido", body = crate::api::types::ServiceControlResponse),
        (status = 400, description = "No se pudo detener", body = crate::api::types::ServiceControlResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ServiceControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = crate::api::types::ServiceControlResponse)
    )
)]
pub async fn strongswan_stop_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::strongswan::strongswan_control_handler(state, "stop", Some(lang)).await
}

/// Reinicia el servicio de StrongSwan en el host.
#[utoipa::path(
    post,
    path = "/api/strongswan/restart",
    responses(
        (status = 200, description = "Servicio StrongSwan reiniciado", body = crate::api::types::ServiceControlResponse),
        (status = 400, description = "No se pudo reiniciar", body = crate::api::types::ServiceControlResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ServiceControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = crate::api::types::ServiceControlResponse)
    )
)]
pub async fn strongswan_restart_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::strongswan::strongswan_control_handler(state, "restart", Some(lang)).await
}
