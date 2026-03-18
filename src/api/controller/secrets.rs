use axum::{extract::{Path, State}, response::IntoResponse, Json};
use crate::api::types::*;

#[utoipa::path(
    get,
    path = "/api/secrets",
    responses(
        (status = 200, description = "Listado de secrets", body = SecretListResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operacion no soportada", body = SecretCrudResponse)
    )
)]
pub async fn list_secrets_handler(State(state): State<crate::AppState>) -> impl IntoResponse {
    crate::api::service::secrets::list_secrets_handler(state).await
}

#[utoipa::path(
    get,
    path = "/api/secrets/{secret_name}",
    params(("secret_name" = String, Path, description = "Nombre del secret")),
    responses(
        (status = 200, description = "Secret encontrado", body = SecretResponse),
        (status = 400, description = "Nombre invalido", body = SecretCrudResponse),
        (status = 404, description = "Secret no encontrado", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operacion no soportada", body = SecretCrudResponse)
    )
)]
pub async fn get_secret_handler(
    State(state): State<crate::AppState>,
    Path(secret_name): Path<String>,
) -> impl IntoResponse {
    crate::api::service::secrets::secret_read_handler(state, secret_name).await
}

#[utoipa::path(
    post,
    path = "/api/secrets",
    request_body = SecretCreateRequest,
    responses(
        (status = 201, description = "Secret creado", body = SecretCrudResponse),
        (status = 400, description = "Solicitud invalida", body = SecretCrudResponse),
        (status = 409, description = "Secret ya existe", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operacion no soportada", body = SecretCrudResponse)
    )
)]
pub async fn create_secret_handler(
    State(state): State<crate::AppState>,
    Json(payload): Json<SecretCreateRequest>,
) -> impl IntoResponse {
    crate::api::service::secrets::secret_upsert_handler(
        state,
        payload.name,
        payload.secret_type,
        payload.config,
        false,
    )
    .await
}

#[utoipa::path(
    put,
    path = "/api/secrets/{secret_name}",
    request_body = SecretUpsertRequest,
    params(("secret_name" = String, Path, description = "Nombre del secret")),
    responses(
        (status = 200, description = "Secret actualizado", body = SecretCrudResponse),
        (status = 400, description = "Solicitud invalida", body = SecretCrudResponse),
        (status = 404, description = "Secret no existe", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operacion no soportada", body = SecretCrudResponse)
    )
)]
pub async fn update_secret_handler(
    State(state): State<crate::AppState>,
    Path(secret_name): Path<String>,
    Json(payload): Json<SecretUpsertRequest>,
) -> impl IntoResponse {
    crate::api::service::secrets::secret_upsert_handler(state, secret_name, payload.secret_type, payload.config, true).await
}

#[utoipa::path(
    delete,
    path = "/api/secrets/{secret_name}",
    params(("secret_name" = String, Path, description = "Nombre del secret")),
    responses(
        (status = 200, description = "Secret eliminado", body = SecretCrudResponse),
        (status = 400, description = "Nombre invalido", body = SecretCrudResponse),
        (status = 404, description = "Secret no encontrado", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operacion no soportada", body = SecretCrudResponse)
    )
)]
pub async fn delete_secret_handler(
    State(state): State<crate::AppState>,
    Path(secret_name): Path<String>,
) -> impl IntoResponse {
    crate::api::service::secrets::secret_delete_handler(state, secret_name).await
}
