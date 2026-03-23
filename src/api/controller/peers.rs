use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::api::types::*;

/// Levanta (inicia) una conexion IKE de un peer de StrongSwan.
#[utoipa::path(
    post,
    path = "/api/peers/{peer_name}/up",
    params(
        ("peer_name" = String, Path, description = "Nombre de la conexion/peer en StrongSwan"),
        PeerControlQuery
    ),
    responses(
        (status = 200, description = "Peer levantado", body = PeerControlResponse),
        (status = 400, description = "Error al ejecutar comando", body = PeerControlResponse),
        (status = 500, description = "Error interno", body = PeerControlResponse),
        (status = 501, description = "Operacion no soportada en este OS", body = PeerControlResponse)
    )
)]
pub async fn peer_up_handler(
    State(state): State<crate::AppState>,
    Path(peer_name): Path<String>,
    Query(query): Query<PeerControlQuery>,
) -> impl IntoResponse {
    crate::api::service::peers::peer_control_handler(state, peer_name, true, query.phase).await
}

/// Baja (termina) una conexion IKE de un peer de StrongSwan.
#[utoipa::path(
    post,
    path = "/api/peers/{peer_name}/down",
    params(
        ("peer_name" = String, Path, description = "Nombre de la conexion/peer en StrongSwan"),
        PeerControlQuery
    ),
    responses(
        (status = 200, description = "Peer bajado", body = PeerControlResponse),
        (status = 400, description = "Error al ejecutar comando", body = PeerControlResponse),
        (status = 500, description = "Error interno", body = PeerControlResponse),
        (status = 501, description = "Operacion no soportada en este OS", body = PeerControlResponse)
    )
)]
pub async fn peer_down_handler(
    State(state): State<crate::AppState>,
    Path(peer_name): Path<String>,
    Query(query): Query<PeerControlQuery>,
) -> impl IntoResponse {
    crate::api::service::peers::peer_control_handler(state, peer_name, false, query.phase).await
}

/// Obtiene el estado detallado de un peer.
#[utoipa::path(
    get,
    path = "/api/peers/{peer_name}/status",
    params(("peer_name" = String, Path, description = "Nombre de la conexion/peer en StrongSwan")),
    responses(
        (status = 200, description = "Estado del peer", body = PeerStatusResponse),
        (status = 400, description = "Nombre invalido", body = PeerStatusErrorResponse),
        (status = 500, description = "Error interno", body = PeerStatusErrorResponse),
        (status = 501, description = "Operacion no soportada", body = PeerStatusErrorResponse)
    )
)]
pub async fn peer_status_handler(
    State(state): State<crate::AppState>,
    Path(peer_name): Path<String>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(PeerStatusErrorResponse {
                peer_name,
                success: false,
                message: "Operacion soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match crate::api::service::connections::sanitize_connection_name(&peer_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(PeerStatusErrorResponse {
                        peer_name,
                        success: false,
                        message: "peer_name invalido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let peer_runtime = match crate::api::service::peers::get_runtime_status_for_peer(&name).await {
            Ok(v) => v,
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error obteniendo estado de peer '{}': {}", name, err));
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PeerStatusErrorResponse {
                        peer_name: name,
                        success: false,
                        message: format!("No se pudo consultar estado del peer: {}", err),
                    }),
                )
                    .into_response();
            }
        };

        let firewall_snapshot = crate::api::service::peers::collect_firewall_rules_snapshot().await;
        let response = crate::api::service::peers::build_peer_status_response(peer_runtime, &firewall_snapshot);
        (StatusCode::OK, Json(response)).into_response()
    }
}

/// Lista el estado detallado de todos los peers conocidos.
#[utoipa::path(
    get,
    path = "/api/peers/status",
    responses(
        (status = 200, description = "Estado de todos los peers", body = PeerStatusListResponse),
        (status = 500, description = "Error interno", body = PeerStatusErrorResponse),
        (status = 501, description = "Operacion no soportada", body = PeerStatusErrorResponse)
    )
)]
pub async fn list_peers_status_handler(
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(PeerStatusErrorResponse {
                peer_name: String::new(),
                success: false,
                message: "Operacion soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let peers = match crate::api::service::peers::get_runtime_status_for_all_peers().await {
            Ok(v) => v,
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error listando estado de peers: {}", err));
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PeerStatusErrorResponse {
                        peer_name: String::new(),
                        success: false,
                        message: format!("No se pudo consultar estado de peers: {}", err),
                    }),
                )
                    .into_response();
            }
        };

        let firewall_snapshot = crate::api::service::peers::collect_firewall_rules_snapshot().await;
        let response = PeerStatusListResponse {
            peers: peers
                .into_iter()
                .map(|peer| crate::api::service::peers::build_peer_status_response(peer, &firewall_snapshot))
                .collect(),
        };

        (StatusCode::OK, Json(response)).into_response()
    }
}
