/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 */
 
mod models;
mod engine;
mod db;
mod worker;
mod config;
mod metrics;
mod logger;
mod middleware;

use axum::{routing::{delete, get, post, put}, Router, extract::{Path, State}, response::IntoResponse, middleware as axum_middleware, Json};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use tokio::process::Command;
use tower_http::cors::CorsLayer;
use tokio_rustls::rustls::{ServerConfig, pki_types::CertificateDer};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tower::Service;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipa_redoc::{Redoc, Servable};

type SharedState = Arc<RwLock<models::BifrostTopology>>;
type MetricsState = Arc<metrics::Metrics>;

#[derive(Clone)]
struct AppState {
    topology: SharedState,
    metrics: MetricsState,
    logger: Arc<logger::Logger>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct PeerControlResponse {
    peer_name: String,
    action: String,
    success: bool,
    message: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct ServiceControlResponse {
    service_name: String,
    action: String,
    success: bool,
    message: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct ConnectionUpsertRequest {
    config_body: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct ConnectionCreateRequest {
    name: String,
    config_body: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct ConnectionResponse {
    name: String,
    config_body: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct ConnectionListResponse {
    connections: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct ConnectionCrudResponse {
    name: String,
    action: String,
    success: bool,
    message: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_topology_handler,
        metrics_handler,
        peer_up_handler,
        peer_down_handler,
        strongswan_start_handler,
        strongswan_stop_handler,
        list_connections_handler,
        get_connection_handler,
        create_connection_handler,
        update_connection_handler,
        delete_connection_handler,
    ),
    components(schemas(
        models::BifrostTopology,
        models::NetworkNode,
        models::VpnEdge,
        models::NodeType,
        models::VpnStatus,
        PeerControlResponse,
        ServiceControlResponse,
        ConnectionCreateRequest,
        ConnectionUpsertRequest,
        ConnectionResponse,
        ConnectionListResponse,
        ConnectionCrudResponse
    ))
)]
struct ApiDoc;

use auto_instrument::auto_instrument;

/// Obtiene las métricas de Prometheus
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Métricas Prometheus en formato de texto")
    )
)]
#[auto_instrument]
async fn metrics_handler(
    State(state): State<AppState>
) -> impl IntoResponse {
    match state.metrics.encode_metrics() {
        Ok(data) => {
            state.logger.log_api_request("GET", "/metrics", 200, 0);
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                data,
            )
        }
        Err(_) => {
            state.logger.log_api_error("GET", "/metrics", 500, "Error encoding metrics");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                "Error encoding metrics".to_string(),
            )
        }
    }
}

