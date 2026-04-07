use std::collections::HashSet;
use std::time::Duration;

use axum::http::StatusCode;
use axum::Json;
use tokio::time::sleep;

use crate::api::types::{
    ChildRuntimeStatus, ChildSaStatusResponse, FirewallRulesResponse, FirewallRulesSnapshot,
    PeerControlResponse, PeerPhaseStatusResponse, PeerRuntimeStatus, PeerStatusResponse,
};

pub async fn peer_control_handler(
    state: crate::AppState,
    peer_name: String,
    bring_up: bool,
    phase: Option<u8>,
    language: Option<String>,
) -> impl axum::response::IntoResponse {
    let action = if bring_up { "up" } else { "down" };
    let requested_lang = language.as_deref();
    let peer_name = peer_name.trim().to_string();

    if peer_name.is_empty() {
        let message = crate::i18n::message_for_code(
            &state.pool,
            crate::i18n::CODE_PEER_NAME_REQUIRED,
            requested_lang,
        )
        .await;
        return (
            StatusCode::BAD_REQUEST,
            Json(PeerControlResponse {
                code: crate::i18n::CODE_PEER_NAME_REQUIRED,
                peer_name,
                action: action.to_string(),
                success: false,
                message,
            }),
        );
    }

    if let Some(p) = phase {
        if p != 1 && p != 2 {
            let message = crate::i18n::message_for_code(
                &state.pool,
                crate::i18n::CODE_PEER_PHASE_INVALID,
                requested_lang,
            )
            .await;
            return (
                StatusCode::BAD_REQUEST,
                Json(PeerControlResponse {
                    code: crate::i18n::CODE_PEER_PHASE_INVALID,
                    peer_name,
                    action: action.to_string(),
                    success: false,
                    message,
                }),
            );
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let message = crate::i18n::message_for_code(
            &state.pool,
            crate::i18n::CODE_NOT_SUPPORTED,
            requested_lang,
        )
        .await;
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(PeerControlResponse {
                code: crate::i18n::CODE_NOT_SUPPORTED,
                peer_name,
                action: action.to_string(),
                success: false,
                message,
            }),
        );
    }

    #[cfg(target_os = "linux")]
    {
        if bring_up {
            let run_ike = phase.map_or(true, |p| p == 1);
            let run_child = phase.map_or(true, |p| p == 2);

            if run_ike {
                match crate::exec::run_command(
                    &crate::exec::ExecConfig::default(),
                    "swanctl",
                    &["--initiate", "--ike", &peer_name],
                    Some(Duration::from_secs(20)),
                )
                .await
                {
                    Ok(output) => {
                        if output.status_code != Some(0) {
                            let base_message = crate::i18n::message_for_code(
                                &state.pool,
                                crate::i18n::CODE_PEER_IKE_FAILED,
                                requested_lang,
                            )
                            .await;
                            state.logger.error(&format!(
                                "Fallo al levantar IKE para peer '{}': {}",
                                peer_name, output.stderr
                            ));
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(PeerControlResponse {
                                    code: crate::i18n::CODE_PEER_IKE_FAILED,
                                    peer_name,
                                    action: action.to_string(),
                                    success: false,
                                    message: if output.stderr.is_empty() {
                                        base_message
                                    } else {
                                        format!("{}: {}", base_message, output.stderr)
                                    },
                                }),
                            );
                        }
                        state.logger.info(&format!("Fase 1 (IKE) levantada para peer '{}'", peer_name));
                    }
                    Err(err) => {
                        let base_message = crate::i18n::message_for_code(
                            &state.pool,
                            crate::i18n::CODE_INTERNAL_ERROR,
                            requested_lang,
                        )
                        .await;
                        state.logger.error(&format!(
                            "No se pudo ejecutar swanctl para IKE del peer '{}': {}",
                            peer_name, format!("{:?}", err)
                        ));
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(PeerControlResponse {
                                code: crate::i18n::CODE_INTERNAL_ERROR,
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: format!("{}: {:?}", base_message, err),
                            }),
                        );
                    }
                }
            }

            if run_ike && run_child {
                sleep(Duration::from_millis(500)).await;
            }

            if run_child {
                match crate::exec::run_command(
                    &crate::exec::ExecConfig::default(),
                    "swanctl",
                    &["--initiate", "--child", &peer_name],
                    Some(Duration::from_secs(20)),
                )
                .await
                {
                    Ok(output) => {
                        if output.status_code != Some(0) {
                            let base_message = crate::i18n::message_for_code(
                                &state.pool,
                                crate::i18n::CODE_PEER_CHILD_FAILED,
                                requested_lang,
                            )
                            .await;
                            state.logger.error(&format!(
                                "Fallo al levantar CHILD SA para peer '{}': {}",
                                peer_name, output.stderr
                            ));
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(PeerControlResponse {
                                    code: crate::i18n::CODE_PEER_CHILD_FAILED,
                                    peer_name,
                                    action: action.to_string(),
                                    success: false,
                                    message: if output.stderr.is_empty() {
                                        base_message
                                    } else {
                                        format!("{}: {}", base_message, output.stderr)
                                    },
                                }),
                            );
                        }
                        state.logger.info(&format!("Fase 2 (CHILD SA) levantada para peer '{}'", peer_name));
                    }
                    Err(err) => {
                        let base_message = crate::i18n::message_for_code(
                            &state.pool,
                            crate::i18n::CODE_INTERNAL_ERROR,
                            requested_lang,
                        )
                        .await;
                        state.logger.error(&format!(
                            "No se pudo ejecutar swanctl para CHILD SA del peer '{}': {}",
                            peer_name, format!("{:?}", err)
                        ));
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(PeerControlResponse {
                                code: crate::i18n::CODE_INTERNAL_ERROR,
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: format!("{}: {:?}", base_message, err),
                            }),
                        );
                    }
                }
            }

            let message = crate::i18n::message_for_code(
                &state.pool,
                crate::i18n::CODE_OK,
                requested_lang,
            )
            .await;

            return (
                StatusCode::OK,
                Json(PeerControlResponse {
                    code: crate::i18n::CODE_OK,
                    peer_name,
                    action: action.to_string(),
                    success: true,
                    message,
                }),
            );
        } else {
            let child_only = phase == Some(2);

            let args: Vec<&str> = if child_only {
                vec!["--terminate", "--child", &peer_name]
            } else {
                vec!["--terminate", "--ike", &peer_name]
            };
            match crate::exec::run_command(
                &crate::exec::ExecConfig::default(),
                "swanctl",
                &args,
                Some(Duration::from_secs(20)),
            )
            .await
            {
                Ok(output) => {
                    if output.status_code == Some(0) {
                        let message = crate::i18n::message_for_code(
                            &state.pool,
                            crate::i18n::CODE_OK,
                            requested_lang,
                        )
                        .await;
                        let log_desc = if child_only { "Fase 2 (CHILD SA)" } else { "peer" };
                        state.logger.info(&format!("{} '{}' bajado", log_desc, peer_name));

                        (
                            StatusCode::OK,
                            Json(PeerControlResponse {
                                code: crate::i18n::CODE_OK,
                                peer_name,
                                action: action.to_string(),
                                success: true,
                                message,
                            }),
                        )
                    } else {
                        let base_message = crate::i18n::message_for_code(
                            &state.pool,
                            crate::i18n::CODE_PEER_CHILD_FAILED,
                            requested_lang,
                        )
                        .await;
                        state.logger.error(&format!(
                            "Fallo al bajar peer '{}': {}",
                            peer_name, output.stderr
                        ));
                        (
                            StatusCode::BAD_REQUEST,
                            Json(PeerControlResponse {
                                code: crate::i18n::CODE_PEER_CHILD_FAILED,
                                peer_name,
                                action: action.to_string(),
                                success: false,
                                message: if output.stderr.is_empty() {
                                    base_message
                                } else {
                                    format!("{}: {}", base_message, output.stderr)
                                },
                            }),
                        )
                    }
                }
                Err(err) => {
                    let base_message = crate::i18n::message_for_code(
                        &state.pool,
                        crate::i18n::CODE_INTERNAL_ERROR,
                        requested_lang,
                    )
                    .await;
                    state.logger.error(&format!(
                        "No se pudo ejecutar swanctl para bajar peer '{}': {}",
                        peer_name, format!("{:?}", err)
                    ));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(PeerControlResponse {
                            code: crate::i18n::CODE_INTERNAL_ERROR,
                            peer_name,
                            action: action.to_string(),
                            success: false,
                            message: format!("{}: {:?}", base_message, err),
                        }),
                    )
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub async fn get_runtime_status_for_peer(peer_name: &str) -> Result<PeerRuntimeStatus, String> {
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "swanctl",
        &["--list-sas", "--ike", peer_name],
        Some(Duration::from_secs(20)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar swanctl --list-sas: {:?}", err))?;

    if output.status_code != Some(0) {
        if output.stdout.is_empty() && output.stderr.to_ascii_lowercase().contains("not found") {
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

        return Err(if output.stderr.is_empty() {
            format!("swanctl --list-sas --ike {} fallo sin detalle", peer_name)
        } else {
            output.stderr
        });
    }

    let parsed = parse_peer_runtime_statuses(&output.stdout);
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
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "swanctl",
        &["--list-sas"],
        Some(Duration::from_secs(20)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar swanctl --list-sas: {:?}", err))?;

    if output.status_code != Some(0) {
        return Err(if output.stderr.is_empty() {
            "swanctl --list-sas fallo sin detalle".to_string()
        } else {
            output.stderr
        });
    }

    let mut parsed = parse_peer_runtime_statuses(&output.stdout);
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
    let output = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "swanctl",
        &["--list-conns"],
        Some(Duration::from_secs(20)),
    )
    .await
    .map_err(|err| format!("No se pudo ejecutar swanctl --list-conns: {:?}", err))?;

    if output.status_code == Some(0) {
        let stdout = output.stdout;
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

    if let Ok(output) = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "nft",
        &["list", "ruleset"],
        Some(Duration::from_secs(5)),
    )
    .await
    {
        if output.status_code == Some(0) {
            snapshot.firewall = output.stdout.lines().map(|line| line.to_string()).collect();
        }
    }

    if let Ok(output) = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "iptables-save",
        &["-t", "filter"],
        Some(Duration::from_secs(5)),
    )
    .await
    {
        if output.status_code == Some(0) {
            snapshot.filter = output.stdout.lines().map(|line| line.to_string()).collect();
        }
    }

    if let Ok(output) = crate::exec::run_command(
        &crate::exec::ExecConfig::default(),
        "iptables-save",
        &["-t", "nat"],
        Some(Duration::from_secs(5)),
    )
    .await
    {
        if output.status_code == Some(0) {
            snapshot.nat = output.stdout.lines().map(|line| line.to_string()).collect();
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
