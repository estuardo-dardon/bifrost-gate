use axum::{routing::{delete, get, post, put}, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route("/api/secrets", get(crate::api::controller::secrets::list_secrets_handler))
        .route("/api/secrets", post(crate::api::controller::secrets::create_secret_handler))
        .route("/api/secrets/:secret_name", get(crate::api::controller::secrets::get_secret_handler))
        .route("/api/secrets/:secret_name", put(crate::api::controller::secrets::update_secret_handler))
        .route("/api/secrets/:secret_name", delete(crate::api::controller::secrets::delete_secret_handler))
}