#[tokio::main]
async fn main() {
    // 1. Cargar configuración
    let settings = config::Settings::new().expect("No se pudo cargar config.toml");

    // 2. Inicializar logger asíncrono global (spawn background writer)
    logger::init_async_logger(
        settings.logging.service_access_log.as_deref(),
        settings.logging.service_error_log.as_deref(),
        settings.logging.worker_log.as_deref(),
        settings.logging.use_journalctl.unwrap_or(true),
        settings.logging.channel_capacity.unwrap_or(1000),
        settings.logging.rotate_size_mb.unwrap_or(10) * 1024 * 1024,
    );

    // Forward tracing events into our async logger pipeline
    logger::init_tracing_forwarder();

    // 3. Crear logger del servicio con rutas personalizadas (usa el canal asíncrono)
    let service_logger = Arc::new(logger::Logger::with_custom_paths(
        settings.logging.service_level,
        "service",
        settings.logging.service_access_log.as_deref(),
        settings.logging.service_error_log.as_deref(),
        None,
    ));

    service_logger.info("Bifröst-Gate iniciando...");
    service_logger.info(&format!("Nivel de log del servicio: {}", settings.logging.service_level));
    service_logger.info(&format!("Nivel de log de workers: {}", settings.logging.worker_level));
    if let Some(ref path) = settings.logging.service_access_log {
        service_logger.info(&format!("Access log: {}", path));
    }
    if let Some(ref path) = settings.logging.service_error_log {
        service_logger.info(&format!("Error log: {}", path));
    }
    service_logger.info(&format!("API key auth enabled: {}", settings.auth.enabled));

    // 3. Inicializar componentes
    let pool = db::init_db().await;

    if settings.auth.enabled {
        let bootstrap_user = settings
            .auth
            .bootstrap_user
            .clone()
            .unwrap_or_else(|| "admin".to_string());

        if let Some(ref bootstrap_key) = settings.auth.bootstrap_api_key {
            match db::seed_api_key_if_missing(&pool, &bootstrap_user, bootstrap_key).await {
                Ok(true) => service_logger.info("API key bootstrap creada en DB"),
                Ok(false) => service_logger.info("Bootstrap omitido: ya existen API keys activas"),
                Err(err) => service_logger.error(&format!("Error sembrando API key bootstrap: {}", err)),
            }
        }

        match db::count_active_api_keys(&pool).await {
            Ok(0) => {
                service_logger.error("Auth habilitada pero no hay API keys activas en DB. Configura [auth].bootstrap_api_key o crea una key por otro medio.");
            }
            Ok(count) => {
                service_logger.info(&format!("API keys activas en DB: {}", count));
            }
            Err(err) => {
                service_logger.error(&format!("No se pudo contar API keys activas: {}", err));
            }
        }
    }

    let current_topology = Arc::new(RwLock::new(engine::generate_mock_topology()));
    
    // 4. Inicializar métricas Prometheus
    let metrics = Arc::new(metrics::Metrics::new()
        .expect("Failed to initialize Prometheus metrics"));
    
    // 5. Iniciar el Worker con su propio logger
    let worker_state = Arc::clone(&current_topology);
    let worker_pool = pool.clone();
    let worker_logger = Arc::new(logger::Logger::with_custom_paths(
        settings.logging.worker_level,
        "worker",
        None,
        None,
        settings.logging.worker_log.as_deref(),
    ));
    let worker_service_logger = Arc::clone(&service_logger);
    
    tokio::spawn(async move {
        worker_service_logger.info("Iniciando worker Heimdall...");
        worker::start_heimdall_worker_with_logger(worker_state, worker_pool, worker_logger).await;
    });

    // 6. Configurar la API
    let cors = CorsLayer::permissive();
    let app_state = AppState {
        topology: Arc::clone(&current_topology),
        metrics: metrics.clone(),
        logger: Arc::clone(&service_logger),
    };
    
    // Middleware de logging
    let logging_middleware_state = middleware::LoggingMiddlewareState {
        logger: Arc::clone(&service_logger),
    };

    let auth_header_name = settings
        .auth
        .header_name
        .clone()
        .unwrap_or_else(|| "x-api-key".to_string());

    let api_key_middleware_state = middleware::ApiKeyMiddlewareState {
        logger: Arc::clone(&service_logger),
        enabled: settings.auth.enabled,
        pool: pool.clone(),
        header_name: auth_header_name.clone(),
    };

    let protected_routes = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/api/topology", get(get_topology_handler))
        .route("/api/peers/:peer_name/up", post(peer_up_handler))
        .route("/api/peers/:peer_name/down", post(peer_down_handler))
        .route("/api/strongswan/start", post(strongswan_start_handler))
        .route("/api/strongswan/stop", post(strongswan_stop_handler))
        .route("/api/connections", get(list_connections_handler))
        .route("/api/connections", post(create_connection_handler))
        .route("/api/connections/:connection_name", get(get_connection_handler))
        .route("/api/connections/:connection_name", put(update_connection_handler))
        .route("/api/connections/:connection_name", delete(delete_connection_handler))
        .layer(
            axum_middleware::from_fn_with_state(
                api_key_middleware_state,
                middleware::api_key_middleware,
            )
        )
        .with_state(app_state);
    
    let docs_auth_middleware_state = middleware::DocsAuthMiddlewareState {
        logger: Arc::clone(&service_logger),
        pool: pool.clone(),
    };

    let docs_routes = Router::new()
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/api/tryme", ApiDoc::openapi()))
        .layer(
            axum_middleware::from_fn_with_state(
                docs_auth_middleware_state,
                middleware::docs_basic_auth_middleware,
            )
        );

    let app = Router::new()
        .merge(docs_routes)
        .merge(protected_routes)
        .layer(
            axum_middleware::from_fn_with_state(
                logging_middleware_state,
                middleware::logging_middleware,
            )
        )
        .layer(cors);

    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port)
        .parse()
        .expect("Dirección de servidor inválida");

    // 7. Lógica de encendido
    if settings.tls.enabled {
        let cert_file = File::open(&settings.tls.cert_path).expect("No cert.pem");
        let key_file = File::open(&settings.tls.key_path).expect("No key.pem");
        let mut cert_reader = BufReader::new(cert_file);
        let mut key_reader = BufReader::new(key_file);

        let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>().expect("Error en certificados");
        
        let key = rustls_pemfile::private_key(&mut key_reader)
            .expect("Error en llave")
            .expect("No se encontró llave");

        let mut tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("Configuración TLS inválida");
        
        tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        
        println!("🔐 Bifröst-Gate (TLS Nativo) en https://{}", addr);
        println!("📊 Métricas Prometheus: https://{}/metrics", addr);
        println!("📚 Swagger UI: https://{}/api/docs", addr);
        println!("📖 ReDoc API: https://{}/api/tryme", addr);
        if settings.auth.enabled {
            println!("🔑 API key requerida en header '{}'", auth_header_name);
        }
        
        service_logger.info(&format!("🔐 Servidor TLS iniciado en https://{}", addr));

        loop {
            let (stream, _remote_addr) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let app = app.clone();

            tokio::spawn(async move {
                if let Ok(tls_stream) = acceptor.accept(stream).await {
                    let io = TokioIo::new(tls_stream);
                    
                    let service = service_fn(move |req| {
                        let mut app = app.clone();
                        app.call(req)
                    });

                    if let Err(err) = Builder::new(TokioExecutor::new())
                        .serve_connection(io, service)
                        .await 
                    {
                        eprintln!("Error en conexión TLS: {:?}", err);
                    }
                }
            });
        }
    } else {
        println!("🚀 Bifröst-Gate (Modo inseguro) en http://{}", addr);
        println!("📊 Métricas Prometheus: http://{}/metrics", addr);
        println!("📚 Swagger UI: http://{}/api/docs", addr);
        println!("📖 ReDoc API: http://{}/api/tryme", addr);
        if settings.auth.enabled {
            println!("🔑 API key requerida en header '{}'", auth_header_name);
        }
        
        service_logger.info(&format!("🚀 Servidor HTTP iniciado en http://{}", addr));
        
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

/// Obtiene la topología actual de Bifröst
#[utoipa::path(
    get,
    path = "/api/topology",
    responses(
        (status = 200, description = "Topología obtenida exitosamente", body = BifrostTopology)
    )
)]
async fn get_topology_handler(
    State(state): State<AppState>,
) -> axum::Json<models::BifrostTopology> {
    let topo = state.topology.read().unwrap();
    
    // Incrementar métricas
    state.metrics.topology_requests.inc();
    state.metrics.topology_nodes_count.set(topo.nodes.len() as f64);
    state.metrics.topology_edges_count.set(topo.edges.len() as f64);
    
    axum::Json(topo.clone())
}

