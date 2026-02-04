/*
 * Bifröst-Gate: Módulo de Métricas Prometheus
 * Copyright (C) 2026 Estuardo Dardón.
 */

use prometheus::{Registry, TextEncoder, Encoder, IntCounter, Gauge};
use once_cell::sync::OnceCell;
use std::sync::Arc;

static DROPPED_LOGS_COUNTER: OnceCell<IntCounter> = OnceCell::new();

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    pub topology_requests: IntCounter,
    pub topology_nodes_count: Gauge,
    pub topology_edges_count: Gauge,
    // retained metrics are registered in `new()`; some counters are exposed via OnceCell
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

        let dropped_logs = IntCounter::new(
            "bifrost_logs_dropped_total",
            "Number of log messages dropped due to full async channel"
        )?;
        registry.register(Box::new(dropped_logs.clone()))?;

        let _ = DROPPED_LOGS_COUNTER.set(dropped_logs.clone());

        Ok(Metrics {
            registry: Arc::new(registry),
            topology_requests,
            topology_nodes_count,
            topology_edges_count,
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

/// Increment the dropped logs counter from other modules (best-effort).
pub fn incr_dropped_logs() {
    if let Some(counter) = DROPPED_LOGS_COUNTER.get() {
        counter.inc();
    }
}

