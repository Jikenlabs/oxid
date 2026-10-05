use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub storage_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub max_memory_cache_mb: usize,
    pub default_dpi: u32,
    pub node_id: String,
    pub redis_url: Option<String>,
    pub office_engine: String,
    pub api_keys: std::collections::HashMap<String, u64>,
    pub auth_required: bool,
    pub convert_ttl_secs: u64,
    pub cors_allowed_origins: Vec<String>,
    pub cors_allow_credentials: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let base_dir = std::env::var("OXID_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));

        let node_id = std::env::var("OXID_NODE_ID")
            .unwrap_or_else(|_| format!("node-{}", &uuid::Uuid::new_v4().to_string()[..8]));

        let redis_url = std::env::var("OXID_REDIS_URL").ok();
        let office_engine = std::env::var("OXID_OFFICE_ENGINE")
            .unwrap_or_else(|_| "hybrid".to_string())
            .to_lowercase();

        // Analyse du format des clés API : "clé1:quota,clé2:quota" (quota par défaut = 1000 requêtes/heure)
        let mut api_keys = std::collections::HashMap::new();
        if let Ok(keys_str) = std::env::var("OXID_API_KEYS") {
            for entry in keys_str.split(',') {
                let parts: Vec<&str> = entry.trim().split(':').collect();
                if parts.len() >= 2 {
                    if let Ok(limit) = parts[1].parse::<u64>() {
                        api_keys.insert(parts[0].to_string(), limit);
                    }
                } else if !parts.is_empty() && !parts[0].is_empty() {
                    api_keys.insert(parts[0].to_string(), 1000);
                }
            }
        }

        let auth_required = std::env::var("OXID_AUTH_REQUIRED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(!api_keys.is_empty());

        let convert_ttl_secs = std::env::var("OXID_CONVERT_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(900); // Durée de vie par défaut : 15 minutes (900s)

        // Analyse de la configuration CORS (Cross-Origin Resource Sharing)
        // Variable prioritaire : OXID_CORS_ALLOWED_ORIGINS (alias de compatibilité : OXID_CORS_ORIGIN)
        // Valeurs séparées par des virgules (ex: "http://localhost:3000,https://app.mondomaine.fr")
        // Valeur par défaut : "*" (toutes les origines sont autorisées)
        let cors_raw = std::env::var("OXID_CORS_ALLOWED_ORIGINS")
            .or_else(|_| std::env::var("OXID_CORS_ORIGIN"))
            .unwrap_or_else(|_| "*".to_string());

        let cors_allowed_origins: Vec<String> = cors_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let is_wildcard = cors_allowed_origins.iter().any(|o| o == "*");

        // Autoriser l'envoi de cookies/identifiants d'authentification (Access-Control-Allow-Credentials)
        // Désactivé par défaut si wildcard '*' (interdit par la norme W3C CORS), activé par défaut si origines explicites
        let cors_allow_credentials = std::env::var("OXID_CORS_ALLOW_CREDENTIALS")
            .map(|v| v == "true" || v == "1")
            .unwrap_or_else(|_| !is_wildcard && !cors_allowed_origins.is_empty());

        Self {
            host: std::env::var("OXID_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("OXID_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            storage_dir: base_dir.join("documents"),
            cache_dir: base_dir.join("cache"),
            max_memory_cache_mb: 256,
            default_dpi: 120,
            node_id,
            redis_url,
            office_engine,
            api_keys,
            auth_required,
            convert_ttl_secs,
            cors_allowed_origins,
            cors_allow_credentials,
        }
    }
}

impl AppConfig {
    /// Construit la couche middleware CORS adaptée selon les origines configurées.
    pub fn build_cors_layer(&self) -> tower_http::cors::CorsLayer {
        use axum::http::header::{self, HeaderName};
        use axum::http::HeaderValue;
        use tower_http::cors::{AllowHeaders, AllowMethods, Any, CorsLayer};

        let is_wildcard = self.cors_allowed_origins.is_empty()
            || self.cors_allowed_origins.iter().any(|o| o == "*");

        if is_wildcard {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any)
        } else {
            let valid_origins: Vec<HeaderValue> = self
                .cors_allowed_origins
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();

            if valid_origins.is_empty() {
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any)
                    .expose_headers(Any)
            } else if self.cors_allow_credentials {
                let exposed_headers = vec![
                    header::CONTENT_DISPOSITION,
                    header::CONTENT_LENGTH,
                    header::CONTENT_TYPE,
                    header::CONTENT_RANGE,
                    HeaderName::from_static("x-ratelimit-limit"),
                    HeaderName::from_static("x-ratelimit-remaining"),
                    HeaderName::from_static("x-ratelimit-reset"),
                ];

                CorsLayer::new()
                    .allow_origin(valid_origins)
                    .allow_methods(AllowMethods::mirror_request())
                    .allow_headers(AllowHeaders::mirror_request())
                    .expose_headers(exposed_headers)
                    .allow_credentials(true)
            } else {
                CorsLayer::new()
                    .allow_origin(valid_origins)
                    .allow_methods(Any)
                    .allow_headers(Any)
                    .expose_headers(Any)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_cors_default_configuration() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("OXID_CORS_ALLOWED_ORIGINS");
        std::env::remove_var("OXID_CORS_ORIGIN");
        std::env::remove_var("OXID_CORS_ALLOW_CREDENTIALS");

        let config = AppConfig::default();
        assert_eq!(config.cors_allowed_origins, vec!["*"]);
        assert!(!config.cors_allow_credentials);

        let _cors_layer = config.build_cors_layer();
    }

    #[test]
    fn test_cors_custom_multiple_origins() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var(
            "OXID_CORS_ALLOWED_ORIGINS",
            "https://app.jikenlabs.com, http://localhost:3000 , http://127.0.0.1:8080 ",
        );
        std::env::remove_var("OXID_CORS_ORIGIN");
        std::env::remove_var("OXID_CORS_ALLOW_CREDENTIALS");

        let config = AppConfig::default();
        assert_eq!(
            config.cors_allowed_origins,
            vec![
                "https://app.jikenlabs.com",
                "http://localhost:3000",
                "http://127.0.0.1:8080"
            ]
        );
        // Si origines explicites, credentials est activé par défaut
        assert!(config.cors_allow_credentials);

        let _cors_layer = config.build_cors_layer();

        // Nettoyage
        std::env::remove_var("OXID_CORS_ALLOWED_ORIGINS");
    }

    #[test]
    fn test_cors_credentials_override_and_alias() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("OXID_CORS_ALLOWED_ORIGINS");
        std::env::set_var("OXID_CORS_ORIGIN", "https://viewer.example.org");
        std::env::set_var("OXID_CORS_ALLOW_CREDENTIALS", "false");

        let config = AppConfig::default();
        assert_eq!(config.cors_allowed_origins, vec!["https://viewer.example.org"]);
        assert!(!config.cors_allow_credentials);

        let _cors_layer = config.build_cors_layer();

        // Nettoyage
        std::env::remove_var("OXID_CORS_ORIGIN");
        std::env::remove_var("OXID_CORS_ALLOW_CREDENTIALS");
    }
}