/// Levanta (inicia) una conexión IKE de un peer de StrongSwan.
#[utoipa::path(
    post,
    path = "/api/peers/{peer_name}/up",
    params(
        ("peer_name" = String, Path, description = "Nombre de la conexión/peer en StrongSwan")
    ),
    responses(
        (status = 200, description = "Peer levantado", body = PeerControlResponse),
        (status = 400, description = "Error al ejecutar comando", body = PeerControlResponse),
        (status = 500, description = "Error interno", body = PeerControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = PeerControlResponse)
    )
)]
async fn peer_up_handler(
    State(state): State<AppState>,
    Path(peer_name): Path<String>,
) -> impl IntoResponse {
    peer_control_handler(state, peer_name, true).await
}

/// Baja (termina) una conexión IKE de un peer de StrongSwan.
#[utoipa::path(
    post,
    path = "/api/peers/{peer_name}/down",
    params(
        ("peer_name" = String, Path, description = "Nombre de la conexión/peer en StrongSwan")
    ),
    responses(
        (status = 200, description = "Peer bajado", body = PeerControlResponse),
        (status = 400, description = "Error al ejecutar comando", body = PeerControlResponse),
        (status = 500, description = "Error interno", body = PeerControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = PeerControlResponse)
    )
)]
async fn peer_down_handler(
    State(state): State<AppState>,
    Path(peer_name): Path<String>,
) -> impl IntoResponse {
    peer_control_handler(state, peer_name, false).await
}

