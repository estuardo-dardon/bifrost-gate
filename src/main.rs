mod api;
mod config;
mod db;
mod engine;
mod exec;
mod logger;
mod metrics;
mod middleware;
mod models;
mod worker;
mod i18n;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{extract::State, middleware as axum_middleware, response::IntoResponse, routing::get, Router};
use auto_instrument::auto_instrument;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio_rustls::rustls::{pki_types::CertificateDer, ServerConfig};
use tower::Service;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_redoc::{Redoc, Servable};
use utoipa_swagger_ui::SwaggerUi;
use sqlx::SqlitePool;

pub(crate) type SharedState = Arc<RwLock<models::BifrostTopology>>;
pub(crate) type MetricsState = Arc<metrics::Metrics>;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) topology: SharedState,
    pub(crate) metrics: MetricsState,
    pub(crate) logger: Arc<logger::Logger>,
    pub(crate) pool: SqlitePool,
    pub(crate) worker_heartbeat_epoch_seconds: Arc<AtomicU64>,
}

#[utoipa::path(
    get,
    path = "/metrics",
    responses((status = 200, description = "Metricas Prometheus en formato de texto"))
)]
#[auto_instrument]
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
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
            state
                .logger
                .log_api_error("GET", "/metrics", 500, "Error encoding metrics");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "text/plain")],
                "Error encoding metrics".to_string(),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/heartbeat",
    responses((status = 200, description = "Estado de salud del servicio", body = crate::api::types::HeartbeatResponse))
)]
#[auto_instrument]
pub async fn heartbeat_handler(State(state): State<AppState>) -> impl IntoResponse {
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();

    let worker_last = state
        .worker_heartbeat_epoch_seconds
        .load(Ordering::Relaxed);
    let worker_ok = worker_last > 0 && now_epoch.saturating_sub(worker_last) <= 30;

    let database_ok = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    let strongswan_ok = {
        #[cfg(target_os = "linux")]
        {
            match crate::exec::run_command(
                &crate::exec::ExecConfig::default(),
                "swanctl",
                &["--list-sas"],
                Some(Duration::from_secs(5)),
            )
            .await
            {
                Ok(output) => output.status_code == Some(0),
                Err(_) => false,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    };

    let status = if !database_ok || !worker_ok {
        3
    } else if !strongswan_ok {
        2
    } else {
        1
    };

    let message = match status {
        1 => "OK: servicios disponibles".to_string(),
        2 => "WARN: algun servicio no responde".to_string(),
        _ => "CRITICAL: sin conexion con servicios mayores".to_string(),
    };

    let response = crate::api::types::HeartbeatResponse {
        status,
        message,
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };

    (axum::http::StatusCode::OK, axum::Json(response))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("bifrost-gate {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let settings = config::Settings::new().expect("No se pudo cargar config.toml");

    logger::init_async_logger(
        settings.logging.service_access_log.as_deref(),
        settings.logging.service_error_log.as_deref(),
        settings.logging.worker_log.as_deref(),
        settings.logging.use_journalctl.unwrap_or(true),
        settings.logging.channel_capacity.unwrap_or(1000),
        settings.logging.rotate_size_mb.unwrap_or(10) * 1024 * 1024,
    );
    logger::init_tracing_forwarder();

    let service_logger = Arc::new(logger::Logger::with_custom_paths(
        settings.logging.service_level,
        "service",
        settings.logging.service_access_log.as_deref(),
        settings.logging.service_error_log.as_deref(),
        None,
    ));

    let pool = db::init_db().await;

    if settings.auth.enabled {
        let bootstrap_user = settings
            .auth
            .bootstrap_user
            .clone()
            .unwrap_or_else(|| "admin".to_string());

        if let Some(ref bootstrap_key) = settings.auth.bootstrap_api_key {
            let trimmed = bootstrap_key.trim();
            let looks_default = trimmed.eq_ignore_ascii_case("change-me-in-production")
                || trimmed.eq_ignore_ascii_case("replace-with-strong-secret")
                || trimmed.eq_ignore_ascii_case("changeme")
                || trimmed.eq_ignore_ascii_case("change-me")
                || trimmed.eq_ignore_ascii_case("default");
            let too_short = trimmed.len() < 24;
            if trimmed.is_empty() || looks_default || too_short {
                service_logger.error(
                    "Config insegura: [auth].bootstrap_api_key es débil o es un valor por defecto. Reemplázala por un secreto fuerte (>=24 chars) antes de arrancar.",
                );
                std::process::exit(1);
            }
            match db::seed_api_key_if_missing(&pool, &bootstrap_user, bootstrap_key).await {
                Ok(true) => service_logger.info("API key bootstrap creada en DB"),
                Ok(false) => service_logger.info("Bootstrap omitido: ya existen API keys activas"),
                Err(err) => {
                    service_logger.error(&format!("Error sembrando API key bootstrap: {}", err))
                }
            }
        }

        match db::count_active_api_keys(&pool).await {
            Ok(0) => {
                service_logger.error("Auth habilitada pero no hay API keys activas en DB. Configura [auth].bootstrap_api_key o crea una key por otro medio.");
                std::process::exit(1);
            }
            Ok(count) => {
                service_logger.info(&format!("API keys activas en DB: {}", count));
            }
            Err(err) => {
                service_logger.error(&format!("No se pudo contar API keys activas: {}", err));
                std::process::exit(1);
            }
        }
    }
    let current_topology = Arc::new(RwLock::new(engine::generate_mock_topology()));
    let metrics = Arc::new(metrics::Metrics::new().expect("Failed to initialize Prometheus metrics"));

    let worker_state = Arc::clone(&current_topology);
    let worker_pool = pool.clone();
    let worker_heartbeat_epoch_seconds = Arc::new(AtomicU64::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs(),
    ));
    let worker_heartbeat_for_task = Arc::clone(&worker_heartbeat_epoch_seconds);
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
        worker::start_heimdall_worker_with_logger(
            worker_state,
            worker_pool,
            worker_logger,
            worker_heartbeat_for_task,
        )
        .await;
    });

    let cors = CorsLayer::permissive();
    let app_state = AppState {
        topology: Arc::clone(&current_topology),
        metrics: metrics.clone(),
        logger: Arc::clone(&service_logger),
        pool: pool.clone(),
        worker_heartbeat_epoch_seconds,
    };

    let logging_middleware_state = middleware::LoggingMiddlewareState {
        logger: Arc::clone(&service_logger),
    };

    let response_localization_middleware_state = middleware::ResponseLocalizationMiddlewareState {
        pool: pool.clone(),
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
        .merge(api::router::topology::routes())
        .merge(api::router::peers::routes())
        .merge(api::router::strongswan::routes())
        .merge(api::router::connections::routes())
        .merge(api::router::secrets::routes())
        .merge(api::router::certificates::routes())
        .layer(axum_middleware::from_fn_with_state(
            api_key_middleware_state,
            middleware::api_key_middleware,
        ))
        .with_state(app_state.clone());

    let docs_auth_middleware_state = middleware::DocsAuthMiddlewareState {
        logger: Arc::clone(&service_logger),
        pool: pool.clone(),
    };

    let public_routes = Router::new()
        .route("/heartbeat", get(heartbeat_handler))
        .with_state(app_state.clone());

    let docs_routes = Router::new()
        .route("/metrics", get(metrics_handler))
        .merge(SwaggerUi::new("/api/docs").url("/api/docs/openapi.json", api::docs::ApiDoc::openapi()))
        .merge(Redoc::with_url("/api/tryme", api::docs::ApiDoc::openapi()))
        .merge(api::router::response_codes::routes())
        .layer(axum_middleware::from_fn_with_state(
            docs_auth_middleware_state,
            middleware::docs_basic_auth_middleware,
        ))
        .with_state(app_state);

    let app = Router::new()
        .merge(public_routes)
        .merge(docs_routes)
        .merge(protected_routes)
        .layer(axum_middleware::from_fn_with_state(
            response_localization_middleware_state,
            middleware::response_localization_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            logging_middleware_state,
            middleware::logging_middleware,
        ))
        .layer(cors);

    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port)
        .parse()
        .expect("Direccion de servidor invalida");

    if settings.tls.enabled {
        let cert_file = File::open(&settings.tls.cert_path).expect("No cert.pem");
        let key_file = File::open(&settings.tls.key_path).expect("No key.pem");
        let mut cert_reader = BufReader::new(cert_file);
        let mut key_reader = BufReader::new(key_file);

        let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .expect("Error en certificados");

        let key = rustls_pemfile::private_key(&mut key_reader)
            .expect("Error en llave")
            .expect("No se encontro llave");

        let mut tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("Configuracion TLS invalida");

        tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls_config));
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

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

                    let _ = Builder::new(TokioExecutor::new()).serve_connection(io, service).await;
                }
            });
        }
    } else {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}
