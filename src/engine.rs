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
use std::env;

#[cfg(target_os = "linux")]
use anyhow::{Context, anyhow};

#[cfg(target_os = "linux")]
use tokio::process::Command;

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
struct ConnInfo {
    name: String,
    remote_hint: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
struct ActiveSa {
    name: String,
    state: String,
    remote_addr: Option<String>,
}

/// Punto de entrada principal para obtener la topología.
/// Decida entre datos reales (Linux) o simulados (otros OS).
pub async fn generate_current_topology() -> BifrostTopology {
    if should_force_mock_topology() {
        return generate_mock_topology();
    }

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

fn should_force_mock_topology() -> bool {
    match env::var("BIFROST_FORCE_MOCK") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// Implementación real para Linux usando rsvici.
#[cfg(target_os = "linux")]
async fn obtener_topologia_real_linux() -> anyhow::Result<BifrostTopology> {
    // 1) Verificación de conectividad con VICI.
    // Si falla, no seguimos con swanctl porque el daemon no está disponible.
    let _conn = rsvici::unix::connect("/var/run/charon.vici")
        .await
        .context("No se pudo conectar al socket VICI /var/run/charon.vici")?;

    // 2) Obtener conexiones configuradas (fuente para estado Down).
    let list_conns_output = Command::new("swanctl")
        .arg("--list-conns")
        .output()
        .await
        .context("Fallo al ejecutar swanctl --list-conns")?;
    if !list_conns_output.status.success() {
        return Err(anyhow!(
            "swanctl --list-conns devolvio codigo {}: {}",
            list_conns_output.status,
            String::from_utf8_lossy(&list_conns_output.stderr)
        ));
    }

    // 3) Obtener SAs activas (fuente para Up/Connecting).
    let list_sas_output = Command::new("swanctl")
        .arg("--list-sas")
        .output()
        .await
        .context("Fallo al ejecutar swanctl --list-sas")?;
    if !list_sas_output.status.success() {
        return Err(anyhow!(
            "swanctl --list-sas devolvio codigo {}: {}",
            list_sas_output.status,
            String::from_utf8_lossy(&list_sas_output.stderr)
        ));
    }

    let conns_stdout = String::from_utf8_lossy(&list_conns_output.stdout);
    let sas_stdout = String::from_utf8_lossy(&list_sas_output.stdout);

    let configured = parse_configured_connections(&conns_stdout);
    let active = parse_active_sas(&sas_stdout);

    Ok(build_topology_from_strongswan(configured, active))
}

#[cfg(target_os = "linux")]
fn build_topology_from_strongswan(configured: Vec<ConnInfo>, active: Vec<ActiveSa>) -> BifrostTopology {
    let gateway_id = "gateway-root".to_string();
    let mut nodes = vec![NetworkNode {
        id: gateway_id.clone(),
        name: "Bifröst Gateway (Local)".into(),
        node_type: NodeType::Gateway,
        address: Some("127.0.0.1".into()),
    }];

    let mut edges = Vec::new();
    let mut conn_status: HashMap<String, VpnStatus> = HashMap::new();
    let mut conn_remote: HashMap<String, Option<String>> = HashMap::new();
    let mut configured_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for sa in active {
        let status = map_ike_state_to_status(&sa.state);
        conn_status.insert(sa.name.clone(), status);
        conn_remote.insert(sa.name, sa.remote_addr);
    }

    for conn in configured {
        configured_names.insert(conn.name.clone());
        let node_id = format!("peer-{}", sanitize_id(&conn.name));
        let remote_addr = conn_remote
            .get(&conn.name)
            .cloned()
            .flatten()
            .or(conn.remote_hint.clone());

        nodes.push(NetworkNode {
            id: node_id.clone(),
            name: conn.name.clone(),
            node_type: NodeType::RemoteEndpoint,
            address: remote_addr,
        });

        edges.push(VpnEdge {
            from_node: gateway_id.clone(),
            to_node: node_id,
            status: conn_status
                .get(&conn.name)
                .cloned()
                .unwrap_or(VpnStatus::Down),
        });
    }

    // Si hay SAs activas que no aparecieron en --list-conns, también se reflejan.
    for (name, status) in conn_status {
        if configured_names.contains(&name) {
            continue;
        }

        let node_id = format!("peer-{}", sanitize_id(&name));
        let remote_addr = conn_remote.get(&name).cloned().flatten();
        nodes.push(NetworkNode {
            id: node_id.clone(),
            name: name.clone(),
            node_type: NodeType::RemoteEndpoint,
            address: remote_addr,
        });

        edges.push(VpnEdge {
            from_node: gateway_id.clone(),
            to_node: node_id,
            status,
        });
    }

    BifrostTopology { nodes, edges }
}

#[cfg(target_os = "linux")]
fn parse_configured_connections(output: &str) -> Vec<ConnInfo> {
    let mut conns = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_remote: Option<String> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Top-level examples:
        // "connA: IKEv2, no reauthentication"
        // "connB:"
        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') && line.contains(':') {
            if let Some(name) = current_name.take() {
                conns.push(ConnInfo {
                    name,
                    remote_hint: current_remote.take(),
                });
            }

            let name = line
                .split(':')
                .next()
                .map(str::trim)
                .unwrap_or_default();
            if !name.is_empty() {
                current_name = Some(name.to_string());
                current_remote = None;
            }
            continue;
        }

        if line.starts_with("remote:") {
            let value = line.trim_start_matches("remote:").trim();
            if !value.is_empty() {
                current_remote = Some(value.to_string());
            }
        }
    }

    if let Some(name) = current_name {
        conns.push(ConnInfo {
            name,
            remote_hint: current_remote,
        });
    }

    conns
}

#[cfg(target_os = "linux")]
fn parse_active_sas(output: &str) -> Vec<ActiveSa> {
    let mut result = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_state: Option<String> = None;
    let mut current_remote: Option<String> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Top-level SA examples:
        // "connA: #1, ESTABLISHED, IKEv2, ..."
        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') && line.contains(':') {
            if let (Some(name), Some(state)) = (current_name.take(), current_state.take()) {
                result.push(ActiveSa {
                    name,
                    state,
                    remote_addr: current_remote.take(),
                });
            }

            let mut split = line.splitn(2, ':');
            let name = split.next().unwrap_or_default().trim();
            let rest = split.next().unwrap_or_default();
            let state = rest
                .split(',')
                .nth(1)
                .map(str::trim)
                .unwrap_or("UNKNOWN");

            if !name.is_empty() {
                current_name = Some(name.to_string());
                current_state = Some(state.to_string());
                current_remote = None;
            }
            continue;
        }