async fn peer_control_handler(
    state: AppState,
    peer_name: String,
    bring_up: bool,
) -> impl IntoResponse {
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
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        );
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("swanctl");
        if bring_up {
            cmd.arg("--initiate").arg("--ike").arg(&peer_name);
        } else {
            cmd.arg("--terminate").arg("--ike").arg(&peer_name);
        }

        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    state.logger.info(&format!("Peer '{}' action '{}' ejecutada", peer_name, action));
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let message = if stdout.is_empty() {
                        format!("Peer '{}' {} exitosamente", peer_name, action)
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
                    state.logger.error(&format!(
                        "Fallo al ejecutar action '{}' para peer '{}': {}",
                        action, peer_name, stderr
                    ));
                    (
                        StatusCode::BAD_REQUEST,
                        Json(PeerControlResponse {
                            peer_name,
                            action: action.to_string(),
                            success: false,
                            message: if stderr.is_empty() {
                                "swanctl devolvió error sin detalle".to_string()
                            } else {
                                stderr
                            },
                        }),
                    )
                }
            }
            Err(err) => {
                state.logger.error(&format!(
                    "No se pudo ejecutar swanctl para peer '{}' action '{}': {}",
                    peer_name, action, err
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

/// Inicia el servicio de StrongSwan en el host.
#[utoipa::path(
    post,
    path = "/api/strongswan/start",
    responses(
        (status = 200, description = "Servicio StrongSwan iniciado", body = ServiceControlResponse),
        (status = 400, description = "No se pudo iniciar", body = ServiceControlResponse),
        (status = 500, description = "Error interno", body = ServiceControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = ServiceControlResponse)
    )
)]
async fn strongswan_start_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    strongswan_control_handler(state, "start").await
}

/// Detiene el servicio de StrongSwan en el host.
#[utoipa::path(
    post,
    path = "/api/strongswan/stop",
    responses(
        (status = 200, description = "Servicio StrongSwan detenido", body = ServiceControlResponse),
        (status = 400, description = "No se pudo detener", body = ServiceControlResponse),
        (status = 500, description = "Error interno", body = ServiceControlResponse),
        (status = 501, description = "Operación no soportada en este OS", body = ServiceControlResponse)
    )
)]
async fn strongswan_stop_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    strongswan_control_handler(state, "stop").await
}

async fn strongswan_control_handler(
    state: AppState,
    action: &str,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ServiceControlResponse {
                service_name: "strongswan".to_string(),
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con systemd".to_string(),
            }),
        );
    }

    #[cfg(target_os = "linux")]
    {
        let unit = match detect_strongswan_unit().await {
            Ok(Some(unit)) => unit,
            Ok(None) => {
                let msg = "No se encontró unidad systemd de StrongSwan (strongswan, strongswan-starter o charon-systemd)".to_string();
                state.logger.error(&msg);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ServiceControlResponse {
                        service_name: "strongswan".to_string(),
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                );
            }
            Err(err) => {
                let msg = format!("Error detectando unidad de StrongSwan: {}", err);
                state.logger.error(&msg);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ServiceControlResponse {
                        service_name: "strongswan".to_string(),
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                );
            }
        };

        match Command::new("systemctl")
            .arg(action)
            .arg(&unit)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    let msg = format!("Servicio '{}' {} exitosamente", unit, action);
                    state.logger.info(&msg);
                    (
                        StatusCode::OK,
                        Json(ServiceControlResponse {
                            service_name: unit,
                            action: action.to_string(),
                            success: true,
                            message: msg,
                        }),
                    )
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let msg = if stderr.is_empty() {
                        format!("systemctl devolvió error al ejecutar '{} {}'", action, unit)
                    } else {
                        stderr
                    };
                    state.logger.error(&msg);
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ServiceControlResponse {
                            service_name: unit,
                            action: action.to_string(),
                            success: false,
                            message: msg,
                        }),
                    )
                }
            }
            Err(err) => {
                let msg = format!("No se pudo ejecutar systemctl: {}", err);
                state.logger.error(&msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ServiceControlResponse {
                        service_name: unit,
                        action: action.to_string(),
                        success: false,
                        message: msg,
                    }),
                )
            }
        }
    }
}

