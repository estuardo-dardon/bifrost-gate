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

use axum::{routing::get, Router, extract::State, response::IntoResponse};
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

/// Obtiene las métricas de Prometheus
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Métricas Prometheus en formato de texto")
    )
)]
async fn metrics_handler(
    State(metrics): State<MetricsState>
) -> impl IntoResponse {
    match metrics.encode_metrics() {
        Ok(data) => (
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            data,
        ),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "Error encoding metrics".to_string(),
        ),
    }
}

#[tokio::main]
async fn main() {
    // 1. Cargar configuración
    let settings = config::Settings::new().expect("No se pudo cargar config.toml");

    // 2. Inicializar componentes
    let pool = db::init_db().await;
    let current_topology = Arc::new(RwLock::new(engine::generate_mock_topology()));
    
    // 3. Inicializar métricas Prometheus
    let metrics = Arc::new(metrics::Metrics::new()
        .expect("Failed to initialize Prometheus metrics"));
    
    // 4. Iniciar el Worker
    let worker_state = Arc::clone(&current_topology);
    let worker_pool = pool.clone();
    tokio::spawn(async move {
        worker::start_heimdall_worker(worker_state, worker_pool).await;
    });

    // 5. Configurar la API
    let cors = CorsLayer::permissive();
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .route("/metrics", get(metrics_handler))
        .route("/api/topology", get(get_topology_handler))
        .with_state(Arc::clone(&current_topology))
        .layer(axum::middleware::Next::new())
        .layer(cors)
        .with_state(metrics.clone());

    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port)
        .parse()
        .expect("Dirección de servidor inválida");

    // 6. Lógica de encendido
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
    State(state): State<SharedState>,
    State(metrics): State<MetricsState>,
) -> axum::Json<models::BifrostTopology> {
    let topo = state.read().unwrap();
    
    // Incrementar métricas
    metrics.topology_requests.inc();
    metrics.topology_nodes_count.set(topo.nodes.len() as f64);
    metrics.topology_edges_count.set(topo.edges.len() as f64);
    
    axum::Json(topo.clone())
}