        // "remote 'id' @ 203.0.113.10[4500]"
        if line.starts_with("remote ") {
            current_remote = extract_remote_ip(line);
        }
    }

    if let (Some(name), Some(state)) = (current_name, current_state) {
        result.push(ActiveSa {
            name,
            state,
            remote_addr: current_remote,
        });
    }

    result
}

#[cfg(target_os = "linux")]
fn extract_remote_ip(line: &str) -> Option<String> {
    let at_pos = line.find("@ ")? + 2;
    let tail = &line[at_pos..];
    let end = tail.find('[').unwrap_or(tail.len());
    let ip = tail[..end].trim();
    if ip.is_empty() {
        None
    } else {
        Some(ip.to_string())
    }
}

#[cfg(target_os = "linux")]
fn map_ike_state_to_status(state: &str) -> VpnStatus {
    match state {
        "ESTABLISHED" | "INSTALLED" | "REKEYED" => VpnStatus::Up,
        "CONNECTING" | "CONNECTING_CHILD" | "REKEYING" => VpnStatus::Connecting,
        _ => VpnStatus::Down,
    }
}

#[cfg(target_os = "linux")]
fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
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
        assert!(alerts[0].contains("perdido"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_active_sas_and_remote_ip() {
        let input = "connA: #1, ESTABLISHED, IKEv2, 1234abcd\n  local  'gw' @ 10.0.0.1[4500]\n  remote 'peer' @ 203.0.113.10[4500]\n";
        let parsed = parse_active_sas(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "connA");
        assert_eq!(parsed[0].state, "ESTABLISHED");
        assert_eq!(parsed[0].remote_addr.as_deref(), Some("203.0.113.10"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_configured_connections() {
        let input = "connA: IKEv2\n  local:  10.0.0.1\n  remote: 203.0.113.10\nconnB:\n  remote: 198.51.100.5\n";
        let parsed = parse_configured_connections(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "connA");
        assert_eq!(parsed[0].remote_hint.as_deref(), Some("203.0.113.10"));
        assert_eq!(parsed[1].name, "connB");
        assert_eq!(parsed[1].remote_hint.as_deref(), Some("198.51.100.5"));
    }
}