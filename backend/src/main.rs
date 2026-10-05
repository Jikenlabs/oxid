use oxid::api::routes::{create_router, AppState};
use oxid::auth::ApiKeyManager;
use oxid::cache::CacheManager;
use oxid::config::AppConfig;
use oxid::connectors::{self, DocumentStorage};
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialisation du sous-système de traçage et de logs
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oxid=info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::default();
    info!("Démarrage du serveur Oxid sur {}:{}", config.host, config.port);
    info!("Répertoire de stockage : {}", config.storage_dir.display());
    info!("Répertoire de cache : {}", config.cache_dir.display());

    info!("Identifiant de nœud cluster : {}", config.node_id);
    if let Some(ref r_url) = config.redis_url {
        info!("Cache distribué L2 et gestion des quotas activés via Redis/Valkey : {}", r_url);
    } else {
        info!("Cache distribué L2 activé via volume partagé / disque");
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

    // Tâche de fond périodique : purge des documents éphémères expirés toutes les 5 minutes
    let storage_cleanup = storage.clone();
    let ttl_secs = config.convert_ttl_secs;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            let purged = storage_cleanup.cleanup_expired_documents(ttl_secs);
            if purged > 0 {
                info!("Nettoyeur de documents éphémères : {} fichier(s) expiré(s) purgé(s)", purged);
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

    // Configuration des en-têtes CORS (Cross-Origin Resource Sharing)
    if config.cors_allowed_origins.iter().any(|o| o == "*") || config.cors_allowed_origins.is_empty() {
        info!("Politique CORS : toutes les origines autorisées (*)");
    } else {
        info!(
            "Politique CORS : {} origine(s) autorisée(s) : {:?} (credentials: {})",
            config.cors_allowed_origins.len(),
            config.cors_allowed_origins,
            config.cors_allow_credentials
        );
    }
    let cors = config.build_cors_layer();

    // Distribution des ressources statiques du frontend (fallback)
    let frontend_dist = std::env::var("OXID_FRONTEND_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./static"));

    let api_router = create_router(state);

    let app = if frontend_dist.exists() {
        info!("Distribution des composants frontend depuis {}", frontend_dist.display());
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
