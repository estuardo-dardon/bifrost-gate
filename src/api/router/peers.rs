use axum::{routing::{get, post}, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route("/api/peers/:peer_name/up", post(crate::api::controller::peers::peer_up_handler))
        .route("/api/peers/:peer_name/down", post(crate::api::controller::peers::peer_down_handler))
        .route("/api/peers/:peer_name/status", get(crate::api::controller::peers::peer_status_handler))
        .route("/api/peers/status", get(crate::api::controller::peers::list_peers_status_handler))
}
