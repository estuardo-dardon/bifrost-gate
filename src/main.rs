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
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;
use std::fs::File;
use std::io::BufReader;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;
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
    config: Value,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct ConnectionCreateRequest {
    name: String,
    config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct ConnectionResponse {
    name: String,
    config: String,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
enum SecretType {
    Eap,
    Xauth,
    Ntlm,
    Ike,
    Ppk,
    Private,
    Rsa,
    Ecdsa,
    Pkcs8,
    Pkcs12,
    Token,
}

impl SecretType {
    fn as_str(self) -> &'static str {
        match self {
            SecretType::Eap => "eap",
            SecretType::Xauth => "xauth",
            SecretType::Ntlm => "ntlm",
            SecretType::Ike => "ike",
            SecretType::Ppk => "ppk",
            SecretType::Private => "private",
            SecretType::Rsa => "rsa",
            SecretType::Ecdsa => "ecdsa",
            SecretType::Pkcs8 => "pkcs8",
            SecretType::Pkcs12 => "pkcs12",
            SecretType::Token => "token",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "eap" => Some(SecretType::Eap),
            "xauth" => Some(SecretType::Xauth),
            "ntlm" => Some(SecretType::Ntlm),
            "ike" => Some(SecretType::Ike),
            "ppk" => Some(SecretType::Ppk),
            "private" => Some(SecretType::Private),
            "rsa" => Some(SecretType::Rsa),
            "ecdsa" => Some(SecretType::Ecdsa),
            "pkcs8" => Some(SecretType::Pkcs8),
            "pkcs12" => Some(SecretType::Pkcs12),
            "token" => Some(SecretType::Token),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct SecretUpsertRequest {
    secret_type: SecretType,
    config: Value,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct SecretCreateRequest {
    name: String,
    secret_type: SecretType,
    config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct SecretResponse {
    name: String,
    secret_type: SecretType,
    config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct SecretListResponse {
    secrets: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct SecretCrudResponse {
    name: String,
    action: String,
    success: bool,
    message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
enum CertificateKind {
    Ca,
    User,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct CaCertificateCreateRequest {
    name: String,
    common_name: String,
    organization: Option<String>,
    country: Option<String>,
    days: Option<u32>,
    key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct CaCertificateUpsertRequest {
    common_name: String,
    organization: Option<String>,
    country: Option<String>,
    days: Option<u32>,
    key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct UserCertificateCreateRequest {
    name: String,
    ca_name: String,
    identity: String,
    san: Option<Vec<String>>,
    days: Option<u32>,
    key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct UserCertificateUpsertRequest {
    ca_name: String,
    identity: String,
    san: Option<Vec<String>>,
    days: Option<u32>,
    key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct ConnectionCertificateAttachRequest {
    certificate_name: String,
    local_id: Option<String>,
    remote_ca_name: Option<String>,
    set_remote_auth_pubkey: Option<bool>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct CertificateListResponse {
    certificates: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct CertificateDetailsResponse {
    name: String,
    kind: CertificateKind,
    certificate_path: String,
    private_key_path: Option<String>,
    subject: Option<String>,
    issuer: Option<String>,
    not_after: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct CertificateCrudResponse {
    name: String,
    kind: CertificateKind,
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
        list_secrets_handler,
        get_secret_handler,
        create_secret_handler,
        update_secret_handler,
        delete_secret_handler,
        list_ca_certificates_handler,
        get_ca_certificate_handler,
        create_ca_certificate_handler,
        update_ca_certificate_handler,
        delete_ca_certificate_handler,
        list_user_certificates_handler,
        get_user_certificate_handler,
        create_user_certificate_handler,
        update_user_certificate_handler,
        delete_user_certificate_handler,
        attach_certificate_to_connection_handler,
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
        ConnectionCrudResponse,
        SecretType,
        SecretCreateRequest,
        SecretUpsertRequest,
        SecretResponse,
        SecretListResponse,
        SecretCrudResponse,
        CertificateKind,
        CaCertificateCreateRequest,
        CaCertificateUpsertRequest,
        UserCertificateCreateRequest,
        UserCertificateUpsertRequest,
        ConnectionCertificateAttachRequest,
        CertificateListResponse,
        CertificateDetailsResponse,
        CertificateCrudResponse
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
        .route(
            "/api/connections/:connection_name/certificate",
            post(attach_certificate_to_connection_handler),
        )
        .route("/api/secrets", get(list_secrets_handler))
        .route("/api/secrets", post(create_secret_handler))
        .route("/api/secrets/:secret_name", get(get_secret_handler))
        .route("/api/secrets/:secret_name", put(update_secret_handler))
        .route("/api/secrets/:secret_name", delete(delete_secret_handler))
        .route("/api/certificates/ca", get(list_ca_certificates_handler))
        .route("/api/certificates/ca", post(create_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", get(get_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", put(update_ca_certificate_handler))
        .route("/api/certificates/ca/:ca_name", delete(delete_ca_certificate_handler))
        .route("/api/certificates/user", get(list_user_certificates_handler))
        .route("/api/certificates/user", post(create_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", get(get_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", put(update_user_certificate_handler))
        .route("/api/certificates/user/:cert_name", delete(delete_user_certificate_handler))
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
        if bring_up {
            // Fase 1: Levantar IKE
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
                                    "swanctl falló al iniciar IKE sin detalle".to_string()
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

            // Esperar a que se establezca la Fase 1 antes de levantar Fase 2
            sleep(Duration::from_millis(500)).await;

            // Fase 2: Levantar CHILD SA (IPSec)
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
                                    "swanctl falló al iniciar CHILD SA sin detalle".to_string()
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
            // Bajar: Terminar IKE (esto también termina las CHILD SAs)
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
                        state.logger.error(&format!(
                            "Fallo al bajar peer '{}': {}",
                            peer_name, stderr
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
    connection_upsert_handler(state, payload.name, payload.config, false).await
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
    connection_upsert_handler(state, connection_name, payload.config, true).await
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

/// Lista secrets administrados por Bifröst en /etc/swanctl/conf.d.
#[utoipa::path(
    get,
    path = "/api/secrets",
    responses(
        (status = 200, description = "Listado de secrets", body = SecretListResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operación no soportada", body = SecretCrudResponse)
    )
)]
async fn list_secrets_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
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
        match list_managed_secrets().await {
            Ok(secrets) => (StatusCode::OK, Json(SecretListResponse { secrets })).into_response(),
            Err(err) => {
                state.logger.error(&format!("Error listando secrets: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(SecretCrudResponse {
                        name: String::new(),
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando secrets: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// Obtiene un secret administrado por Bifröst (con valores sensibles enmascarados).
#[utoipa::path(
    get,
    path = "/api/secrets/{secret_name}",
    params(
        ("secret_name" = String, Path, description = "Nombre del secret")
    ),
    responses(
        (status = 200, description = "Secret encontrado", body = SecretResponse),
        (status = 400, description = "Nombre inválido", body = SecretCrudResponse),
        (status = 404, description = "Secret no encontrado", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operación no soportada", body = SecretCrudResponse)
    )
)]
async fn get_secret_handler(
    State(state): State<AppState>,
    Path(secret_name): Path<String>,
) -> impl IntoResponse {
    secret_read_handler(state, secret_name).await
}

/// Crea un secret StrongSwan y recarga credenciales.
#[utoipa::path(
    post,
    path = "/api/secrets",
    request_body = SecretCreateRequest,
    responses(
        (status = 201, description = "Secret creado", body = SecretCrudResponse),
        (status = 400, description = "Solicitud inválida", body = SecretCrudResponse),
        (status = 409, description = "Secret ya existe", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operación no soportada", body = SecretCrudResponse)
    )
)]
async fn create_secret_handler(
    State(state): State<AppState>,
    Json(payload): Json<SecretCreateRequest>,
) -> impl IntoResponse {
    secret_upsert_handler(
        state,
        payload.name,
        payload.secret_type,
        payload.config,
        false,
    )
    .await
}

/// Actualiza un secret StrongSwan y recarga credenciales.
#[utoipa::path(
    put,
    path = "/api/secrets/{secret_name}",
    request_body = SecretUpsertRequest,
    params(
        ("secret_name" = String, Path, description = "Nombre del secret")
    ),
    responses(
        (status = 200, description = "Secret actualizado", body = SecretCrudResponse),
        (status = 400, description = "Solicitud inválida", body = SecretCrudResponse),
        (status = 404, description = "Secret no existe", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operación no soportada", body = SecretCrudResponse)
    )
)]
async fn update_secret_handler(
    State(state): State<AppState>,
    Path(secret_name): Path<String>,
    Json(payload): Json<SecretUpsertRequest>,
) -> impl IntoResponse {
    secret_upsert_handler(state, secret_name, payload.secret_type, payload.config, true).await
}

/// Elimina un secret StrongSwan y recarga credenciales.
#[utoipa::path(
    delete,
    path = "/api/secrets/{secret_name}",
    params(
        ("secret_name" = String, Path, description = "Nombre del secret")
    ),
    responses(
        (status = 200, description = "Secret eliminado", body = SecretCrudResponse),
        (status = 400, description = "Nombre inválido", body = SecretCrudResponse),
        (status = 404, description = "Secret no encontrado", body = SecretCrudResponse),
        (status = 500, description = "Error interno", body = SecretCrudResponse),
        (status = 501, description = "Operación no soportada", body = SecretCrudResponse)
    )
)]
async fn delete_secret_handler(
    State(state): State<AppState>,
    Path(secret_name): Path<String>,
) -> impl IntoResponse {
    secret_delete_handler(state, secret_name).await
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
                let config = extract_connection_body(&name, &content).unwrap_or(content);
                (StatusCode::OK, Json(ConnectionResponse { name, config })).into_response()
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
    config: Value,
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

        let config_body = match render_connection_body_from_json(&config) {
            Ok(body) => body,
            Err(message) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name,
                        action: action.to_string(),
                        success: false,
                        message,
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
                    message: "config es requerido".to_string(),
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

#[cfg(target_os = "linux")]
fn render_connection_body_from_json(config: &Value) -> Result<String, String> {
    let object = config
        .as_object()
        .ok_or_else(|| "config debe ser un objeto JSON".to_string())?;

    if object.is_empty() {
        return Ok(String::new());
    }

    let mut lines = Vec::new();
    render_connection_entries(object, 0, &mut lines)?;
    Ok(lines.join("\n"))
}

#[cfg(target_os = "linux")]
fn render_connection_entries(
    entries: &serde_json::Map<String, Value>,
    indent: usize,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let prefix = " ".repeat(indent);

    for (key, value) in entries {
        if key.trim().is_empty() {
            return Err("config contiene una clave vacía".to_string());
        }

        match value {
            Value::Object(obj) => {
                out.push(format!("{}{} {{", prefix, key));
                render_connection_entries(obj, indent + 2, out)?;
                out.push(format!("{}}}", prefix));
            }
            Value::Array(items) => {
                if items.is_empty() {
                    return Err(format!("config.{} no puede ser una lista vacía", key));
                }

                let mut rendered = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::String(s) => rendered.push(s.clone()),
                        Value::Number(n) => rendered.push(n.to_string()),
                        Value::Bool(b) => rendered.push(b.to_string()),
                        _ => {
                            return Err(format!(
                                "config.{} solo permite strings, numeros o booleanos en listas",
                                key
                            ))
                        }
                    }
                }

                out.push(format!("{}{} = {}", prefix, key, rendered.join(", ")));
            }
            Value::String(s) => out.push(format!("{}{} = {}", prefix, key, s)),
            Value::Number(n) => out.push(format!("{}{} = {}", prefix, key, n)),
            Value::Bool(b) => out.push(format!("{}{} = {}", prefix, key, b)),
            Value::Null => {
                return Err(format!("config.{} no puede ser null", key));
            }
        }
    }

    Ok(())
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

async fn secret_read_handler(
    state: AppState,
    secret_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_secret_name(&secret_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: secret_name,
                        action: "read".to_string(),
                        success: false,
                        message: "secret_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = secret_file_path(&name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => match parse_secret_config_for_response(&content) {
                Ok((secret_type, config)) => {
                    (StatusCode::OK, Json(SecretResponse { name, secret_type, config })).into_response()
                }
                Err(err) => {
                    state.logger.error(&format!("Error parseando secret '{}': {}", name, err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Archivo de secret inválido".to_string(),
                        }),
                    )
                        .into_response()
                }
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: "Secret no encontrado".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error leyendo secret: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            name,
                            action: "read".to_string(),
                            success: false,
                            message: format!("Error leyendo secret: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

async fn secret_upsert_handler(
    state: AppState,
    secret_name: String,
    secret_type: SecretType,
    config: Value,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_secret_name(&secret_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: secret_name,
                        action: action.to_string(),
                        success: false,
                        message: "secret_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let config_lines = match validate_and_render_secret_config(secret_type, &config) {
            Ok(lines) => lines,
            Err(message) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name,
                        action: action.to_string(),
                        success: false,
                        message,
                    }),
                )
                    .into_response()
            }
        };

        let path = secret_file_path(&name);
        let exists = tokio::fs::metadata(&path).await.is_ok();

        if !update && exists {
            return (
                StatusCode::CONFLICT,
                Json(SecretCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: "El secret ya existe".to_string(),
                }),
            )
                .into_response();
        }

        if update && !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(SecretCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: "El secret no existe".to_string(),
                }),
            )
                .into_response();
        }

        let conf_text = build_secret_conf(&name, secret_type, &config_lines);
        if let Err(err) = tokio::fs::write(&path, conf_text).await {
            state.logger.error(&format!("Error escribiendo secret '{}': {}", name, err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SecretCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: format!("Error escribiendo archivo: {}", err),
                }),
            )
                .into_response();
        }

        if let Err(err) = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).await {
            state.logger.error(&format!("Error ajustando permisos del secret '{}': {}", name, err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SecretCrudResponse {
                    name,
                    action: action.to_string(),
                    success: false,
                    message: format!("Error asegurando permisos del archivo: {}", err),
                }),
            )
                .into_response();
        }

        match reload_swanctl_creds().await {
            Ok(()) => {
                let code = if update { StatusCode::OK } else { StatusCode::CREATED };
                (
                    code,
                    Json(SecretCrudResponse {
                        name,
                        action: action.to_string(),
                        success: true,
                        message: "Secret guardado y credenciales recargadas".to_string(),
                    }),
                )
                    .into_response()
            }
            Err(err) => {
                state.logger.error(&format!("Error recargando credenciales: {}", err));
                (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name,
                        action: action.to_string(),
                        success: false,
                        message: format!("Secret guardado pero falló load-creds: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn secret_delete_handler(
    state: AppState,
    secret_name: String,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(SecretCrudResponse {
                name: secret_name,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_secret_name(&secret_name) {
            Some(value) => value,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(SecretCrudResponse {
                        name: secret_name,
                        action: "delete".to_string(),
                        success: false,
                        message: "secret_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let path = secret_file_path(&name);
        match tokio::fs::remove_file(&path).await {
            Ok(_) => match reload_swanctl_creds().await {
                Ok(()) => (
                    StatusCode::OK,
                    Json(SecretCrudResponse {
                        name,
                        action: "delete".to_string(),
                        success: true,
                        message: "Secret eliminado y credenciales recargadas".to_string(),
                    }),
                )
                    .into_response(),
                Err(err) => {
                    state.logger.error(&format!("Secret eliminado, pero falló load-creds: {}", err));
                    (
                        StatusCode::BAD_REQUEST,
                        Json(SecretCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: format!("Secret eliminado pero falló load-creds: {}", err),
                        }),
                    )
                        .into_response()
                }
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    (
                        StatusCode::NOT_FOUND,
                        Json(SecretCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: "Secret no encontrado".to_string(),
                        }),
                    )
                        .into_response()
                } else {
                    state.logger.error(&format!("Error eliminando secret: {}", err));
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(SecretCrudResponse {
                            name,
                            action: "delete".to_string(),
                            success: false,
                            message: format!("Error eliminando secret: {}", err),
                        }),
                    )
                        .into_response()
                }
            }
        }
    }
}

/// Lista certificados CA administrados por Bifröst.
#[utoipa::path(
    get,
    path = "/api/certificates/ca",
    responses(
        (status = 200, description = "Listado de CA", body = CertificateListResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn list_ca_certificates_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: String::new(),
                kind: CertificateKind::Ca,
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_certificates(CertificateKind::Ca).await {
            Ok(certificates) => {
                (StatusCode::OK, Json(CertificateListResponse { certificates })).into_response()
            }
            Err(err) => {
                state.logger.error(&format!("Error listando CA: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name: String::new(),
                        kind: CertificateKind::Ca,
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando CA: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// Obtiene detalles de un certificado CA.
#[utoipa::path(
    get,
    path = "/api/certificates/ca/{ca_name}",
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA encontrada", body = CertificateDetailsResponse),
        (status = 400, description = "Nombre inválido", body = CertificateCrudResponse),
        (status = 404, description = "CA no encontrada", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn get_ca_certificate_handler(
    State(state): State<AppState>,
    Path(ca_name): Path<String>,
) -> impl IntoResponse {
    certificate_read_handler(state, ca_name, CertificateKind::Ca).await
}

/// Crea una CA (certificado + llave privada) y recarga credenciales.
#[utoipa::path(
    post,
    path = "/api/certificates/ca",
    request_body = CaCertificateCreateRequest,
    responses(
        (status = 201, description = "CA creada", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud inválida", body = CertificateCrudResponse),
        (status = 409, description = "CA ya existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn create_ca_certificate_handler(
    State(state): State<AppState>,
    Json(payload): Json<CaCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    certificate_ca_upsert_handler(state, payload.name, params, false).await
}

/// Actualiza una CA (reemplaza certificado + llave) y recarga credenciales.
#[utoipa::path(
    put,
    path = "/api/certificates/ca/{ca_name}",
    request_body = CaCertificateUpsertRequest,
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA actualizada", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud inválida", body = CertificateCrudResponse),
        (status = 404, description = "CA no existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn update_ca_certificate_handler(
    State(state): State<AppState>,
    Path(ca_name): Path<String>,
    Json(payload): Json<CaCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = CaCertificateParams {
        common_name: payload.common_name,
        organization: payload.organization,
        country: payload.country,
        days: payload.days.unwrap_or(3650),
        key_size: payload.key_size.unwrap_or(4096),
    };
    certificate_ca_upsert_handler(state, ca_name, params, true).await
}

/// Elimina una CA y su llave privada, recargando credenciales.
#[utoipa::path(
    delete,
    path = "/api/certificates/ca/{ca_name}",
    params(("ca_name" = String, Path, description = "Nombre de la CA")),
    responses(
        (status = 200, description = "CA eliminada", body = CertificateCrudResponse),
        (status = 400, description = "Nombre inválido", body = CertificateCrudResponse),
        (status = 404, description = "CA no encontrada", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn delete_ca_certificate_handler(
    State(state): State<AppState>,
    Path(ca_name): Path<String>,
) -> impl IntoResponse {
    certificate_delete_handler(state, ca_name, CertificateKind::Ca).await
}

/// Lista certificados de usuario administrados por Bifröst.
#[utoipa::path(
    get,
    path = "/api/certificates/user",
    responses(
        (status = 200, description = "Listado de certificados de usuario", body = CertificateListResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn list_user_certificates_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: String::new(),
                kind: CertificateKind::User,
                action: "list".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        match list_managed_certificates(CertificateKind::User).await {
            Ok(certificates) => {
                (StatusCode::OK, Json(CertificateListResponse { certificates })).into_response()
            }
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error listando certificados de usuario: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name: String::new(),
                        kind: CertificateKind::User,
                        action: "list".to_string(),
                        success: false,
                        message: format!("Error listando certificados de usuario: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

/// Obtiene detalles de un certificado de usuario.
#[utoipa::path(
    get,
    path = "/api/certificates/user/{cert_name}",
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado encontrado", body = CertificateDetailsResponse),
        (status = 400, description = "Nombre inválido", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no encontrado", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn get_user_certificate_handler(
    State(state): State<AppState>,
    Path(cert_name): Path<String>,
) -> impl IntoResponse {
    certificate_read_handler(state, cert_name, CertificateKind::User).await
}

/// Crea un certificado de usuario firmado por una CA y recarga credenciales.
#[utoipa::path(
    post,
    path = "/api/certificates/user",
    request_body = UserCertificateCreateRequest,
    responses(
        (status = 201, description = "Certificado creado", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud inválida", body = CertificateCrudResponse),
        (status = 409, description = "Certificado ya existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn create_user_certificate_handler(
    State(state): State<AppState>,
    Json(payload): Json<UserCertificateCreateRequest>,
) -> impl IntoResponse {
    let params = UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    certificate_user_upsert_handler(state, payload.name, params, false).await
}

/// Actualiza un certificado de usuario (reemplaza llave/cert) y recarga credenciales.
#[utoipa::path(
    put,
    path = "/api/certificates/user/{cert_name}",
    request_body = UserCertificateUpsertRequest,
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado actualizado", body = CertificateCrudResponse),
        (status = 400, description = "Solicitud inválida", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no existe", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn update_user_certificate_handler(
    State(state): State<AppState>,
    Path(cert_name): Path<String>,
    Json(payload): Json<UserCertificateUpsertRequest>,
) -> impl IntoResponse {
    let params = UserCertificateParams {
        ca_name: payload.ca_name,
        identity: payload.identity,
        san: payload.san.unwrap_or_default(),
        days: payload.days.unwrap_or(825),
        key_size: payload.key_size.unwrap_or(4096),
    };
    certificate_user_upsert_handler(state, cert_name, params, true).await
}

/// Elimina un certificado de usuario y su llave privada, recargando credenciales.
#[utoipa::path(
    delete,
    path = "/api/certificates/user/{cert_name}",
    params(("cert_name" = String, Path, description = "Nombre del certificado de usuario")),
    responses(
        (status = 200, description = "Certificado eliminado", body = CertificateCrudResponse),
        (status = 400, description = "Nombre inválido", body = CertificateCrudResponse),
        (status = 404, description = "Certificado no encontrado", body = CertificateCrudResponse),
        (status = 500, description = "Error interno", body = CertificateCrudResponse),
        (status = 501, description = "Operación no soportada", body = CertificateCrudResponse)
    )
)]
async fn delete_user_certificate_handler(
    State(state): State<AppState>,
    Path(cert_name): Path<String>,
) -> impl IntoResponse {
    certificate_delete_handler(state, cert_name, CertificateKind::User).await
}

/// Adjunta un certificado de usuario a una conexión administrada por Bifröst.
#[utoipa::path(
    post,
    path = "/api/connections/{connection_name}/certificate",
    request_body = ConnectionCertificateAttachRequest,
    params(("connection_name" = String, Path, description = "Nombre de la conexión")),
    responses(
        (status = 200, description = "Certificado adjuntado", body = ConnectionCrudResponse),
        (status = 400, description = "Solicitud inválida", body = ConnectionCrudResponse),
        (status = 404, description = "Conexión o certificado no encontrado", body = ConnectionCrudResponse),
        (status = 500, description = "Error interno", body = ConnectionCrudResponse),
        (status = 501, description = "Operación no soportada", body = ConnectionCrudResponse)
    )
)]
async fn attach_certificate_to_connection_handler(
    State(_state): State<AppState>,
    Path(connection_name): Path<String>,
    Json(payload): Json<ConnectionCertificateAttachRequest>,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(ConnectionCrudResponse {
                name: connection_name,
                action: "attach-certificate".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let conn_name = match sanitize_connection_name(&connection_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: connection_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: "connection_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let cert_name = match sanitize_certificate_name(&payload.certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: conn_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: "certificate_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let conn_path = connection_file_path(&conn_name);
        let cert_path = user_certificate_cert_path(&cert_name);
        if tokio::fs::metadata(&cert_path).await.is_err() {
            return (
                StatusCode::NOT_FOUND,
                Json(ConnectionCrudResponse {
                    name: conn_name,
                    action: "attach-certificate".to_string(),
                    success: false,
                    message: "El certificado de usuario no existe".to_string(),
                }),
            )
                .into_response();
        }

        if let Some(ca_name) = &payload.remote_ca_name {
            let sanitized = match sanitize_certificate_name(ca_name) {
                Some(v) => v,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ConnectionCrudResponse {
                            name: conn_name,
                            action: "attach-certificate".to_string(),
                            success: false,
                            message: "remote_ca_name inválido".to_string(),
                        }),
                    )
                        .into_response()
                }
            };

            if tokio::fs::metadata(ca_certificate_cert_path(&sanitized)).await.is_err() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ConnectionCrudResponse {
                        name: conn_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: "La CA remota especificada no existe".to_string(),
                    }),
                )
                    .into_response();
            }
        }

        let content = match tokio::fs::read_to_string(&conn_path).await {
            Ok(value) => value,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ConnectionCrudResponse {
                        name: conn_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: "La conexión no existe".to_string(),
                    }),
                )
                    .into_response()
            }
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ConnectionCrudResponse {
                        name: conn_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: format!("Error leyendo conexión: {}", err),
                    }),
                )
                    .into_response()
            }
        };

        let config_body = extract_connection_body(&conn_name, &content).unwrap_or(content.clone());
        let updated = match apply_certificate_to_connection_config(&config_body, &payload) {
            Ok(v) => v,
            Err(err) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ConnectionCrudResponse {
                        name: conn_name,
                        action: "attach-certificate".to_string(),
                        success: false,
                        message: err,
                    }),
                )
                    .into_response()
            }
        };

        let conf_text = build_connection_conf(&conn_name, &updated);
        if let Err(err) = tokio::fs::write(&conn_path, conf_text).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ConnectionCrudResponse {
                    name: conn_name,
                    action: "attach-certificate".to_string(),
                    success: false,
                    message: format!("Error escribiendo conexión: {}", err),
                }),
            )
                .into_response();
        }

        match reload_swanctl_conns().await {
            Ok(()) => (
                StatusCode::OK,
                Json(ConnectionCrudResponse {
                    name: conn_name,
                    action: "attach-certificate".to_string(),
                    success: true,
                    message: "Certificado aplicado a la conexión".to_string(),
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(ConnectionCrudResponse {
                    name: conn_name,
                    action: "attach-certificate".to_string(),
                    success: false,
                    message: format!("Conexión actualizada pero falló reload: {}", err),
                }),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone)]
struct CaCertificateParams {
    common_name: String,
    organization: Option<String>,
    country: Option<String>,
    days: u32,
    key_size: u32,
}

#[derive(Debug, Clone)]
struct UserCertificateParams {
    ca_name: String,
    identity: String,
    san: Vec<String>,
    days: u32,
    key_size: u32,
}

async fn certificate_read_handler(
    state: AppState,
    certificate_name: String,
    kind: CertificateKind,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind,
                action: "read".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_certificate_name(&certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: certificate_name,
                        kind,
                        action: "read".to_string(),
                        success: false,
                        message: "certificate_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let cert_path = certificate_cert_path(kind, &name);
        let key_path = certificate_key_path(kind, &name);

        if tokio::fs::metadata(&cert_path).await.is_err() {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "read".to_string(),
                    success: false,
                    message: "Certificado no encontrado".to_string(),
                }),
            )
                .into_response();
        }

        match get_certificate_metadata(&cert_path).await {
            Ok((subject, issuer, not_after)) => (
                StatusCode::OK,
                Json(CertificateDetailsResponse {
                    name,
                    kind,
                    certificate_path: cert_path.to_string_lossy().to_string(),
                    private_key_path: Some(key_path.to_string_lossy().to_string()),
                    subject,
                    issuer,
                    not_after,
                }),
            )
                .into_response(),
            Err(err) => {
                state
                    .logger
                    .error(&format!("Error leyendo metadata de certificado: {}", err));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CertificateCrudResponse {
                        name,
                        kind,
                        action: "read".to_string(),
                        success: false,
                        message: format!("No se pudo leer metadata del certificado: {}", err),
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn certificate_ca_upsert_handler(
    _state: AppState,
    certificate_name: String,
    params: CaCertificateParams,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind: CertificateKind::Ca,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_certificate_name(&certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: certificate_name,
                        kind: CertificateKind::Ca,
                        action: action.to_string(),
                        success: false,
                        message: "name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        if params.common_name.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: "common_name es requerido".to_string(),
                }),
            )
                .into_response();
        }

        let cert_path = ca_certificate_cert_path(&name);
        let key_path = ca_certificate_key_path(&name);
        let exists = tokio::fs::metadata(&cert_path).await.is_ok();

        if !update && exists {
            return (
                StatusCode::CONFLICT,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: "La CA ya existe".to_string(),
                }),
            )
                .into_response();
        }
        if update && !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: "La CA no existe".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = ensure_certificate_directories().await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: format!("No se pudieron preparar directorios: {}", err),
                }),
            )
                .into_response();
        }

        if let Err(err) = generate_ca_certificate_files(&cert_path, &key_path, &params).await {
            return (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: format!("No se pudo generar la CA: {}", err),
                }),
            )
                .into_response();
        }

        match reload_swanctl_creds().await {
            Ok(()) => (
                if update { StatusCode::OK } else { StatusCode::CREATED },
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: true,
                    message: "CA generada y credenciales recargadas".to_string(),
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::Ca,
                    action: action.to_string(),
                    success: false,
                    message: format!("CA generada pero falló load-creds: {}", err),
                }),
            )
                .into_response(),
        }
    }
}

async fn certificate_user_upsert_handler(
    _state: AppState,
    certificate_name: String,
    params: UserCertificateParams,
    update: bool,
) -> impl IntoResponse {
    let action = if update { "update" } else { "create" };

    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind: CertificateKind::User,
                action: action.to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_certificate_name(&certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: certificate_name,
                        kind: CertificateKind::User,
                        action: action.to_string(),
                        success: false,
                        message: "name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };
        let ca_name = match sanitize_certificate_name(&params.ca_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name,
                        kind: CertificateKind::User,
                        action: action.to_string(),
                        success: false,
                        message: "ca_name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        if params.identity.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: "identity es requerido".to_string(),
                }),
            )
                .into_response();
        }

        let cert_path = user_certificate_cert_path(&name);
        let key_path = user_certificate_key_path(&name);
        let exists = tokio::fs::metadata(&cert_path).await.is_ok();
        if !update && exists {
            return (
                StatusCode::CONFLICT,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: "El certificado de usuario ya existe".to_string(),
                }),
            )
                .into_response();
        }
        if update && !exists {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: "El certificado de usuario no existe".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = ensure_certificate_directories().await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: format!("No se pudieron preparar directorios: {}", err),
                }),
            )
                .into_response();
        }

        let ca_cert_path = ca_certificate_cert_path(&ca_name);
        let ca_key_path = ca_certificate_key_path(&ca_name);
        if tokio::fs::metadata(&ca_cert_path).await.is_err() || tokio::fs::metadata(&ca_key_path).await.is_err() {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: "La CA especificada no existe o está incompleta".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = generate_user_certificate_files(
            &cert_path,
            &key_path,
            &ca_cert_path,
            &ca_key_path,
            &params,
        )
        .await
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: format!("No se pudo generar certificado de usuario: {}", err),
                }),
            )
                .into_response();
        }

        match reload_swanctl_creds().await {
            Ok(()) => (
                if update { StatusCode::OK } else { StatusCode::CREATED },
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: true,
                    message: "Certificado de usuario generado y credenciales recargadas".to_string(),
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind: CertificateKind::User,
                    action: action.to_string(),
                    success: false,
                    message: format!(
                        "Certificado de usuario generado pero falló load-creds: {}",
                        err
                    ),
                }),
            )
                .into_response(),
        }
    }
}