/// Lista conexiones administradas por Bifröst en /etc/swanctl/conf.d.
#[utoipa::path(
    get,
    path = "/api/connections",
    responses(
        (status = 200, description = "Listado de conexiones", body = ConnectionListResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn list_connections_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: String::new(),
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_connections().await {
            Ok(connections) => (StatusCode::OK, Json(ConnectionListResponse { connections })).into_response(),
            Err(err) => {
                state.logger.error(&format!("Error listando conexiones: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ConnectionCrudResponse {
                        name: String::new(),
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando conexiones: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// Obtiene la configuración de una conexión administrada por Bifröst.
#[utoipa::path(
    get,
    path = "/api/connections/{connection_name}",
    params(
        ("connection_name" = String, Path, description = "Nombre de la conexión")
    ),
    responses(
        (status = 200, description = "Conexión encontrada", body = ConnectionResponse),
        (status = 400, description = "Nombre inválido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexión no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn get_connection_handler(
    State(state): State<AppState>,
    Path(connection_name): Path<String>,
) -> impl IntoResponse {
    connection_read_handler(state, connection_name).await
}

/// Crea una conexión StrongSwan y recarga conexiones.
#[utoipa::path(
    post,
    path = "/api/connections",
    request_body = ConnectionCreateRequest,
    responses(
        (status = 201, description = "Conexión creada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud inválida", body = ConnectionCrudResponse),
        (status = 409, description = "Conexión ya existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn create_connection_handler(
    State(state): State<AppState>,
    Json(payload): Json<ConnectionCreateRequest>,
) -> impl IntoResponse {
    connection_upsert_handler(state, payload.name, payload.config_body, false).await
}

/// Actualiza una conexión StrongSwan y recarga conexiones.
#[utoipa::path(
    put,
    path = "/api/connections/{connection_name}",
    request_body = ConnectionUpsertRequest,
    params(
        ("connection_name" = String, Path, description = "Nombre de la conexión")
    ),
    responses(
        (status = 200, description = "Conexión actualizada", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud inválida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexión no existe", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn update_connection_handler(
    State(state): State<AppState>,
    Path(connection_name): Path<String>,
    Json(payload): Json<ConnectionUpsertRequest>,
) -> impl IntoResponse {
    connection_upsert_handler(state, connection_name, payload.config_body, true).await
}

/// Elimina una conexión StrongSwan y recarga conexiones.
#[utoipa::path(
    delete,
    path = "/api/connections/{connection_name}",
    params(
        ("connection_name" = String, Path, description = "Nombre de la conexión")
    ),
    responses(
        (status = 200, description = "Conexión eliminada", body = ConnectionCrudResponse),
        (status = 400, description = "Nombre inválido", body = ConnectionCrudResponse),
        (status = 404, description = "Conexión no encontrada", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn delete_connection_handler(
    State(state): State<AppState>,
    Path(connection_name): Path<String>,
) -> impl IntoResponse {
    connection_delete_handler(state, connection_name).await
}

async fn connection_read_handler(
    state: AppState,
    connection_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_connection_name(&connection_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: connection_name,
                        action: "read".to_string(),
                        success: false,
                        message: "connection_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = connection_file_path(&name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let config_body = extract_connection_body(&name, &content).unwrap_or(content);
                (StatusCode::OK, Json(ConnectionResponse { name, config_body })).into_response()
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Conexión no encontrada".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error leyendo conexión: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: format!("Error leyendo conexión: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

async fn connection_upsert_handler(
    state: AppState,
    connection_name: String,
    config_body: String,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_connection_name(&connection_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: connection_name,
                        action: action.to_string(),
                        success: false,
                        message: "connection_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        if config_body.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ConnectionCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: "config_body es requerido".to_string(),
                }),
            )
                .into_response();
        }

        let path = connection_file_path(&name);
        let exists = tokio::fs::metadata(&path).await.is_ok();

        if !update && exists {
            return (
                StatusCode::CONFLICT,
                Json(ConnectionCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: "La conexión ya existe".to_string(),
                }),
            )
                .into_response();
        }

        if update && !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(ConnectionCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: "La conexión no existe".to_string(),
                }),
            )
                .into_response();
        }

        let conf_text = build_connection_conf(&name, &config_body);
        if let Err(err) = tokio::fs::write(&path, conf_text).await {
            state.logger.error(&format!("Error escribiendo conexión '{}': {}", name, err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ConnectionCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: format!("Error escribiendo archivo: {}", err),
                }),
            )
                .into_response();
        }

        match reload_swanctl_conns().await {
            Ok(()) => {
                let code = if update { StatusCode::OK } else { StatusCode::CREATED };
                (
                    code,
                    Json(ConnectionCrudResponse {
                        name,
                        action: action.to_string(),
                        success: true,
                        message: "Conexión guardada y recargada".to_string(),
                    }),
                )
                    .into_response()
            }
            Err(err) => {
                state.logger.error(&format!("Error recargando conexiones: {}", err));
                (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name,
                        action: action.to_string(),
                        success: false,
                        message: format!("Conexión guardada pero falló reload: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn connection_delete_handler(
    state: AppState,
    connection_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_connection_name(&connection_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: connection_name,
                        action: "delete".to_string(),
                        success: false,
                        message: "connection_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = connection_file_path(&name);
        match tokio::fs::remove_file(&path).await {
            Ok(_) => match reload_swanctl_conns().await {
                Ok(()) => (
                    StatusCode::OK,
                    Json(ConnectionCrudResponse {
                        name,
                        action: "delete".to_string(),
                        success: true,
                        message: "Conexión eliminada y recargada".to_string(),
                    }),
                )
                    .into_response(),
                Err(err) => {
                    state.logger.error(&format!("Conexión eliminada, pero falló reload: {}", err));
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: format!("Conexión eliminada pero falló reload: {}", err),
                        }),
                    )
                        .into_response()
                }
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: "Conexión no encontrada".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error eliminando conexión: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ConnectionCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: format!("Error eliminando conexión: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn connection_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/conf.d/bifrost-{}.conf", name))
}

#[cfg(target_os = "linux")]
fn sanitize_connection_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn build_connection_conf(name: &str, config_body: &str) -> String {
    let mut out = String::from("connections {\n");
    out.push_str(&format!("  {} {{\n", name));
    for line in config_body.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(target_os = "linux")]
fn extract_connection_body(name: &str, content: &str) -> Option<String> {
    let needle = format!("{} {{", name);
    let start = content.find(&needle)?;
    let after = &content[start + needle.len()..];
    let end = after.rfind('}')?;
    let body = after[..end].trim_matches('\n');
    let cleaned = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    Some(cleaned.trim().to_string())
}

#[cfg(target_os = "linux")]
async fn reload_swanctl_conns() -> Result<(), String> {
    match Command::new("swanctl").arg("--load-conns").output().await {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    Err("swanctl --load-conns falló sin detalle".to_string())
                } else {
                    Err(stderr)
                }
            }
        }
        Err(err) => Err(format!("No se pudo ejecutar swanctl --load-conns: {}", err)),
    }
}

#[cfg(target_os = "linux")]
async fn list_managed_connections() -> Result<Vec<String>, std::io::Error> {
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir("/etc/swanctl/conf.d").await?;
    while let Some(entry) = dir.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with("bifrost-") || !file_name.ends_with(".conf") {
            continue;
        }

        let name = file_name
            .trim_start_matches("bifrost-")
            .trim_end_matches(".conf")
            .to_string();
        names.push(name);
    }
    names.sort();
    Ok(names)
}

#[cfg(target_os = "linux")]
async fn detect_strongswan_unit() -> Result<Option<String>, std::io::Error> {
    let candidates = ["strongswan", "strongswan-starter", "charon-systemd"];

    for unit in candidates {
        let output = Command::new("systemctl")
            .arg("show")
            .arg(unit)
            .arg("--property")
            .arg("LoadState")
            .arg("--value")
            .output()
            .await?;

        if !output.status.success() {
            continue;
        }

        let load_state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if load_state != "not-found" && !load_state.is_empty() {
            return Ok(Some(unit.to_string()));
        }
    }

    Ok(None)
}


