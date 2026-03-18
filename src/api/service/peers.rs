use std::collections::HashSet;
use std::time::Duration;

use axum::http::StatusCode;
use axum::Json;
use tokio::process::Command;
use tokio::time::sleep;

use crate::api::types::{
    ChildRuntimeStatus, ChildSaStatusResponse, FirewallRulesResponse, FirewallRulesSnapshot,
    PeerControlResponse, PeerPhaseStatusResponse, PeerRuntimeStatus, PeerStatusResponse,
};

pub async fn peer_control_handler(
    state: crate::AppState,
    peer_name: String,
    bring_up: bool,
) -> impl axum::response::IntoResponse {
    let action = if bring_up { "up" } else { "down" };
    let peer_name = peer_name.trim().to_string();

    if peer_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(PeerControlResponse {
                peer_name,
                action: action.to_string(),
                success: false,
                message: "peer_name es requerido".to_string(),
            }),
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(PeerControlResponse {
                peer_name,
                action: action.to_string(),
                success: false,
                message: "Operacion soportada solo en Linux con StrongSwan".to_string(),
            }),
        );
    }

    #[cfg(target_os = "linux")]
    {
        if bring_up {
            let mut cmd_ike = Command::new("swanctl");
            cmd_ike.arg("--initiate").arg("--ike").arg(&peer_name);

            match cmd_ike.output().await {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        state.logger.error(&format!(
                            "Fallo al levantar IKE para peer '{}': {}",
                            peer_name, stderr
                        ));
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(PeerControlResponse {
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: if stderr.is_empty() {
                                    "swanctl fallo al iniciar IKE sin detalle".to_string()
                                } else {
                                    format!("Error IKE: {}", stderr)
                                },
                            }),
                        );
                    }
                    state.logger.info(&format!("Fase 1 (IKE) levantada para peer '{}'", peer_name));
                }
                Err(err) => {
                    state.logger.error(&format!(
                        "No se pudo ejecutar swanctl para IKE del peer '{}': {}",
                        peer_name, err
                    ));
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(PeerControlResponse {
                            peer_name,
                            action: action.to_string(),
                            success: false,
                            message: format!("Error ejecutando swanctl (IKE): {}", err),
                        }),
                    );
                }
            }

            sleep(Duration::from_millis(500)).await;

            let mut cmd_child = Command::new("swanctl");
            cmd_child.arg("--initiate").arg("--child").arg(&peer_name);

            match cmd_child.output().await {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        state.logger.error(&format!(
                            "Fallo al levantar CHILD SA para peer '{}': {}",
                            peer_name, stderr
                        ));
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(PeerControlResponse {
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: if stderr.is_empty() {
                                    "swanctl fallo al iniciar CHILD SA sin detalle".to_string()
                                } else {
                                    format!("Error CHILD SA (Fase 2): {}", stderr)
                                },
                            }),
                        );
                    }
                    state.logger.info(&format!("Fase 2 (CHILD SA) levantada para peer '{}'", peer_name));

                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let message = if stdout.is_empty() {
                        format!("VPN '{}' levantada (Fase 1 + Fase 2)", peer_name)
                    } else {
                        stdout
                    };

                    return (
                        StatusCode::OK,
                        Json(PeerControlResponse {
                            peer_name,
                            action: action.to_string(),
                            success: true,
                            message,
                        }),
                    );
                }
                Err(err) => {
                    state.logger.error(&format!(
                        "No se pudo ejecutar swanctl para CHILD SA del peer '{}': {}",
                        peer_name, err
                    ));
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(PeerControlResponse {
                            peer_name,
                            action: action.to_string(),
                            success: false,
                            message: format!("Error ejecutando swanctl (CHILD SA): {}", err),
                        }),
                    );
                }
            }
        } else {
            let mut cmd = Command::new("swanctl");
            cmd.arg("--terminate").arg("--ike").arg(&peer_name);

            match cmd.output().await {
                Ok(output) => {
                    if output.status.success() {
                        state.logger.info(&format!("Peer '{}' bajado", peer_name));
                        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        let message = if stdout.is_empty() {
                            format!("Peer '{}' bajado exitosamente", peer_name)
                        } else {
                            stdout
                        };

                        (
                            StatusCode::OK,
                            Json(PeerControlResponse {
                                peer_name,
                                action: action.to_string(),
                                success: true,
                                message,
                            }),
                        )
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        state.logger.error(&format!("Fallo al bajar peer '{}': {}", peer_name, stderr));
                        (
                            StatusCode::BAD_REQUEST,
                            Json(PeerControlResponse {
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: if stderr.is_empty() {
                                    "swanctl devolvio error sin detalle".to_string()
                                } else {
                                    stderr
                                },
                            }),
                        )
                    }
                }
                Err(err) => {
                    state.logger.error(&format!(
                        "No se pudo ejecutar swanctl para bajar peer '{}': {}",
                        peer_name, err
                    ));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(PeerControlResponse {
                            peer_name,
                            action: action.to_string(),
                            success: false,
                            message: format!("Error ejecutando swanctl: {}", err),
                        }),
                    )
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub async fn get_runtime_status_for_peer(peer_name: &str) -> Result<PeerRuntimeStatus, String> {
    let mut cmd = Command::new("swanctl");
    cmd.arg("--list-sas").arg("--ike").arg(peer_name);

    let output = cmd
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar swanctl --list-sas: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() && stderr.to_ascii_lowercase().contains("not found") {
            return Ok(PeerRuntimeStatus {
                peer_name: peer_name.to_string(),
                phase1_state: "DOWN".to_string(),
                phase1_active: false,
                phase1_active_for_seconds: None,
                phase1_packets_in: 0,
                phase1_packets_out: 0,
                child_sas: Vec::new(),
            });
        }

        return Err(if stderr.is_empty() {
            format!("swanctl --list-sas --ike {} fallo sin detalle", peer_name)
        } else {
            stderr
        });
    }

    let parsed = parse_peer_runtime_statuses(&String::from_utf8_lossy(&output.stdout));
    Ok(parsed
        .into_iter()
        .find(|item| item.peer_name == peer_name)
        .unwrap_or(PeerRuntimeStatus {
            peer_name: peer_name.to_string(),
            phase1_state: "DOWN".to_string(),
            phase1_active: false,
            phase1_active_for_seconds: None,
            phase1_packets_in: 0,
            phase1_packets_out: 0,
            child_sas: Vec::new(),
        }))
}

#[cfg(target_os = "linux")]
pub async fn get_runtime_status_for_all_peers() -> Result<Vec<PeerRuntimeStatus>, String> {
    let mut cmd = Command::new("swanctl");
    cmd.arg("--list-sas");
    let output = cmd
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar swanctl --list-sas: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "swanctl --list-sas fallo sin detalle".to_string()
        } else {
            stderr
        });
    }

    let mut parsed = parse_peer_runtime_statuses(&String::from_utf8_lossy(&output.stdout));
    let configured = list_all_peer_names().await?;
    let mut seen: HashSet<String> = parsed.iter().map(|p| p.peer_name.clone()).collect();

    for name in configured {
        if seen.contains(&name) {
            continue;
        }
        parsed.push(PeerRuntimeStatus {
            peer_name: name.clone(),
            phase1_state: "DOWN".to_string(),
            phase1_active: false,
            phase1_active_for_seconds: None,
            phase1_packets_in: 0,
            phase1_packets_out: 0,
            child_sas: Vec::new(),
        });
        seen.insert(name);
    }

    parsed.sort_by(|a, b| a.peer_name.cmp(&b.peer_name));
    Ok(parsed)
}

