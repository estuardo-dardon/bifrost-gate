use axum::{extract::State, Json};
use crate::models::BifrostTopology;

/// Obtiene la topología actual de Bifröst
#[utoipa::path(
    get,
    path = "/api/topology",
    responses(
        (status = 200, description = "Topología obtenida exitosamente", body = BifrostTopology)
    )
)]
pub async fn get_topology_handler(
    State(state): State<crate::AppState>,
) -> Json<BifrostTopology> {
    crate::api::service::topology::get_topology(state)
}
