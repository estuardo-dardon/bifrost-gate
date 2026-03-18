use axum::{routing::get, Router};

pub fn routes() -> Router<crate::AppState> {
    Router::new().route("/api/topology", get(crate::api::controller::topology::get_topology_handler))
}
