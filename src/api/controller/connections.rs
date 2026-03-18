use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use crate::api::types::*;

#[utoipa::path(
    get,
    path = "/api/connections",
    responses(
        (status = 200, description = "Listado de conexiones", body = ConnectionListResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn list_connections_handler(State(state): State<crate::AppState>) -> impl IntoResponse {
    crate::api::service::connections::list_connections_handler(state).await
}

#[utoipa::path(
    get,
    path = "/api/connections/{connection_name}",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion encontrada", body = ConnectionResponse),
        (status = 400, description = "Nombre invalido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn get_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
) -> impl IntoResponse {
    crate::api::service::connections::connection_read_handler(state, connection_name).await
}

#[utoipa::path(
    post,
    path = "/api/connections",
    request_body = ConnectionCreateRequest,
    responses(
        (status = 201, description = "Conexion creada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 409, description = "Conexion ya existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn create_connection_handler(
    State(state): State<crate::AppState>,
    Json(payload): Json<ConnectionCreateRequest>,
) -> impl IntoResponse {
    crate::api::service::connections::connection_upsert_handler(state, payload.name, payload.config, false).await
}

#[utoipa::path(
    put,
    path = "/api/connections/{connection_name}",
    request_body = ConnectionUpsertRequest,
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion actualizada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn update_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    Json(payload): Json<ConnectionUpsertRequest>,
) -> impl IntoResponse {
    crate::api::service::connections::connection_upsert_handler(state, connection_name, payload.config, true).await
}

#[utoipa::path(
    delete,
    path = "/api/connections/{connection_name}",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion eliminada", body = ConnectionCrudResponse),
        (status = 400, description = "Nombre invalido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn delete_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
) -> impl IntoResponse {
    crate::api::service::connections::connection_delete_handler(state, connection_name).await
}

#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/certificate",
    request_body = ConnectionCertificateAttachRequest,
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Certificado adjuntado", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion o certificado no encontrado", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn attach_certificate_to_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    Json(payload): Json<ConnectionCertificateAttachRequest>,
) -> impl IntoResponse {
    crate::api::service::connections::attach_certificate_to_connection_handler(state, connection_name, payload).await
}