async fn certificate_delete_handler(
    state: AppState,
    certificate_name: String,
    kind: CertificateKind,
) -> impl IntoResponse {
    #[cfg(not(target_os = "linux"))]
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CertificateCrudResponse {
                name: certificate_name,
                kind,
                action: "delete".to_string(),
                success: false,
                message: "Operación soportada solo en Linux con StrongSwan".to_string(),
            }),
        )
            .into_response();
    }

    #[cfg(target_os = "linux")]
    {
        let name = match sanitize_certificate_name(&certificate_name) {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CertificateCrudResponse {
                        name: certificate_name,
                        kind,
                        action: "delete".to_string(),
                        success: false,
                        message: "name inválido".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        let cert_path = certificate_cert_path(kind, &name);
        let key_path = certificate_key_path(kind, &name);
        if tokio::fs::metadata(&cert_path).await.is_err() {
            return (
                StatusCode::NOT_FOUND,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "delete".to_string(),
                    success: false,
                    message: "Certificado no encontrado".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = tokio::fs::remove_file(&cert_path).await {
            state.logger.error(&format!("Error eliminando certificado: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "delete".to_string(),
                    success: false,
                    message: format!("Error eliminando certificado: {}", err),
                }),
            )
                .into_response();
        }
        let _ = tokio::fs::remove_file(&key_path).await;

        match reload_swanctl_creds().await {
            Ok(()) => (
                StatusCode::OK,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "delete".to_string(),
                    success: true,
                    message: "Certificado eliminado y credenciales recargadas".to_string(),
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::BAD_REQUEST,
                Json(CertificateCrudResponse {
                    name,
                    kind,
                    action: "delete".to_string(),
                    success: false,
                    message: format!("Certificado eliminado pero falló load-creds: {}", err),
                }),
            )
                .into_response(),
        }
    }
}

#[cfg(target_os = "linux")]
fn connection_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/conf.d/bifrost-{}.conf", name))
}

#[cfg(target_os = "linux")]
fn certificate_cert_path(kind: CertificateKind, name: &str) -> PathBuf {
    match kind {
        CertificateKind::Ca => ca_certificate_cert_path(name),
        CertificateKind::User => user_certificate_cert_path(name),
    }
}

#[cfg(target_os = "linux")]
fn certificate_key_path(kind: CertificateKind, name: &str) -> PathBuf {
    match kind {
        CertificateKind::Ca => ca_certificate_key_path(name),
        CertificateKind::User => user_certificate_key_path(name),
    }
}

#[cfg(target_os = "linux")]
fn ca_certificate_cert_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/x509ca/bifrost-ca-{}.crt", name))
}