#[cfg(target_os = "linux")]
async fn list_all_peer_names() -> Result<Vec<String>, String> {
    let output = Command::new("swanctl")
        .arg("--list-conns")
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar swanctl --list-conns: {}", err))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut names = Vec::new();
        for raw_line in stdout.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') && line.contains(':') {
                if let Some(name) = line.split(':').next().map(str::trim) {
                    if !name.is_empty() {
                        names.push(name.to_string());
                    }
                }
            }
        }

        names.sort();
        names.dedup();
        return Ok(names);
    }

    crate::api::service::connections::list_managed_connections()
        .await
        .map(|mut names| {
            names.retain(|name| !name.starts_with("secret-"));
            names.sort();
            names.dedup();
            names
        })
        .map_err(|err| format!("No se pudo listar conexiones gestionadas: {}", err))
}

#[cfg(target_os = "linux")]
fn parse_peer_runtime_statuses(output: &str) -> Vec<PeerRuntimeStatus> {
    let mut peers = Vec::new();
    let mut current: Option<PeerRuntimeStatus> = None;

    for raw_line in output.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') && trimmed.contains(": #") {
            if let Some(prev) = current.take() {
                peers.push(prev);
            }

            let (name, state) = parse_ike_header_line(trimmed);
            current = Some(PeerRuntimeStatus {
                peer_name: name,
                phase1_state: state.clone(),
                phase1_active: is_sa_active_state(&state),
                phase1_active_for_seconds: parse_active_for_seconds(trimmed),
                phase1_packets_in: parse_counter_value(trimmed, "packets_i"),
                phase1_packets_out: parse_counter_value(trimmed, "packets_o"),
                child_sas: Vec::new(),
            });
            continue;
        }

        if let Some(peer) = current.as_mut() {
            if let Some(child) = parse_child_sa_line(trimmed, &peer.peer_name) {
                peer.child_sas.push(child);
            }
        }
    }

    if let Some(prev) = current {
        peers.push(prev);
    }

    peers
}

