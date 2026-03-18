use axum::{extract::State, Json};

/// Obtiene la topología actual de Bifröst
#[utoipa::path(
    get,
    path = "/api/topology",
    responses(
        (status = 200, description = "Topología obtenida exitosamente", body = crate::models::BifrostTopology)
    )
)]
pub async fn get_topology_handler(
    State(state): State<crate::AppState>,
) -> Json<crate::models::BifrostTopology> {
    crate::api::service::topology::get_topology(state)
}
