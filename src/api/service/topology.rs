use axum::Json;

pub fn get_topology(state: crate::AppState) -> Json<crate::models::BifrostTopology> {
    let topo = state.topology.read().unwrap();

    state.metrics.topology_requests.inc();
    state.metrics.topology_nodes_count.set(topo.nodes.len() as f64);
    state.metrics.topology_edges_count.set(topo.edges.len() as f64);

    Json(topo.clone())
}
