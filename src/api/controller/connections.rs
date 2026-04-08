use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
#[allow(unused_imports)]
use serde_json::json;
use crate::api::types::*;

#[utoipa::path(
    get,
    path = "/api/connections",
    responses(
        (status = 200, description = "Listado de conexiones", body = ConnectionListResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn list_connections_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::list_connections_handler(state, Some(lang)).await
}

#[utoipa::path(
    get,
    path = "/api/connections/{connection_name}",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion encontrada", body = ConnectionResponse),
        (status = 400, description = "Nombre invalido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn get_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_read_handler(state, connection_name, Some(lang)).await
}

#[utoipa::path(
    post,
    path = "/api/connections",
    request_body(
        content = ConnectionCreateRequest,
        examples(
            (
                "peer_to_peer_psk" = (
                    summary = "Peer to Peer con PSK",
                    description = "Flujo end-to-end: 1) POST /api/secrets con secret_type=ike y config.ids=[local.id, remote.id]. 2) POST /api/connections con este ejemplo. 3) GET /api/connections/{connection_name} o verifica estado del túnel. Los IDs del secret deben coincidir con local.id y remote.id.",
                    value = json!({
                        "name": "peer-to-peer",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "remote_addrs": "2.2.2.2",
                            "proposals": "aes256-sha256-modp1024!",
                            "rekey_time": "86400s",
                            "local": {
                                "id": "1.1.1.1",
                                "auth": "psk"
                            },
                            "remote": {
                                "id": "2.2.2.2",
                                "auth": "psk"
                            },
                            "children": {
                                "net-net": {
                                    "local_ts": "10.0.0.1/32",
                                    "remote_ts": "10.1.1.1/32",
                                    "esp_proposals": "aes256-sha256!",
                                    "rekey_time": "3600s",
                                    "dpd_action": "restart",
                                    "dpd_delay": "30s",
                                    "close_action": "restart",
                                    "start_action": "start",
                                    "encap": true
                                }
                            }
                        }
                    })
                )
            ),
            (
                "peer_to_any_eap_mschapv2" = (
                    summary = "Peer to Any con EAP-MSCHAPv2",
                    description = "Flujo end-to-end: 1) Crea un secret por usuario remoto en POST /api/secrets con secret_type=eap y config.id=<usuario>. 2) Crea la conexión con remote.auth=eap-mschapv2. 3) Para rotar contraseñas, usa PUT /api/secrets/{secret_name}.",
                    value = json!({
                        "name": "peer-to-any-eap",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "1.1.1.1",
                                "auth": "pubkey",
                                "certs": ["serverCert.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "eap-mschapv2",
                                "eap_id": "%any"
                            },
                            "children": {
                                "net-any": {
                                    "local_ts": ["10.1.1.0/24", "10.2.1.0/24"],
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "trap",
                                    "dpd_action": "clear"
                                }
                            },
                            "pools": "vpn-pool",
                            "install_policy": true
                        }
                    })
                )
            ),
            (
                "peer_to_any_user_certificate" = (
                    summary = "Peer to Any con usuarios por certificado",
                    description = "Flujo end-to-end: 1) Crea CA y certificados en /api/certificates. 2) Crea esta conexión con remote.auth=pubkey y remote.cacerts. 3) Opcional: usa POST /api/connections/{connection_name}/certificate para adjuntar certificado/local_id remoto automáticamente.",
                    value = json!({
                        "name": "peer-to-any-user-cert",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "vpn.example.com",
                                "auth": "pubkey",
                                "certs": ["serverCert.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "pubkey",
                                "cacerts": ["caCert.pem"]
                            },
                            "children": {
                                "rw-cert": {
                                    "local_ts": "10.10.0.0/16",
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "trap"
                                }
                            },
                            "pools": "rw-pool"
                        }
                    })
                )
            ),
            (
                "peer_to_any_mutual_certificates" = (
                    summary = "Peer to Any con certificados mutuos",
                    description = "Flujo end-to-end: 1) Administra certificados en /api/certificates (local y CA remota). 2) Crea la conexión con auth=pubkey en local y remote. 3) Ajusta asociación de certificado de conexión con /api/connections/{connection_name}/certificate cuando aplique.",
                    value = json!({
                        "name": "peer-to-any-mutual-cert",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "remote_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "gw.local.example",
                                "auth": "pubkey",
                                "certs": ["gw-local.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "pubkey",
                                "cacerts": ["remote-ca.pem"]
                            },
                            "children": {
                                "default": {
                                    "local_ts": "10.30.0.0/16",
                                    "remote_ts": "0.0.0.0/0",
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "add"
                                }
                            }
                        }
                    })
                )
            ),
            (
                "peer_to_any_multi_user_certificates" = (
                    summary = "Peer to Any con múltiples usuarios por certificado",
                    description = "Flujo end-to-end: 1) Crea CA en /api/certificates/ca. 2) Emite un certificado por usuario en /api/certificates/user (alice, bob, etc.) con esa CA. 3) Crea esta conexión para aceptar múltiples clientes por identidad de certificado (remote.id=%any + remote.auth=pubkey + remote.cacerts).",
                    value = json!({
                        "name": "rw-multi-user-cert",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "vpn.example.com",
                                "auth": "pubkey",
                                "certs": ["vpn-gateway.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "pubkey",
                                "cacerts": ["corp-ca.pem"]
                            },
                            "children": {
                                "rw-cert-users": {
                                    "local_ts": ["10.10.0.0/16", "10.20.0.0/16"],
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "trap",
                                    "dpd_action": "clear"
                                }
                            },
                            "pools": "rw-pool"
                        }
                    })
                )
            ),
            (
                "peer_to_any_certificate_plus_user_password" = (
                    summary = "Peer to Any con certificado + usuario/clave",
                    description = "Flujo end-to-end: 1) Crea CA/certificados en /api/certificates. 2) Crea un secret EAP por usuario en /api/secrets (secret_type=eap). 3) Crea la conexión con autenticación múltiple: certificado del cliente (remote.auth=pubkey) + segundo factor de usuario/clave (remote.auth2=eap-mschapv2).",
                    value = json!({
                        "name": "rw-cert-plus-password",
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "vpn.example.com",
                                "auth": "pubkey",
                                "certs": ["vpn-gateway.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "pubkey",
                                "auth2": "eap-mschapv2",
                                "eap_id": "%any",
                                "cacerts": ["corp-ca.pem"]
                            },
                            "children": {
                                "rw-cert-eap": {
                                    "local_ts": "10.50.0.0/16",
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "trap",
                                    "dpd_action": "clear"
                                }
                            },
                            "pools": "rw-pool",
                            "install_policy": true
                        }
                    })
                )
            )
        )
    ),
    responses(
        (status = 201, description = "Conexion creada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 409, description = "Conexion ya existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn create_connection_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConnectionCreateRequest>,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_upsert_handler(state, payload.name, payload.config, false, Some(lang)).await
}

#[utoipa::path(
    put,
    path = "/api/connections/{connection_name}",
    request_body(
        content = ConnectionUpsertRequest,
        examples(
            (
                "update_peer_to_peer_psk" = (
                    summary = "Actualizar Peer to Peer con PSK",
                    description = "Actualiza parámetros del túnel y reutiliza secrets existentes en /api/secrets para IDs IKE.",
                    value = json!({
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "remote_addrs": "2.2.2.2",
                            "proposals": "aes256-sha256-modp1024!",
                            "rekey_time": "86400s",
                            "local": { "id": "1.1.1.1", "auth": "psk" },
                            "remote": { "id": "2.2.2.2", "auth": "psk" },
                            "children": {
                                "net-net": {
                                    "local_ts": "10.0.0.1/32",
                                    "remote_ts": "10.1.1.1/32",
                                    "esp_proposals": "aes256-sha256!",
                                    "rekey_time": "3600s",
                                    "dpd_action": "restart",
                                    "dpd_delay": "30s",
                                    "close_action": "restart",
                                    "start_action": "start",
                                    "encap": true
                                }
                            }
                        }
                    })
                )
            ),
            (
                "update_peer_to_any_eap" = (
                    summary = "Actualizar Peer to Any con EAP",
                    description = "Los usuarios remotos y sus credenciales EAP se gestionan vía /api/secrets con secret_type = eap.",
                    value = json!({
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "1.1.1.1",
                                "auth": "pubkey",
                                "certs": ["serverCert.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "eap-mschapv2",
                                "eap_id": "%any"
                            },
                            "children": {
                                "net-any": {
                                    "local_ts": ["10.1.1.0/24", "10.2.1.0/24"],
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "trap",
                                    "dpd_action": "clear"
                                }
                            },
                            "pools": "vpn-pool",
                            "install_policy": true
                        }
                    })
                )
            ),
            (
                "update_peer_to_any_cert" = (
                    summary = "Actualizar Peer to Any con certificados",
                    description = "Usa certificados administrados por /api/certificates y adjunta certificado local con /api/connections/{connection_name}/certificate si necesitas automatizar local_id/remote_ca.",
                    value = json!({
                        "config": {
                            "version": 2,
                            "local_addrs": "%any",
                            "remote_addrs": "%any",
                            "proposals": "aes256-sha256-modp2048!",
                            "local": {
                                "id": "gw.local.example",
                                "auth": "pubkey",
                                "certs": ["gw-local.pem"],
                                "send_cert": "always"
                            },
                            "remote": {
                                "id": "%any",
                                "auth": "pubkey",
                                "cacerts": ["remote-ca.pem"]
                            },
                            "children": {
                                "default": {
                                    "local_ts": "10.30.0.0/16",
                                    "remote_ts": "0.0.0.0/0",
                                    "esp_proposals": "aes256-sha256!",
                                    "start_action": "add"
                                }
                            }
                        }
                    })
                )
            )
        )
    ),
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion actualizada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn update_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<ConnectionUpsertRequest>,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_upsert_handler(state, connection_name, payload.config, true, Some(lang)).await
}

#[utoipa::path(
    delete,
    path = "/api/connections/{connection_name}",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion eliminada", body = ConnectionCrudResponse),
        (status = 400, description = "Nombre invalido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn delete_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_delete_handler(state, connection_name, Some(lang)).await
}

#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/enable",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion habilitada", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn enable_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_set_enabled_handler(
        state,
        connection_name,
        true,
        Some(lang),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/disable",
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Conexion deshabilitada", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn disable_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::connection_set_enabled_handler(
        state,
        connection_name,
        false,
        Some(lang),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/certificate",
    request_body = ConnectionCertificateAttachRequest,
    params(("connection_name" = String, Path, description = "Nombre de la conexion")),
    responses(
        (status = 200, description = "Certificado adjuntado", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud invalida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexion o certificado no encontrado", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operacion no soportada", body = ConnectionCrudResponse)
    )
)]
pub async fn attach_certificate_to_connection_handler(
    State(state): State<crate::AppState>,
    Path(connection_name): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<ConnectionCertificateAttachRequest>,
) -> impl IntoResponse {
    let lang = crate::i18n::resolve_requested_language(&headers);
    crate::api::service::connections::attach_certificate_to_connection_handler(state, connection_name, payload, Some(lang)).await
}
