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
    let cache_key = "bifrost:topology";

    if let Some(cached_topology) = state.cache.get::<BifrostTopology>(cache_key).await {
        state.metrics.topology_requests.inc();
        return Json(cached_topology);
    }

    let topo = crate::api::service::topology::get_topology(state.clone());
    state.cache.set(cache_key, &topo.0, Some(60)).await;

    topo
}
