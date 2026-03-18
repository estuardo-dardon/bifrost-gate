use axum::{extract::{Path, State}, response::IntoResponse, Json};
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
pub async fn list_ca_certificates_handler(State(state): State<crate::AppState>) -> impl IntoResponse {
    crate::api::service::certificates::list_ca_certificates_handler(state).await
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
) -> impl IntoResponse {
    crate::api::service::certificates::certificate_read_handler(state, ca_name, crate::api::types::CertificateKind::Ca).await
}

#[utoipa::path(
    post,
    path = "/api/certificates/ca",
    request_body = CaCertificateCreateRequest,
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
    Json(payload): Json<CaCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    crate::api::service::certificates::certificate_ca_upsert_handler(state, payload.name, params, false).await
}

#[utoipa::path(
    put,
    path = "/api/certificates/ca/{ca_name}",
    request_body = CaCertificateUpsertRequest,
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
    Json(payload): Json<CaCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    crate::api::service::certificates::certificate_ca_upsert_handler(state, ca_name, params, true).await
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
) -> impl IntoResponse {
    crate::api::service::certificates::certificate_delete_handler(state, ca_name, crate::api::types::CertificateKind::Ca).await
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
pub async fn list_user_certificates_handler(State(state): State<crate::AppState>) -> impl IntoResponse {
    crate::api::service::certificates::list_user_certificates_handler(state).await
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
) -> impl IntoResponse {
    crate::api::service::certificates::certificate_read_handler(state, cert_name, crate::api::types::CertificateKind::User).await
}

#[utoipa::path(
    post,
    path = "/api/certificates/user",
    request_body = UserCertificateCreateRequest,
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
    Json(payload): Json<UserCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    crate::api::service::certificates::certificate_user_upsert_handler(state, payload.name, params, false).await
}

#[utoipa::path(
    put,
    path = "/api/certificates/user/{cert_name}",
    request_body = UserCertificateUpsertRequest,
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
    Json(payload): Json<UserCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = crate::api::types::UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    crate::api::service::certificates::certificate_user_upsert_handler(state, cert_name, params, true).await
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
) -> impl IntoResponse {
    crate::api::service::certificates::certificate_delete_handler(state, cert_name, crate::api::types::CertificateKind::User).await
}