#[cfg(target_os = "linux")]
fn ca_certificate_key_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/private/bifrost-ca-{}.key", name))
}

#[cfg(target_os = "linux")]
fn user_certificate_cert_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/x509/bifrost-user-{}.crt", name))
}

#[cfg(target_os = "linux")]
fn user_certificate_key_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/private/bifrost-user-{}.key", name))
}

#[cfg(target_os = "linux")]
fn sanitize_certificate_name(name: &str) -> Option<String> {
    sanitize_connection_name(name)
}

#[cfg(target_os = "linux")]
async fn ensure_certificate_directories() -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all("/etc/swanctl/private").await?;
    tokio::fs::create_dir_all("/etc/swanctl/x509").await?;
    tokio::fs::create_dir_all("/etc/swanctl/x509ca").await?;
    Ok(())
}

#[cfg(target_os = "linux")]
async fn list_managed_certificates(kind: CertificateKind) -> Result<Vec<String>, std::io::Error> {
    let (dir, prefix) = match kind {
        CertificateKind::Ca => ("/etc/swanctl/x509ca", "bifrost-ca-"),
        CertificateKind::User => ("/etc/swanctl/x509", "bifrost-user-"),
    };

    let mut names = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(v) => v,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(err) => return Err(err),
    };

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with(prefix) || !file_name.ends_with(".crt") {
            continue;
        }

        names.push(
            file_name
                .trim_start_matches(prefix)
                .trim_end_matches(".crt")
                .to_string(),
        );
    }
    names.sort();
    Ok(names)
}

