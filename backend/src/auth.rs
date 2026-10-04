use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct QuotaInfo {
    pub key: String,
    pub limit: u64,
    pub remaining: u64,
    pub reset_epoch_secs: u64,
}

#[derive(Debug, Clone)]
pub enum QuotaError {
    InvalidKey,
    QuotaExceeded {
        limit: u64,
        reset_epoch_secs: u64,
    },
    BackendError(String),
}

impl std::fmt::Display for QuotaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuotaError::InvalidKey => write!(f, "Invalid API key"),
            QuotaError::QuotaExceeded { limit, reset_epoch_secs } => {
                write!(f, "Quota exceeded (limit: {}, reset: {})", limit, reset_epoch_secs)
            }
            QuotaError::BackendError(msg) => write!(f, "Backend error: {}", msg),
        }
    }
}

impl std::error::Error for QuotaError {}

#[derive(Debug, Clone)]
struct LocalUsage {
    count: u64,
    window_start: u64,
}

#[derive(Clone)]
pub struct ApiKeyManager {
    keys: HashMap<String, u64>, // api_key -> limit_per_window
    auth_required: bool,
    redis_client: Option<redis::Client>,
    local_state: Arc<RwLock<HashMap<String, LocalUsage>>>,
    window_secs: u64,
}

impl ApiKeyManager {
    pub fn new(
        keys: HashMap<String, u64>,
        auth_required: bool,
        redis_url: Option<String>,
    ) -> Self {
        let redis_client = redis_url.and_then(|url| {
            match redis::Client::open(url.as_str()) {
                Ok(client) => {
                    info!("ApiKeyManager: Redis quota backend connected to {}", url);
                    Some(client)
                }
                Err(e) => {
                    warn!("ApiKeyManager: Failed to open Redis client ({}). Using local memory quota.", e);
                    None
                }
            }
        });

        Self {
            keys,
            auth_required,
            redis_client,
            local_state: Arc::new(RwLock::new(HashMap::new())),
            window_secs: 3600, // 1 hour window
        }
    }

    /// Check if key is authorized and within quota. If valid, increments quota usage by 1.
    pub async fn check_and_consume(&self, provided_key: Option<&str>) -> Result<QuotaInfo, QuotaError> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. If auth is not required and no key is provided, allow with unlimited quota
        if !self.auth_required && provided_key.is_none() {
            return Ok(QuotaInfo {
                key: "anonymous".to_string(),
                limit: 100_000,
                remaining: 99_999,
                reset_epoch_secs: current_time + self.window_secs,
            });
        }

        // 2. Validate key presence
        let key = match provided_key {
            Some(k) if !k.is_empty() => k.trim(),
            _ => return Err(QuotaError::InvalidKey),
        };

        // 3. Check if key is known
        let limit = match self.keys.get(key) {
            Some(&l) => l,
            None => {
                // If keys map is empty but auth_required is false, grant default limit
                if !self.auth_required {
                    10_000
                } else {
                    return Err(QuotaError::InvalidKey);
                }
            }
        };

        // 4. Try Redis for distributed quota tracking
        if let Some(ref client) = self.redis_client {
            match self.check_redis_quota(client, key, limit, current_time).await {
                Ok(info) => return Ok(info),
                Err(e) => {
                    warn!("Redis quota check failed ({}); falling back to local memory quota.", e);
                }
            }
        }

        // 5. Fallback: Local thread-safe in-memory quota tracking
        self.check_local_quota(key, limit, current_time).await
    }

    async fn check_redis_quota(
        &self,
        client: &redis::Client,
        key: &str,
        limit: u64,
        current_time: u64,
    ) -> Result<QuotaInfo, QuotaError> {
        let mut con = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| QuotaError::BackendError(e.to_string()))?;

        let redis_key = format!("oxid:quota:{}", key);

        // Redis atomic INCR
        let current_usage: u64 = redis::cmd("INCR")
            .arg(&redis_key)
            .query_async(&mut con)
            .await
            .map_err(|e| QuotaError::BackendError(e.to_string()))?;

        // If this is the first hit, set expiration to window_secs
        if current_usage == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(&redis_key)
                .arg(self.window_secs)
                .query_async(&mut con)
                .await
                .map_err(|e| QuotaError::BackendError(e.to_string()))?;
        }

        // Get TTL for reset epoch calculation
        let ttl: i64 = redis::cmd("TTL")
            .arg(&redis_key)
            .query_async(&mut con)
            .await
            .unwrap_or(self.window_secs as i64);

        let reset_epoch_secs = current_time + ttl.max(0) as u64;

        if current_usage > limit {
            return Err(QuotaError::QuotaExceeded {
                limit,
                reset_epoch_secs,
            });
        }

        let remaining = limit.saturating_sub(current_usage);

        Ok(QuotaInfo {
            key: key.to_string(),
            limit,
            remaining,
            reset_epoch_secs,
        })
    }

    async fn check_local_quota(
        &self,
        key: &str,
        limit: u64,
        current_time: u64,
    ) -> Result<QuotaInfo, QuotaError> {
        let mut state = self.local_state.write().await;
        let usage = state.entry(key.to_string()).or_insert(LocalUsage {
            count: 0,
            window_start: current_time,
        });

        // Window expired: reset window
        if current_time >= usage.window_start + self.window_secs {
            usage.count = 0;
            usage.window_start = current_time;
        }

        usage.count += 1;
        let reset_epoch_secs = usage.window_start + self.window_secs;

        if usage.count > limit {
            return Err(QuotaError::QuotaExceeded {
                limit,
                reset_epoch_secs,
            });
        }

        let remaining = limit.saturating_sub(usage.count);

        Ok(QuotaInfo {
            key: key.to_string(),
            limit,
            remaining,
            reset_epoch_secs,
        })
    }
}
