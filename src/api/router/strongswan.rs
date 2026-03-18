use axum::{routing::post, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route("/api/strongswan/start", post(crate::api::controller::strongswan::strongswan_start_handler))
        .route("/api/strongswan/stop", post(crate::api::controller::strongswan::strongswan_stop_handler))
}
