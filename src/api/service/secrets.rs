use serde_json::Value;

use crate::api::types::SecretType;

pub async fn list_secrets_handler(state: crate::AppState) -> impl axum::response::IntoResponse {
    crate::api::service::connections::list_secrets_handler(state).await
}

pub async fn secret_read_handler(
    state: crate::AppState,
    secret_name: String,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::secret_read_handler(state, secret_name).await
}

pub async fn secret_upsert_handler(
    state: crate::AppState,
    secret_name: String,
    secret_type: SecretType,
    config: Value,
    update: bool,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::secret_upsert_handler(
        state,
        secret_name,
        secret_type,
        config,
        update,
    )
    .await
}

pub async fn secret_delete_handler(
    state: crate::AppState,
    secret_name: String,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::secret_delete_handler(state, secret_name).await
}
