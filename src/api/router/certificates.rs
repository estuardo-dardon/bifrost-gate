use axum::{routing::{delete, get, post, put}, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route("/api/certificates/ca", get(crate::api::controller::certificates::list_ca_certificates_handler))
        .route("/api/certificates/ca", post(crate::api::controller::certificates::create_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", get(crate::api::controller::certificates::get_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", put(crate::api::controller::certificates::update_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", delete(crate::api::controller::certificates::delete_ca_certificate_handler))
        .route("/api/certificates/user", get(crate::api::controller::certificates::list_user_certificates_handler))
        .route("/api/certificates/user", post(crate::api::controller::certificates::create_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", get(crate::api::controller::certificates::get_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", put(crate::api::controller::certificates::update_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", delete(crate::api::controller::certificates::delete_user_certificate_handler))
}