#[cfg(target_os = "linux")]
fn parse_ike_header_line(line: &str) -> (String, String) {
    let mut split = line.splitn(2, ':');
    let name = split.next().unwrap_or_default().trim().to_string();
    let rest = split.next().unwrap_or_default();
    let state = rest
        .split(',')
        .nth(1)
        .map(str::trim)
        .unwrap_or("UNKNOWN")
        .to_string();
    (name, state)
}

#[cfg(target_os = "linux")]
fn parse_child_sa_line(line: &str, peer_name: &str) -> Option<ChildRuntimeStatus> {
    let (left, right) = line.split_once(':')?;
    let left = left.trim();

    // Formato legado: "peer{N}: INSTALLED, ..."
    // Formato moderno: "peer: #N, reqid X, INSTALLED, ..."
    let (name, state) = if left.contains('{') && left.contains('}') {
        let prefix = left.split('{').next()?.trim();
        if prefix != peer_name {
            return None;
        }

        let state = right
            .split(',')
            .next()
            .map(str::trim)
            .unwrap_or("UNKNOWN")
            .to_string();

        (left.to_string(), state)
    } else if left == peer_name && right.trim_start().starts_with('#') && right.contains("reqid") {
        let state = right
            .split(',')
            .nth(2)
            .map(str::trim)
            .unwrap_or("UNKNOWN")
            .to_string();

        (left.to_string(), state)
    } else {
        return None;
    };

    Some(ChildRuntimeStatus {
        name,
        state: state.clone(),
        active: is_sa_active_state(&state),
        active_for_seconds: parse_active_for_seconds(line),
        packets_in: parse_counter_value(line, "packets_i"),
        packets_out: parse_counter_value(line, "packets_o"),
    })
}

#[cfg(target_os = "linux")]
fn is_sa_active_state(state: &str) -> bool {
    matches!(state, "ESTABLISHED" | "INSTALLED" | "REKEYED")
}

