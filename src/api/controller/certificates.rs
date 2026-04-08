use axum::{extract::{Path, State}, http::HeaderMap, response::IntoResponse, Json};
#[allow(unused_imports)]
use serde_json::json;
use crate::api::types::*;

#[utoipa::path(
    get,
    path = "/api/certificates/ca",
    responses(
        (status = 200, description = "Listado de CA", body = CertificateListResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn list_ca_certificates_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::list_ca_certificates_handler(state, Some(lang)).await
}

#[utoipa::path(
    get,
    path = "/api/certificates/ca/{ca_name}",
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA encontrada", body = CertificateDetailsResponse),
        (status = 400, description = "Nombre invalido", body = CertificateCrudResponse),
        (status = 404, description = "CA no encontrada", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn get_ca_certificate_handler(
    State(state): State<crate::AppState>,
    Path(ca_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_read_handler(state, ca_name, crate::api::types::CertificateKind::Ca, Some(lang)).await
}

#[utoipa::path(
    post,
    path = "/api/certificates/ca",
    request_body(
        content = CaCertificateCreateRequest,
        examples(
            (
                "create_ca_for_remote_access" = (
                    summary = "Crear CA para túneles de acceso remoto",
                    description = "Crea la CA que firmará los certificados de tus usuarios para túneles peer-to-any por certificado.",
                    value = json!({
                        "name": "corp-ca",
                        "common_name": "Corp VPN Root CA",
                        "organization": "Bifrost Corp",
                        "country": "GT",
                        "days": 3650,
                        "key_size": 4096
                    })
                )
            )
        )
    ),
    responses(
        (status = 201, description = "CA creada", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud invalida", body = CertificateCrudResponse),
        (status = 409, description = "CA ya existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn create_ca_certificate_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<CaCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_ca_upsert_handler(state, payload.name, params, false, Some(lang)).await
}

#[utoipa::path(
    put,
    path = "/api/certificates/ca/{ca_name}",
    request_body(
        content = CaCertificateUpsertRequest,
        examples(
            (
                "update_ca_for_remote_access" = (
                    summary = "Actualizar parámetros de la CA",
                    description = "Actualiza metadatos/validez de la CA que se usa para emitir certificados de clientes.",
                    value = json!({
                        "common_name": "Corp VPN Root CA",
                        "organization": "Bifrost Corp",
                        "country": "GT",
                        "days": 3650,
                        "key_size": 4096
                    })
                )
            )
        )
    ),
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA actualizada", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud invalida", body = CertificateCrudResponse),
        (status = 404, description = "CA no existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn update_ca_certificate_handler(
    State(state): State<crate::AppState>,
    Path(ca_name): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<CaCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_ca_upsert_handler(state, ca_name, params, true, Some(lang)).await
}

#[utoipa::path(
    delete,
    path = "/api/certificates/ca/{ca_name}",
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA eliminada", body = CertificateCrudResponse),
        (status = 400, description = "Nombre invalido", body = CertificateCrudResponse),
        (status = 404, description = "CA no encontrada", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn delete_ca_certificate_handler(
    State(state): State<crate::AppState>,
    Path(ca_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_delete_handler(state, ca_name, crate::api::types::CertificateKind::Ca, Some(lang)).await
}

#[utoipa::path(
    get,
    path = "/api/certificates/user",
    responses(
        (status = 200, description = "Listado de certificados de usuario", body = CertificateListResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn list_user_certificates_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::list_user_certificates_handler(state, Some(lang)).await
}

#[utoipa::path(
    get,
    path = "/api/certificates/user/{cert_name}",
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado encontrado", body = CertificateDetailsResponse),
        (status = 400, description = "Nombre invalido", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no encontrado", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn get_user_certificate_handler(
    State(state): State<crate::AppState>,
    Path(cert_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_read_handler(state, cert_name, crate::api::types::CertificateKind::User, Some(lang)).await
}

#[utoipa::path(
    post,
    path = "/api/certificates/user",
    request_body(
        content = UserCertificateCreateRequest,
        examples(
            (
                "create_user_certificate_alice" = (
                    summary = "Crear certificado de usuario (alice)",
                    description = "Certificado cliente para un túnel peer-to-any por certificados. Debe estar firmado por la misma CA referenciada en remote.cacerts de la conexión.",
                    value = json!({
                        "name": "alice-cert",
                        "ca_name": "corp-ca",
                        "identity": "alice",
                        "san": ["alice", "alice@corp.example"],
                        "days": 825,
                        "key_size": 4096
                    })
                )
            ),
            (
                "create_user_certificate_bob" = (
                    summary = "Crear certificado de usuario (bob)",
                    description = "Ejemplo adicional para múltiples usuarios en el mismo túnel (cada usuario con su propio certificado).",
                    value = json!({
                        "name": "bob-cert",
                        "ca_name": "corp-ca",
                        "identity": "bob",
                        "san": ["bob", "bob@corp.example"],
                        "days": 825,
                        "key_size": 4096
                    })
                )
            )
        )
    ),
    responses(
        (status = 201, description = "Certificado creado", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud invalida", body = CertificateCrudResponse),
        (status = 409, description = "Certificado ya existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn create_user_certificate_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<UserCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_user_upsert_handler(state, payload.name, params, false, Some(lang)).await
}

#[utoipa::path(
    put,
    path = "/api/certificates/user/{cert_name}",
    request_body(
        content = UserCertificateUpsertRequest,
        examples(
            (
                "update_user_certificate" = (
                    summary = "Regenerar/actualizar certificado de usuario",
                    description = "Útil para renovar certificado de usuario sin cambiar la identidad lógica usada por el túnel.",
                    value = json!({
                        "ca_name": "corp-ca",
                        "identity": "alice",
                        "san": ["alice", "alice@corp.example"],
                        "days": 825,
                        "key_size": 4096
                    })
                )
            )
        )
    ),
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado actualizado", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud invalida", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn update_user_certificate_handler(
    State(state): State<crate::AppState>,
    Path(cert_name): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<UserCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_user_upsert_handler(state, cert_name, params, true, Some(lang)).await
}

#[utoipa::path(
    delete,
    path = "/api/certificates/user/{cert_name}",
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado eliminado", body = CertificateCrudResponse),
        (status = 400, description = "Nombre invalido", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no encontrado", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operacion no soportada", body = CertificateCrudResponse)
    )
)]
pub async fn delete_user_certificate_handler(
    State(state): State<crate::AppState>,
    Path(cert_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::certificates::certificate_delete_handler(state, cert_name, crate::api::types::CertificateKind::User, Some(lang)).await
}