#[cfg(target_os = "linux")]
fn build_subject(
    common_name: &str,
    organization: Option<&str>,
    country: Option<&str>,
) -> String {
    let mut subject = String::new();
    if let Some(country) = country {
        if !country.trim().is_empty() {
            subject.push_str(&format!("/C={}", country.trim()));
        }
    }
    if let Some(org) = organization {
        if !org.trim().is_empty() {
            subject.push_str(&format!("/O={}", org.trim()));
        }
    }
    subject.push_str(&format!("/CN={}", common_name.trim()));
    subject
}

#[cfg(target_os = "linux")]
async fn run_openssl(args: &[String]) -> Result<(), String> {
    let output = Command::new("openssl")
        .args(args)
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar openssl: {}", err))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err("openssl falló sin detalle".to_string())
    } else {
        Err(stderr)
    }
}

#[cfg(target_os = "linux")]
async fn generate_ca_certificate_files(
    cert_path: &PathBuf,
    key_path: &PathBuf,
    params: &CaCertificateParams,
) -> Result<(), String> {
    let gen_key_args = vec![
        "genpkey".to_string(),
        "-algorithm".to_string(),
        "RSA".to_string(),
        "-pkeyopt".to_string(),
        format!("rsa_keygen_bits:{}", params.key_size),
        "-out".to_string(),
        key_path.to_string_lossy().to_string(),
    ];
    run_openssl(&gen_key_args).await?;

    let subject = build_subject(
        &params.common_name,
        params.organization.as_deref(),
        params.country.as_deref(),
    );
    let req_args = vec![
        "req".to_string(),
        "-x509".to_string(),
        "-new".to_string(),
        "-key".to_string(),
        key_path.to_string_lossy().to_string(),
        "-sha256".to_string(),
        "-days".to_string(),
        params.days.to_string(),
        "-subj".to_string(),
        subject,
        "-out".to_string(),
        cert_path.to_string_lossy().to_string(),
    ];
    run_openssl(&req_args).await?;

    tokio::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de llave privada: {}", err))?;
    tokio::fs::set_permissions(cert_path, std::fs::Permissions::from_mode(0o644))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de certificado: {}", err))?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn generate_user_certificate_files(
    cert_path: &PathBuf,
    key_path: &PathBuf,
    ca_cert_path: &PathBuf,
    ca_key_path: &PathBuf,
    params: &UserCertificateParams,
) -> Result<(), String> {
    let gen_key_args = vec![
        "genpkey".to_string(),
        "-algorithm".to_string(),
        "RSA".to_string(),
        "-pkeyopt".to_string(),
        format!("rsa_keygen_bits:{}", params.key_size),
        "-out".to_string(),
        key_path.to_string_lossy().to_string(),
    ];
    run_openssl(&gen_key_args).await?;

    let subject = build_subject(&params.identity, None, None);
    let csr_path = std::env::temp_dir().join(format!(
        "bifrost-user-{}-{}.csr",
        params.identity.replace('/', "_"),
        std::process::id()
    ));
    let csr_args = vec![
        "req".to_string(),
        "-new".to_string(),
        "-key".to_string(),
        key_path.to_string_lossy().to_string(),
        "-subj".to_string(),
        subject,
        "-out".to_string(),
        csr_path.to_string_lossy().to_string(),
    ];
    run_openssl(&csr_args).await?;

    let mut sign_args = vec![
        "x509".to_string(),
        "-req".to_string(),
        "-in".to_string(),
        csr_path.to_string_lossy().to_string(),
        "-CA".to_string(),
        ca_cert_path.to_string_lossy().to_string(),
        "-CAkey".to_string(),
        ca_key_path.to_string_lossy().to_string(),
        "-CAcreateserial".to_string(),
        "-out".to_string(),
        cert_path.to_string_lossy().to_string(),
        "-days".to_string(),
        params.days.to_string(),
        "-sha256".to_string(),
    ];

    let mut ext_path: Option<PathBuf> = None;
    if !params.san.is_empty() {
        let mut san_values = Vec::with_capacity(params.san.len());
        for value in &params.san {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains(':') {
                san_values.push(trimmed.to_string());
            } else {
                san_values.push(format!("DNS:{}", trimmed));
            }
        }

        if !san_values.is_empty() {
            let ext = std::env::temp_dir().join(format!(
                "bifrost-user-{}-{}.ext",
                params.identity.replace('/', "_"),
                std::process::id()
            ));
            let ext_body = format!(
                "[v3_req]\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=clientAuth\nsubjectAltName={}\n",
                san_values.join(",")
            );
            tokio::fs::write(&ext, ext_body)
                .await
                .map_err(|err| format!("No se pudo escribir extensión SAN: {}", err))?;
            sign_args.push("-extfile".to_string());
            sign_args.push(ext.to_string_lossy().to_string());
            sign_args.push("-extensions".to_string());
            sign_args.push("v3_req".to_string());
            ext_path = Some(ext);
        }
    }

    let sign_result = run_openssl(&sign_args).await;
    let _ = tokio::fs::remove_file(&csr_path).await;
    if let Some(path) = ext_path {
        let _ = tokio::fs::remove_file(&path).await;
    }
    sign_result?;

    tokio::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de llave privada: {}", err))?;
    tokio::fs::set_permissions(cert_path, std::fs::Permissions::from_mode(0o644))
        .await
        .map_err(|err| format!("No se pudo fijar permisos de certificado: {}", err))?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn get_certificate_metadata(
    cert_path: &PathBuf,
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    let args = vec![
        "x509".to_string(),
        "-in".to_string(),
        cert_path.to_string_lossy().to_string(),
        "-noout".to_string(),
        "-subject".to_string(),
        "-issuer".to_string(),
        "-enddate".to_string(),
    ];

    let output = Command::new("openssl")
        .args(&args)
        .output()
        .await
        .map_err(|err| format!("No se pudo ejecutar openssl para metadata: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "openssl no pudo leer metadata".to_string()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut subject = None;
    let mut issuer = None;
    let mut not_after = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("subject=") {
            subject = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("issuer=") {
            issuer = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("notAfter=") {
            not_after = Some(v.trim().to_string());
        }
    }
    Ok((subject, issuer, not_after))
}

#[cfg(target_os = "linux")]
fn apply_certificate_to_connection_config(
    config_body: &str,
    payload: &ConnectionCertificateAttachRequest,
) -> Result<String, String> {
    let cert_name = sanitize_certificate_name(&payload.certificate_name)
        .ok_or_else(|| "certificate_name inválido".to_string())?;

    let mut output = config_body.to_string();
    let mut local_values = vec![
        (
            "auth".to_string(),
            "pubkey".to_string(),
        ),
        (
            "certs".to_string(),
            format!("x509/bifrost-user-{}.crt", cert_name),
        ),
    ];
    if let Some(local_id) = &payload.local_id {
        if local_id.trim().is_empty() {
            return Err("local_id no puede ser vacío".to_string());
        }
        local_values.push(("id".to_string(), local_id.trim().to_string()));
    }
    output = upsert_connection_section_values(&output, "local", &local_values);

    let mut remote_values: Vec<(String, String)> = Vec::new();
    if payload.set_remote_auth_pubkey.unwrap_or(true) {
        remote_values.push(("auth".to_string(), "pubkey".to_string()));
    }
    if let Some(ca_name) = &payload.remote_ca_name {
        let sanitized = sanitize_certificate_name(ca_name)
            .ok_or_else(|| "remote_ca_name inválido".to_string())?;
        remote_values.push((
            "cacerts".to_string(),
            format!("x509ca/bifrost-ca-{}.crt", sanitized),
        ));
    }
    if !remote_values.is_empty() {
        output = upsert_connection_section_values(&output, "remote", &remote_values);
    }

    Ok(output)
}

#[cfg(target_os = "linux")]
fn upsert_connection_section_values(
    config_body: &str,
    section: &str,
    values: &[(String, String)],
) -> String {
    let mut lines: Vec<String> = config_body.lines().map(|line| line.to_string()).collect();
    let section_label = format!("{} {{", section);

    if let Some((start, end, indent)) = find_top_level_section_range(&lines, &section_label) {
        let keys: HashSet<&str> = values.iter().map(|(key, _)| key.as_str()).collect();
        let mut kept = Vec::new();
        for line in &lines[start + 1..end] {
            let trimmed = line.trim();
            if let Some((key, _)) = trimmed.split_once('=') {
                if keys.contains(key.trim()) {
                    continue;
                }
            }
            kept.push(line.clone());
        }

        let inner_indent = format!("{}  ", indent);
        let mut rebuilt = Vec::new();
        rebuilt.push(format!("{}{}", indent, section_label));
        for (key, value) in values {
            rebuilt.push(format!("{}{} = {}", inner_indent, key, value));
        }
        rebuilt.extend(kept);
        rebuilt.push(format!("{}}}", indent));

        lines.splice(start..=end, rebuilt);
    } else {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(section_label);
        for (key, value) in values {
            lines.push(format!("  {} = {}", key, value));
        }
        lines.push("}".to_string());
    }

    lines.join("\n")
}

#[cfg(target_os = "linux")]
fn find_top_level_section_range(
    lines: &[String],
    section_label: &str,
) -> Option<(usize, usize, String)> {
    let mut depth: i32 = 0;
    let mut idx = 0;
    while idx < lines.len() {
        let line = &lines[idx];
        let trimmed = line.trim();
        if depth == 0 && trimmed == section_label {
            let indent = line
                .chars()
                .take_while(|c| c.is_ascii_whitespace())
                .collect::<String>();

            depth += brace_delta(line);
            let mut end = idx;
            let mut j = idx + 1;
            while j < lines.len() {
                depth += brace_delta(&lines[j]);
                if depth == 0 {
                    end = j;
                    break;
                }
                j += 1;
            }
            return Some((idx, end, indent));
        }

        depth += brace_delta(line);
        idx += 1;
    }
    None
}

#[cfg(target_os = "linux")]
fn brace_delta(line: &str) -> i32 {
    let open = line.matches('{').count() as i32;
    let close = line.matches('}').count() as i32;
    open - close
}

#[cfg(target_os = "linux")]
fn secret_file_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/etc/swanctl/conf.d/bifrost-secret-{}.conf", name))
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
fn sanitize_secret_name(name: &str) -> Option<String> {
    sanitize_connection_name(name)
}

#[cfg(target_os = "linux")]
fn build_secret_conf(name: &str, secret_type: SecretType, config_lines: &[String]) -> String {
    let section_name = format!("{}-{}", secret_type.as_str(), name);

    let mut out = String::from("secrets {\n");
    out.push_str(&format!("  {} {{\n", section_name));
    for line in config_lines {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("  }\n}\n");
    out
}

#[cfg(target_os = "linux")]
fn validate_and_render_secret_config(
    secret_type: SecretType,
    config: &Value,
) -> Result<Vec<String>, String> {
    let object = config
        .as_object()
        .ok_or_else(|| "config debe ser un objeto JSON".to_string())?;

    match secret_type {
        SecretType::Eap | SecretType::Xauth | SecretType::Ntlm | SecretType::Ike | SecretType::Ppk => {
            let allowed = HashSet::from(["secret", "id", "ids"]);
            validate_allowed_keys(object, &allowed)?;

            let secret = extract_required_string(object, "secret")?;
            if secret.trim().is_empty() {
                return Err("config.secret no puede estar vacío".to_string());
            }

            let ids = extract_ids(object)?;
            if ids.is_empty() {
                return Err("config.ids (o config.id) es requerido".to_string());
            }

            let mut lines = vec![format!("secret = {}", secret)];
            for (idx, id) in ids.iter().enumerate() {
                let key = if idx == 0 {
                    "id".to_string()
                } else {
                    format!("id{}", idx + 1)
                };
                lines.push(format!("{} = {}", key, id));
            }
            Ok(lines)
        }
        SecretType::Private
        | SecretType::Rsa
        | SecretType::Ecdsa
        | SecretType::Pkcs8
        | SecretType::Pkcs12 => {
            let allowed = HashSet::from(["file", "secret"]);
            validate_allowed_keys(object, &allowed)?;

            let file = extract_required_string(object, "file")?;
            let secret = extract_required_string(object, "secret")?;
            if file.trim().is_empty() {
                return Err("config.file no puede estar vacío".to_string());
            }
            if secret.trim().is_empty() {
                return Err("config.secret no puede estar vacío".to_string());
            }

            Ok(vec![
                format!("file = {}", file),
                format!("secret = {}", secret),
            ])
        }
        SecretType::Token => {
            let allowed = HashSet::from(["handle", "slot", "module", "pin"]);
            validate_allowed_keys(object, &allowed)?;

            let handle = extract_required_string(object, "handle")?;
            if handle.trim().is_empty() {
                return Err("config.handle no puede estar vacío".to_string());
            }

            let mut lines = vec![format!("handle = {}", handle)];
            if let Some(slot) = object.get("slot") {
                lines.push(format!("slot = {}", render_scalar_value(slot, "config.slot")?));
            }
            if let Some(module) = object.get("module") {
                lines.push(format!("module = {}", render_scalar_value(module, "config.module")?));
            }
            if let Some(pin) = object.get("pin") {
                lines.push(format!("pin = {}", render_scalar_value(pin, "config.pin")?));
            }

            Ok(lines)
        }
    }
}

#[cfg(target_os = "linux")]
fn validate_allowed_keys(
    object: &serde_json::Map<String, Value>,
    allowed: &HashSet<&str>,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(format!("config.{} no es válido para este tipo de secret", key));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn extract_required_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, String> {
    let value = object
        .get(key)
        .ok_or_else(|| format!("config.{} es requerido", key))?;

    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(format!("config.{} debe ser string", key)),
    }
}

#[cfg(target_os = "linux")]
fn extract_ids(object: &serde_json::Map<String, Value>) -> Result<Vec<String>, String> {
    if let Some(id) = object.get("id") {
        return match id {
            Value::String(s) if !s.trim().is_empty() => Ok(vec![s.clone()]),
            Value::String(_) => Err("config.id no puede estar vacío".to_string()),
            _ => Err("config.id debe ser string".to_string()),
        };
    }

    if let Some(ids) = object.get("ids") {
        return match ids {
            Value::Array(values) => {
                if values.is_empty() {
                    return Err("config.ids no puede ser vacío".to_string());
                }

                let mut out = Vec::with_capacity(values.len());
                for (idx, value) in values.iter().enumerate() {
                    match value {
                        Value::String(s) if !s.trim().is_empty() => out.push(s.clone()),
                        Value::String(_) => {
                            return Err(format!("config.ids[{}] no puede estar vacío", idx))
                        }
                        _ => return Err(format!("config.ids[{}] debe ser string", idx)),
                    }
                }
                Ok(out)
            }
            _ => Err("config.ids debe ser lista de strings".to_string()),
        };
    }

    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn render_scalar_value(value: &Value, field_name: &str) -> Result<String, String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        _ => Err(format!("{} debe ser string, número o booleano", field_name)),
    }
}

#[cfg(target_os = "linux")]
fn parse_secret_config_for_response(content: &str) -> Result<(SecretType, Value), String> {
    let mut section_name: Option<String> = None;
    let mut assignments: Vec<(String, String)> = Vec::new();
    let mut in_secret_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "secrets {" {
            continue;
        }

        if trimmed.ends_with('{') && section_name.is_none() {
            let name = trimmed.trim_end_matches('{').trim();
            section_name = Some(name.to_string());
            in_secret_block = true;
            continue;
        }

        if in_secret_block && trimmed == "}" {
            break;
        }

        if in_secret_block {
            if let Some((key, value)) = trimmed.split_once('=') {
                assignments.push((key.trim().to_string(), value.trim().to_string()));
            }
        }
    }

    let section_name = section_name.ok_or_else(|| "No se encontró sección de secret".to_string())?;
    let (prefix, _) = section_name
        .split_once('-')
        .ok_or_else(|| "No se pudo identificar tipo de secret".to_string())?;

    let secret_type = SecretType::from_str(prefix)
        .ok_or_else(|| "Tipo de secret desconocido".to_string())?;

    let mut config_map = serde_json::Map::new();
    let mut ids = Vec::new();
    for (key, value) in assignments {
        if key == "secret" || key == "pin" {
            config_map.insert(key, Value::String("***redacted***".to_string()));
            continue;
        }

        if key == "id" || key.starts_with("id") {
            ids.push(Value::String(value));
            continue;
        }

        config_map.insert(key, Value::String(value));
    }

    if !ids.is_empty() {
        config_map.insert("ids".to_string(), Value::Array(ids));
    }

    Ok((secret_type, Value::Object(config_map)))
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
async fn reload_swanctl_creds() -> Result<(), String> {
    match Command::new("swanctl").arg("--load-creds").output().await {
        Ok(output) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    Err("swanctl --load-creds falló sin detalle".to_string())
                } else {
                    Err(stderr)
                }
            }
        }
        Err(err) => Err(format!("No se pudo ejecutar swanctl --load-creds: {}", err)),
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
async fn list_managed_secrets() -> Result<Vec<String>, std::io::Error> {
    let mut names = Vec::new();
    let mut dir = tokio::fs::read_dir("/etc/swanctl/conf.d").await?;
    while let Some(entry) = dir.next_entry().await? {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with("bifrost-secret-") || !file_name.ends_with(".conf") {
            continue;
        }

        let name = file_name
            .trim_start_matches("bifrost-secret-")
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


