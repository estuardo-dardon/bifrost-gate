use axum::{routing::{delete, get, post, put}, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route("/api/connections", get(crate::api::controller::connections::list_connections_handler))
        .route("/api/connections", post(crate::api::controller::connections::create_connection_handler))
        .route("/api/connections/:connection_name", get(crate::api::controller::connections::get_connection_handler))
        .route("/api/connections/:connection_name", put(crate::api::controller::connections::update_connection_handler))
        .route("/api/connections/:connection_name", delete(crate::api::controller::connections::delete_connection_handler))
        .route("/api/connections/:connection_name/enable", post(crate::api::controller::connections::enable_connection_handler))
        .route("/api/connections/:connection_name/disable", post(crate::api::controller::connections::disable_connection_handler))
        .route(
            "/api/connections/:connection_name/certificate",
            post(crate::api::controller::connections::attach_certificate_to_connection_handler),
        )
}
