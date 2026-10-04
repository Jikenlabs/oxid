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

        // Parse API keys format: "key1:quota,key2:quota" (default quota = 1000/hour)
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
            .unwrap_or(900); // 15 minutes default

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
        }
    }
}
