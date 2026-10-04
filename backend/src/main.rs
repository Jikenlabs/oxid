mod api;
mod auth;
mod cache;
mod config;
mod connectors;
mod engine;
mod models;
mod security;

use api::routes::{create_router, AppState};
use auth::ApiKeyManager;
use cache::CacheManager;
use config::AppConfig;
use connectors::DocumentStorage;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oxid=info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::default();
    info!("Starting Oxid server on {}:{}", config.host, config.port);
    info!("Storage directory: {}", config.storage_dir.display());
    info!("Cache directory: {}", config.cache_dir.display());

    info!("Cluster Node ID: {}", config.node_id);
    if let Some(ref r_url) = config.redis_url {
        info!("Distributed Cache & Quotas L2 enabled via Redis/Valkey: {}", r_url);
    } else {
        info!("Distributed Cache L2 enabled via Shared Volume / Disk");
    }

    let storage = DocumentStorage::new(config.storage_dir.clone());
    let cache = CacheManager::with_redis(
        config.cache_dir.clone(),
        config.max_memory_cache_mb,
        config.redis_url.clone(),
    );
    let connectors = std::sync::Arc::new(connectors::ConnectorRegistry::new(config.storage_dir.clone()));
    let auth_manager = ApiKeyManager::new(
        config.api_keys.clone(),
        config.auth_required,
        config.redis_url.clone(),
    );

    // Spawn background task for cleaning expired ephemeral documents every 5 minutes
    let storage_cleanup = storage.clone();
    let ttl_secs = config.convert_ttl_secs;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            let purged = storage_cleanup.cleanup_expired_documents(ttl_secs);
            if purged > 0 {
                info!("Ephemeral document cleaner: purged {} expired file(s)", purged);
            }
        }
    });

    let state = AppState::new(
        config.clone(),
        storage,
        cache,
        connectors,
        auth_manager,
    );

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Static frontend serving fallback
    let frontend_dist = std::env::var("OXID_FRONTEND_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./static"));

    let api_router = create_router(state);

    let app = if frontend_dist.exists() {
        info!("Serving frontend assets from {}", frontend_dist.display());
        let index_file = frontend_dist.join("index.html");
        api_router
            .fallback_service(ServeDir::new(&frontend_dist).fallback(ServeFile::new(index_file)))
    } else {
        api_router
    }
    .layer(cors)
    .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Oxid is ready and listening at http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
