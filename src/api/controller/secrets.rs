use axum::{extract::{Path, State}, http::HeaderMap, response::IntoResponse, Json};
#[allow(unused_imports)]
use serde_json::json;
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
pub async fn list_secrets_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::secrets::list_secrets_handler(state, Some(lang)).await
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
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::secrets::secret_read_handler(state, secret_name, Some(lang)).await
}

#[utoipa::path(
    post,
    path = "/api/secrets",
    request_body(
        content = SecretCreateRequest,
        examples(
            (
                "ike_psk_peer_to_peer" = (
                    summary = "Secret IKE (PSK) para peer-to-peer",
                    description = "Flujo end-to-end: 1) Crea este secret IKE. 2) Crea o actualiza la conexión en /api/connections con local.id=1.1.1.1 y remote.id=2.2.2.2. 3) Si cambias PSK, rota con PUT /api/secrets/{secret_name}. config.ids debe coincidir con los IDs de la conexión.",
                    value = json!({
                        "name": "peer-to-peer-psk",
                        "secret_type": "ike",
                        "config": {
                            "secret": "SuperSecretPSK123!",
                            "ids": ["1.1.1.1", "2.2.2.2"]
                        }
                    })
                )
            ),
            (
                "eap_user_remote_access" = (
                    summary = "Secret EAP para usuario remoto",
                    description = "Flujo end-to-end: 1) Crea un secret EAP por usuario. 2) Configura conexión peer-to-any con remote.auth=eap-mschapv2 y eap_id adecuado. 3) Para cambio de contraseña, usa PUT /api/secrets/{secret_name}.",
                    value = json!({
                        "name": "rw-alice",
                        "secret_type": "eap",
                        "config": {
                            "secret": "AlicePassword#2026",
                            "id": "alice"
                        }
                    })
                )
            ),
            (
                "ike_psk_single_id" = (
                    summary = "Secret IKE con un solo ID",
                    description = "Útil para asociar PSK a una sola identidad. El valor de config.id debe coincidir con local.id o remote.id en /api/connections.",
                    value = json!({
                        "name": "branch-a-psk",
                        "secret_type": "ike",
                        "config": {
                            "secret": "AnotherStrongPSK!",
                            "id": "branch-a.example.com"
                        }
                    })
                )
            ),
            (
                "eap_for_cert_plus_password" = (
                    summary = "Secret EAP para túnel certificado + usuario/clave",
                    description = "Para el escenario de autenticación múltiple (remote.auth=pubkey + remote.auth2=eap-mschapv2). Crea un secret EAP por usuario para el segundo factor.",
                    value = json!({
                        "name": "alice-eap-2fa",
                        "secret_type": "eap",
                        "config": {
                            "secret": "Alice2FA-Password#2026",
                            "id": "alice"
                        }
                    })
                )
            )
        )
    ),
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
    headers: HeaderMap,
    Json(payload): Json<SecretCreateRequest>,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::secrets::secret_upsert_handler(
        state,
        payload.name,
        payload.secret_type,
        payload.config,
        false,
        Some(lang),
    )
    .await
}

#[utoipa::path(
    put,
    path = "/api/secrets/{secret_name}",
    request_body(
        content = SecretUpsertRequest,
        examples(
            (
                "rotate_ike_psk" = (
                    summary = "Rotar PSK IKE de una conexión existente",
                    description = "Flujo end-to-end de rotación: 1) Ejecuta este PUT con el nuevo secret. 2) Mantén iguales local.id/remote.id en la conexión. 3) Verifica con GET /api/connections/{connection_name} y estado del túnel.",
                    value = json!({
                        "secret_type": "ike",
                        "config": {
                            "secret": "NewPSK-Rotated-2026!",
                            "ids": ["1.1.1.1", "2.2.2.2"]
                        }
                    })
                )
            ),
            (
                "rotate_eap_user_password" = (
                    summary = "Actualizar password EAP de un usuario",
                    description = "Flujo end-to-end de usuario EAP: 1) Actualiza este secret. 2) El usuario vuelve a autenticar con la nueva clave. 3) config.id debe seguir siendo la identidad remota del cliente.",
                    value = json!({
                        "secret_type": "eap",
                        "config": {
                            "secret": "AliceNewPassword#2026",
                            "id": "alice"
                        }
                    })
                )
            )
        )
    ),
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
    headers: HeaderMap,
    Json(payload): Json<SecretUpsertRequest>,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::secrets::secret_upsert_handler(state, secret_name, payload.secret_type, payload.config, true, Some(lang)).await
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
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::secrets::secret_delete_handler(state, secret_name, Some(lang)).await
}
