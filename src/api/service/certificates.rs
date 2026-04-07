use crate::api::types::{CaCertificateParams, CertificateKind, UserCertificateParams};

pub async fn list_ca_certificates_handler(
    state: crate::AppState,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::list_ca_certificates_handler(state, language).await
}

pub async fn list_user_certificates_handler(
    state: crate::AppState,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::list_user_certificates_handler(state, language).await
}

pub async fn certificate_read_handler(
    state: crate::AppState,
    certificate_name: String,
    kind: CertificateKind,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::certificate_read_handler(state, certificate_name, kind, language).await
}

pub async fn certificate_ca_upsert_handler(
    state: crate::AppState,
    certificate_name: String,
    params: CaCertificateParams,
    update: bool,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    let internal = crate::api::service::connections::CaCertificateParams {
        common_name: params.common_name,
        organization: params.organization,
        country: params.country,
        days: params.days,
        key_size: params.key_size,
    };
    crate::api::service::connections::certificate_ca_upsert_handler(state, certificate_name, internal, update, language).await
}

pub async fn certificate_user_upsert_handler(
    state: crate::AppState,
    certificate_name: String,
    params: UserCertificateParams,
    update: bool,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    let internal = crate::api::service::connections::UserCertificateParams {
        ca_name: params.ca_name,
        identity: params.identity,
        san: params.san,
        days: params.days,
        key_size: params.key_size,
    };
    crate::api::service::connections::certificate_user_upsert_handler(state, certificate_name, internal, update, language).await
}

pub async fn certificate_delete_handler(
    state: crate::AppState,
    certificate_name: String,
    kind: CertificateKind,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    crate::api::service::connections::certificate_delete_handler(state, certificate_name, kind, language).await
}
