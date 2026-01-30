/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use crate::models::{BifrostTopology, NetworkNode, VpnEdge, NodeType, VpnStatus};
use std::collections::HashMap;

/// Punto de entrada principal para obtener la topología.
/// Decida entre datos reales (Linux) o simulados (otros OS).
pub async fn generate_current_topology() -> BifrostTopology {
    #[cfg(target_os = "linux")]
    {
        match obtener_topologia_real_linux().await {
            Ok(topo) => topo,
            Err(e) => {
                eprintln!("Error conectando a StrongSwan: {}. Usando Mock.", e);
                generate_mock_topology()
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        generate_mock_topology()
    }
}

/// Implementación real para Linux usando rsvici.
#[cfg(target_os = "linux")]
async fn obtener_topologia_real_linux() -> anyhow::Result<BifrostTopology> {
    use rsvici::ViciConnection;

    // Conexión al socket Unix de StrongSwan
    let mut conn = ViciConnection::connect("/var/run/charon.vici")?;
    
    // Aquí se obtendrían las Security Associations (SAs)
    // Por ahora mapeamos la estructura básica para que compile
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Nodo central (El propio servidor)
    nodes.push(NetworkNode {
        id: "gateway-root".into(),
        name: "Bifröst Gateway (Local)".into(),
        node_type: NodeType::Gateway,
        address: Some("127.0.0.1".into()),
    });

    // NOTA: Aquí iría la lógica de iterar sobre conn.list_sas(None)
    // para llenar el vector de 'edges' con estados reales.

    Ok(BifrostTopology { nodes, edges })
}

/// Compara dos topologías y devuelve un vector con las alertas generadas.
/// Se dispara solo en la transición exacta de Up -> Down.
pub fn detect_status_changes(old_topology: &BifrostTopology, new_topology: &BifrostTopology) -> Vec<String> {
    let mut alerts = Vec::new();
    
    // Mapa para búsqueda rápida: (from, to) -> status
    let old_edges_map: HashMap<(&String, &String), &VpnStatus> = old_topology
        .edges
        .iter()
        .map(|edge| ((&edge.from_node, &edge.to_node), &edge.status))
        .collect();

    for new_edge in &new_topology.edges {
        if let Some(old_status) = old_edges_map.get(&(&new_edge.from_node, &new_edge.to_node)) {
            if **old_status == VpnStatus::Up && new_edge.status == VpnStatus::Down {
                alerts.push(format!(
                    "ALERTA CRÍTICA: La conexión de '{}' a '{}' se ha perdido.",
                    new_edge.from_node, new_edge.to_node
                ));
            }
        }
    }
    alerts
}

/// Datos de prueba para desarrollo en Windows o si StrongSwan no está disponible.
pub fn generate_mock_topology() -> BifrostTopology {
    BifrostTopology {
        nodes: vec![
            NetworkNode {
                id: "srv-central".into(),
                name: "Bifröst HQ (Mock)".into(),
                node_type: NodeType::Gateway,
                address: Some("10.0.0.1".into()),
            },
            NetworkNode {
                id: "branch-01".into(),
                name: "Sucursal Guatemala".into(),
                node_type: NodeType::RemoteEndpoint,
                address: Some("190.1.1.5".into()),
            },
            NetworkNode {
                id: "branch-02".into(),
                name: "Sucursal México".into(),
                node_type: NodeType::RemoteEndpoint,
                address: Some("201.5.5.1".into()),
            },
        ],
        edges: vec![
            VpnEdge {
                from_node: "srv-central".into(),
                to_node: "branch-01".into(),
                status: VpnStatus::Up,
            },
            VpnEdge {
                from_node: "srv-central".into(),
                to_node: "branch-02".into(),
                status: VpnStatus::Connecting,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_on_status_drop() {
        let node_a = "A".to_string();
        let node_b = "B".to_string();

        let old_topo = BifrostTopology {
            nodes: vec![],
            edges: vec![VpnEdge { from_node: node_a.clone(), to_node: node_b.clone(), status: VpnStatus::Up }],
        };

        let new_topo = BifrostTopology {
            nodes: vec![],
            edges: vec![VpnEdge { from_node: node_a, to_node: node_b, status: VpnStatus::Down }],
        };

        let alerts = detect_status_changes(&old_topo, &new_topo);
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].contains("perdiendo"));
    }
}