#[cfg(target_os = "linux")]
fn parse_active_for_seconds(text: &str) -> Option<u64> {
    let tokens = text
        .split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        .collect::<Vec<_>>();

    for token in &tokens {
        if let Some(value) = parse_compact_duration_seconds(token) {
            return Some(value);
        }
    }

    for idx in 0..tokens.len().saturating_sub(1) {
        let count = match tokens[idx].parse::<u64>() {
            Ok(value) => value,
            Err(_) => continue,
        };

        let unit = tokens[idx + 1].to_ascii_lowercase();
        let multiplier = if unit.starts_with("sec") || unit.starts_with("second") {
            Some(1)
        } else if unit.starts_with("min") || unit.starts_with("minute") {
            Some(60)
        } else if unit.starts_with("hour") {
            Some(3600)
        } else if unit.starts_with("day") {
            Some(86400)
        } else {
            None
        };

        if let Some(multiplier) = multiplier {
            return Some(count.saturating_mul(multiplier));
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn parse_compact_duration_seconds(token: &str) -> Option<u64> {
    if token.len() < 2 {
        return None;
    }

    let (number, unit) = token.split_at(token.len().saturating_sub(1));
    let count = number.parse::<u64>().ok()?;
    let multiplier = match unit.to_ascii_lowercase().as_str() {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return None,
    };

    Some(count.saturating_mul(multiplier))
}

#[cfg(target_os = "linux")]
fn parse_counter_value(text: &str, marker: &str) -> u64 {
    for part in text.split(',') {
        let trimmed = part.trim();
        if !trimmed.contains(marker) {
            continue;
        }

        if let Some(value) = trimmed
            .split_whitespace()
            .find_map(|token| token.parse::<u64>().ok())
        {
            return value;
        }
    }
    0
}

#[cfg(target_os = "linux")]
pub async fn collect_firewall_rules_snapshot() -> FirewallRulesSnapshot {
    let mut snapshot = FirewallRulesSnapshot::default();

    if let Ok(output) = Command::new("nft").arg("list").arg("ruleset").output().await {
        if output.status.success() {
            snapshot.firewall = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_string())
                .collect();
        }
    }

    if let Ok(output) = Command::new("iptables-save")
        .arg("-t")
        .arg("filter")
        .output()
        .await
    {
        if output.status.success() {
            snapshot.filter = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_string())
                .collect();
        }
    }

    if let Ok(output) = Command::new("iptables-save")
        .arg("-t")
        .arg("nat")
        .output()
        .await
    {
        if output.status.success() {
            snapshot.nat = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_string())
                .collect();
        }
    }

    snapshot
}

#[cfg(target_os = "linux")]
pub fn build_peer_status_response(
    runtime: PeerRuntimeStatus,
    firewall_snapshot: &FirewallRulesSnapshot,
) -> PeerStatusResponse {
    let mut phase2_packets_in = 0_u64;
    let mut phase2_packets_out = 0_u64;
    let mut phase2_active_for_seconds: Option<u64> = None;
    let mut phase2_active = false;
    let mut phase2_state = "DOWN".to_string();

    for child in &runtime.child_sas {
        phase2_packets_in = phase2_packets_in.saturating_add(child.packets_in);
        phase2_packets_out = phase2_packets_out.saturating_add(child.packets_out);

        if child.active {
            phase2_active = true;
            phase2_state = "INSTALLED".to_string();
        } else if phase2_state == "DOWN" {
            phase2_state = child.state.clone();
        }

        phase2_active_for_seconds = match (phase2_active_for_seconds, child.active_for_seconds) {
            (Some(current), Some(value)) => Some(current.max(value)),
            (None, Some(value)) => Some(value),
            (existing, None) => existing,
        };
    }

    let filtered_rules = filter_rules_for_peer(firewall_snapshot, &runtime.peer_name);

    PeerStatusResponse {
        peer_name: runtime.peer_name,
        phase1: PeerPhaseStatusResponse {
            state: runtime.phase1_state,
            active: runtime.phase1_active,
            active_for_seconds: runtime.phase1_active_for_seconds,
            packets_in: runtime.phase1_packets_in,
            packets_out: runtime.phase1_packets_out,
        },
        phase2: PeerPhaseStatusResponse {
            state: phase2_state,
            active: phase2_active,
            active_for_seconds: phase2_active_for_seconds,
            packets_in: phase2_packets_in,
            packets_out: phase2_packets_out,
        },
        child_sas: runtime
            .child_sas
            .into_iter()
            .map(|child| ChildSaStatusResponse {
                name: child.name,
                state: child.state,
                active: child.active,
                active_for_seconds: child.active_for_seconds,
                packets_in: child.packets_in,
                packets_out: child.packets_out,
            })
            .collect(),
        firewall_rules: filtered_rules,
    }
}

