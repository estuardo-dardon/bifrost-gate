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

use axum::{routing::get, Router, extract::State, response::IntoResponse, middleware as axum_middleware};
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;
use std::fs::File;
use std::io::BufReader;
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

#[derive(OpenApi)]
#[openapi(
    paths(
        get_topology_handler,
        metrics_handler,
    ),
    components(schemas(
        models::BifrostTopology,
        models::NetworkNode,
        models::VpnEdge,
        models::NodeType,
        models::VpnStatus
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
        .layer(
            axum_middleware::from_fn_with_state(
                api_key_middleware_state,
                middleware::api_key_middleware,
            )
        )
        .with_state(app_state);
    
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
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
        println!("📖 ReDoc API: https://{}/redoc", addr);
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
        println!("📖 ReDoc API: http://{}/redoc", addr);
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


