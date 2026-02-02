/*
 * Bifröst-Gate: Módulo de Métricas Prometheus
 * Copyright (C) 2026 Estuardo Dardón.
 */

use prometheus::{Gauge, Registry, TextEncoder, Encoder, IntCounter};
use std::sync::Arc;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    pub topology_requests: IntCounter,
    pub topology_nodes_count: Gauge,
    pub topology_edges_count: Gauge,
    pub errors_total: IntCounter,
}

impl Metrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let topology_requests = IntCounter::new(
            "bifrost_topology_requests_total",
            "Total number of topology requests"
        )?;
        registry.register(Box::new(topology_requests.clone()))?;

        let topology_nodes_count = Gauge::new(
            "bifrost_topology_nodes",
            "Current number of nodes in topology"
        )?;
        registry.register(Box::new(topology_nodes_count.clone()))?;

        let topology_edges_count = Gauge::new(
            "bifrost_topology_edges",
            "Current number of edges in topology"
        )?;
        registry.register(Box::new(topology_edges_count.clone()))?;

        let errors_total = IntCounter::new(
            "bifrost_errors_total",
            "Total number of errors"
        )?;
        registry.register(Box::new(errors_total.clone()))?;

        Ok(Metrics {
            registry: Arc::new(registry),
            topology_requests,
            topology_nodes_count,
            topology_edges_count,
            errors_total,
        })
    }

    pub fn encode_metrics(&self) -> Result<String, Box<dyn std::error::Error>> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