#[cfg(target_os = "linux")]
fn filter_rules_for_peer(snapshot: &FirewallRulesSnapshot, peer_name: &str) -> FirewallRulesResponse {
    let needle = peer_name.to_ascii_lowercase();
    let filter_lines = |lines: &[String]| {
        lines
            .iter()
            .filter(|line| line.to_ascii_lowercase().contains(&needle))
            .cloned()
            .collect::<Vec<_>>()
    };

    FirewallRulesResponse {
        firewall: filter_lines(&snapshot.firewall),
        filter: filter_lines(&snapshot.filter),
        nat: filter_lines(&snapshot.nat),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parse_child_sa_legacy_format_sets_phase2_installed() {
        let output = concat!(
            "peer-mock: #1, ESTABLISHED, IKEv2, local_i remote_r\n",
            "  local  'local-mock-id' @ 192.0.2.10[4500]\n",
            "  remote 'remote-mock-id' @ 198.51.100.20[4500]\n",
            "  peer-mock{1}: INSTALLED, TUNNEL, reqid 1, ESP:AES_CBC-256/HMAC_SHA2_256_128\n",
            "    installed 21s ago, rekeying in 2522s, expires in 3579s\n",
            "    in  c58297e7,      0 bytes,     0 packets\n",
            "    out c081bcb2,      0 bytes,     0 packets\n"
        );

        let peers = parse_peer_runtime_statuses(output);
        let peer = peers
            .into_iter()
            .find(|p| p.peer_name == "peer-mock" && !p.child_sas.is_empty())
            .expect("peer esperado con child sa");

        let response = build_peer_status_response(peer, &FirewallRulesSnapshot::default());
        assert_eq!(response.phase2.state, "INSTALLED");
        assert!(response.phase2.active);
    }

    #[test]
    fn parse_child_sa_modern_format_sets_phase2_installed() {
        let output = concat!(
            "peer-mock: #1, ESTABLISHED, IKEv2, local_i remote_r\n",
            "  local  'local-mock-id' @ 192.0.2.10[4500]\n",
            "  remote 'remote-mock-id' @ 198.51.100.20[4500]\n",
            "  peer-mock: #3, reqid 1, INSTALLED, TUNNEL-in-UDP, ESP:AES_CBC-256/HMAC_SHA2_256_128\n",
            "    installed 1215s ago, rekeying in 1755s, expires in 2385s\n",
            "    in  ccb5f1ef,  60352 bytes,   196 packets,     9s ago\n",
            "    out cf529813,  25312 bytes,   316 packets,     9s ago\n"
        );

        let peers = parse_peer_runtime_statuses(output);
        let peer = peers
            .into_iter()
            .find(|p| p.peer_name == "peer-mock" && !p.child_sas.is_empty())
            .expect("peer esperado con child sa");

        let response = build_peer_status_response(peer, &FirewallRulesSnapshot::default());
        assert_eq!(response.phase2.state, "INSTALLED");
        assert!(response.phase2.active);
    }
}
