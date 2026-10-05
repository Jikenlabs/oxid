use crate::cache::CacheManager;
use crate::config::AppConfig;
use crate::connectors::traits::{DocumentReference, SecurityContext};
use crate::connectors::{ConnectorRegistry, DocumentStorage};
use crate::engine::annotations::AnnotationEngine;
use crate::engine::builder::DocumentBuilderEngine;
use crate::engine::comparison::ComparisonEngine;
use crate::engine::forms::FormEngine;
use crate::engine::pdf::PdfEngine;
use crate::engine::pii::PiiEngine;
use crate::engine::redaction::RedactionEngine;
use crate::engine::signature::DigitalSignatureEngine;
use crate::models::{
    Annotation, CompareRequest, DocumentBuildOrder, DocumentMetadata, FormFillRequest,
    FormFillResponse, FormFieldsSummary, GlobalSearchResponse, PiiScanResponse, RedactionOrder,
    SignRequest, SignResponse, TextDiffResult, WatermarkOptions,
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tracing::{error, info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use crate::auth::{ApiKeyManager, QuotaError};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub storage: DocumentStorage,
    pub cache: CacheManager,
    pub connectors: Arc<ConnectorRegistry>,
    pub auth: ApiKeyManager,
    pub collab_tx: tokio::sync::broadcast::Sender<String>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        storage: DocumentStorage,
        cache: CacheManager,
        connectors: Arc<ConnectorRegistry>,
        auth: ApiKeyManager,
    ) -> Self {
        let (collab_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            config,
            storage,
            cache,
            connectors,
            auth,
            collab_tx,
        }
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/metrics", get(get_prometheus_metrics))
        .route("/api/cluster/status", get(get_cluster_status))
        .route("/api/convert", post(convert_document_oneshot))
        .route("/v1/convert", post(convert_document_oneshot))
        .route("/api/documents", post(upload_document))
        .route("/api/documents/upload", post(upload_document))
        .route("/api/documents/:id", get(get_document_metadata))
        .route("/api/documents/:id/pages/:page/render", get(render_page))
        .route("/api/documents/:id/pages/:page/thumbnail", get(render_thumbnail))
        .route("/api/documents/:id/pages/:page/text", get(get_page_text))
        .route(
            "/api/documents/:id/annotations",
            get(get_annotations).post(save_annotations),
        )
        .route("/api/documents/:id/collab", get(collab_stream))
        .route("/api/documents/:id/redact", post(redact_document))
        .route("/api/documents/:id/sign", post(sign_document))
        .route("/api/documents/build", post(build_document))
        .route("/api/documents/compare", post(compare_documents))
        .route("/api/documents/compare/text", post(compare_documents_text))
        .route("/api/documents/:id/download", get(download_document))
        .route("/api/documents/:id/pii-scan", get(scan_document_pii))
        .route("/api/documents/:id/search", get(search_document_text))
        .route("/api/documents/:id/forms", get(get_document_forms))
        .route("/api/documents/:id/forms/fill", post(fill_document_forms))
        .route("/api/documents/:id/cad/layers", get(get_cad_layers))
        .route("/api/documents/:id/dicom/metadata", get(get_dicom_metadata))
        .route("/api/documents/:id/video", get(stream_document_content))
        .route("/api/documents/:id/content", get(stream_document_content))
        .route("/api/connectors/open", get(open_remote_document))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(state)
}


async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "node_id": state.config.node_id,
        "engine": "Oxid",
        "version": env!("CARGO_PKG_VERSION"),
        "architecture": "Rust + Tokio + Axum",
        "cluster_mode": if state.config.redis_url.is_some() { "redis_distributed" } else { "shared_storage" },
        "connectors": ["filesystem", "s3", "cmis", "url"]
    }))
}

async fn get_cluster_status(State(state): State<AppState>) -> impl IntoResponse {
    use std::sync::atomic::Ordering;
    let l1_hits = state.cache.stats.l1_hits.load(Ordering::Relaxed);
    let l2_hits = state.cache.stats.l2_hits.load(Ordering::Relaxed);
    let misses = state.cache.stats.misses.load(Ordering::Relaxed);
    let writes = state.cache.stats.writes.load(Ordering::Relaxed);
    let l1_count = state.cache.l1_entry_count().await;

    let total_lookups = l1_hits + l2_hits + misses;
    let hit_ratio = if total_lookups > 0 {
        ((l1_hits + l2_hits) as f64 / total_lookups as f64) * 100.0
    } else {
        100.0
    };

    Json(serde_json::json!({
        "node_id": state.config.node_id,
        "cluster_status": "HEALTHY",
        "distributed_cache_backend": state.config.redis_url.as_deref().unwrap_or("shared_volume"),
        "metrics": {
            "l1_memory_cached_pages": l1_count,
            "l1_ram_hits": l1_hits,
            "l2_distributed_hits": l2_hits,
            "cache_misses": misses,
            "cache_writes": writes,
            "cache_hit_ratio_percent": (hit_ratio * 100.0).round() / 100.0,
        }
    }))
}

async fn get_prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    use std::sync::atomic::Ordering;
    let l1_hits = state.cache.stats.l1_hits.load(Ordering::Relaxed);
    let l2_hits = state.cache.stats.l2_hits.load(Ordering::Relaxed);
    let misses = state.cache.stats.misses.load(Ordering::Relaxed);
    let writes = state.cache.stats.writes.load(Ordering::Relaxed);
    let l1_count = state.cache.l1_entry_count().await;

    let metrics_text = format!(
        "# HELP oxid_cache_hits_total Total number of cache hits by tier\n\
         # TYPE oxid_cache_hits_total counter\n\
         oxid_cache_hits_total{{node=\"{}\",tier=\"l1_ram\"}} {}\n\
         oxid_cache_hits_total{{node=\"{}\",tier=\"l2_distributed\"}} {}\n\
         # HELP oxid_cache_misses_total Total number of cache misses\n\
         # TYPE oxid_cache_misses_total counter\n\
         oxid_cache_misses_total{{node=\"{}\"}} {}\n\
         # HELP oxid_cache_writes_total Total number of page writes to cache\n\
         # TYPE oxid_cache_writes_total counter\n\
         oxid_cache_writes_total{{node=\"{}\"}} {}\n\
         # HELP oxid_l1_cache_entries Currently resident pages in L1 RAM cache\n\
         # TYPE oxid_l1_cache_entries gauge\n\
         oxid_l1_cache_entries{{node=\"{}\"}} {}\n\
         # HELP oxid_cluster_up Indicates if node is online\n\
         # TYPE oxid_cluster_up gauge\n\
         oxid_cluster_up{{node=\"{}\"}} 1\n",
        state.config.node_id, l1_hits,
        state.config.node_id, l2_hits,
        state.config.node_id, misses,
        state.config.node_id, writes,
        state.config.node_id, l1_count,
        state.config.node_id,
    );

    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        metrics_text,
    )
}

#[derive(Deserialize)]
struct RemoteOpenParams {
    connector: Option<String>,
    id: Option<String>,
    url: Option<String>,
    token: Option<String>,
    filename: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    password: Option<String>,
}

async fn open_remote_document(
    State(state): State<AppState>,
    Query(params): Query<RemoteOpenParams>,
) -> Result<Json<DocumentMetadata>, (StatusCode, String)> {
    let basic_auth = if let Some(ref u) = params.user {
        let p = params.pass.clone().or(params.password.clone()).unwrap_or_default();
        Some((u.clone(), p))
    } else if let Some(ref t) = params.token {
        if let Some(idx) = t.find(':') {
            Some((t[..idx].to_string(), t[idx + 1..].to_string()))
        } else {
            None
        }
    } else if params.connector.as_deref() == Some("cmis") {
        // Identifiants par défaut pour le serveur de test OpenCMIS
        Some(("user1".to_string(), "cm1sp@ssword".to_string()))
    } else {
        None
    };

    let ctx = SecurityContext {
        user_id: Some("user".to_string()),
        auth_token: params.token.clone(),
        basic_auth,
        permissions: vec!["read".to_string(), "annotate".to_string()],
    };

    // Cas 1 : URL HTTP/HTTPS directe
    if let Some(ref doc_url) = params.url {
        // Validation de l'URL contre les attaques SSRF (blocage des IP privées, loopback et métadonnées cloud)
        let validated_url = crate::security::validate_ssrf_url(doc_url)
            .map_err(|e| (StatusCode::FORBIDDEN, format!("Protection SSRF: {}", e)))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut req = client.get(validated_url.as_str());

        // 1. Prise en charge des identifiants HTTP Basic intégrés dans l'URL (ex: http://user:pass@host/...)
        if !validated_url.username().is_empty() {
            req = req.basic_auth(validated_url.username(), validated_url.password());
        }

        // 2. Prise en charge du paramètre explicite de jeton d'authentification
        if let Some(ref token) = params.token {
            let auth_header = if token.starts_with("Token ") || token.starts_with("Bearer ") || token.starts_with("Basic ") {
                token.to_string()
            } else if token.len() == 40 && token.chars().all(|c| c.is_ascii_hexdigit()) {
                // Jeton d'API REST Django / Paperless-ngx
                format!("Token {}", token)
            } else {
                format!("Bearer {}", token)
            };
            req = req.header(reqwest::header::AUTHORIZATION, auth_header);
        }

        let mut resp = req
            .send()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to fetch URL: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let hint = if status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED {
                " (Authentification requise : fournissez un token via ?token=... ou les identifiants dans l'URL)"
            } else {
                ""
            };
            return Err((
                StatusCode::BAD_GATEWAY,
                format!("Remote URL returned status {}{}", status, hint),
            ));
        }

        let mut filename = params.filename.clone().unwrap_or_else(|| {
            // Extrait le nom de fichier depuis l'URL en repli (fallback)
            doc_url
                .rsplit('?')
                .next()
                .unwrap_or(doc_url)
                .rsplit('/')
                .next()
                .filter(|s| s.contains('.'))
                .unwrap_or("remote_document.pdf")
                .to_string()
        });

        // Tente d'extraire le nom réel depuis l'en-tête Content-Disposition
        if let Some(cd) = resp.headers().get(reqwest::header::CONTENT_DISPOSITION) {
            if let Ok(cd_str) = cd.to_str() {
                if let Some(fn_part) = cd_str.split("filename=").nth(1) {
                    let first_chunk = fn_part.split(';').next().unwrap_or("").trim();
                    let clean_fn = first_chunk
                        .trim_matches('"')
                        .trim_matches('\'')
                        .replace(['"', '\'', '\r', '\n'], "");
                    if !clean_fn.is_empty() {
                        filename = clean_fn;
                    }
                }
            }
        }

        filename = crate::security::sanitize_filename(&filename);

        // Téléchargement borné en flux continu pour éviter le déni de service par épuisement mémoire (OOM) ou bombe de décompression
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))? {
            if bytes.len() + chunk.len() > 100 * 1024 * 1024 {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Le document distant dépasse la taille maximale autorisée (100 Mo)".to_string(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        let (doc_id, path) = state
            .storage
            .save_document(&filename, &bytes)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        return Ok(Json(meta));
    }

    // Cas 2 : Connecteur nommé (S3, CMIS, Filesystem)
    let connector_name = params.connector.unwrap_or_else(|| "filesystem".to_string());
    let resource_id = params
        .id
        .ok_or((StatusCode::BAD_REQUEST, "Paramètre 'id' manquant".to_string()))?;

    if resource_id.contains("..") || resource_id.contains('\0') {
        return Err((StatusCode::BAD_REQUEST, "Tentative de traversée de chemin dans 'id'".to_string()));
    }

    let mut extra = HashMap::new();
    if let Some(f) = params.filename {
        extra.insert("filename".to_string(), f);
    }
    if let Some(t) = params.token {
        extra.insert("ticket".to_string(), t);
    }
    if let Some(u) = params.url {
        extra.insert("cmis_url".to_string(), u);
    }

    let doc_ref = DocumentReference {
        connector: connector_name,
        resource_id,
        extra_params: extra,
    };

    let doc_id = state
        .connectors
        .open_document(&doc_ref, &ctx, &state.storage)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document storage error".to_string()))?;

    let filename = state.storage.get_original_filename(&doc_id);
    let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(meta))
}

async fn upload_document(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<DocumentMetadata>, (StatusCode, String)> {
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or_default().to_string();
                if name == "file" || name == "document" || name.is_empty() {
                    let raw_fn = field
                        .file_name()
                        .unwrap_or("document.pdf")
                        .to_string();
                    let filename = crate::security::sanitize_filename(&raw_fn);
                    let data = match field.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            error!("Failed to read multipart field bytes: {}", e);
                            return Err((StatusCode::BAD_REQUEST, format!("Erreur lecture fichier: {}", e)));
                        }
                    };

                    info!("Received file upload: {} ({} bytes)", filename, data.len());

                    let (doc_id, path) = state
                        .storage
                        .save_document(&filename, &data)
                        .map_err(|e| {
                            error!("Failed to save uploaded document {}: {}", filename, e);
                            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                        })?;

                    let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
                        .map_err(|e| {
                            error!("Failed to generate metadata for {}: {}", filename, e);
                            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                        })?;

                    info!("Document uploaded and processed successfully: doc_id={}", doc_id);
                    return Ok(Json(meta));
                }
            }
            Ok(None) => break,
            Err(e) => {
                error!("Multipart parsing error: {}", e);
                return Err((StatusCode::BAD_REQUEST, format!("Multipart error: {}", e)));
            }
        }
    }

    warn!("Upload request missing 'file' or 'document' field");
    Err((StatusCode::BAD_REQUEST, "Champ 'file' ou 'document' manquant".to_string()))
}

async fn get_document_metadata(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<DocumentMetadata>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let filename = state.storage.get_original_filename(&doc_id);
    let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(meta))
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref() {
        None | Some("") | Some("undefined") | Some("null") => Ok(None),
        Some(s) => s.parse::<f64>().map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize)]
struct RenderParams {
    dpi: Option<u32>,
    watermark: Option<String>,
    format: Option<String>,
    layers: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    wc: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    ww: Option<f64>,
}

async fn render_page(
    State(state): State<AppState>,
    Path((doc_id, page)): Path<(String, usize)>,
    Query(params): Query<RenderParams>,
) -> Result<Response, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if page == 0 || page > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }

    let dpi = params.dpi.unwrap_or(state.config.default_dpi).clamp(10, 600);
    let variant = params.watermark.as_deref().unwrap_or("orig");
    let req_format = params.format.as_deref();
    let layers_str = params.layers.as_deref().unwrap_or("");
    let wc_val = params.wc.map(|v| v.to_string()).unwrap_or_default();
    let ww_val = params.ww.map(|v| v.to_string()).unwrap_or_default();
    let cache_key = CacheManager::compute_key(&doc_id, page, dpi, &format!("{}:{}:{}:{}:{}", variant, req_format.unwrap_or("default"), layers_str, wc_val, ww_val));

    if let Some(cached_data) = state.cache.get(&cache_key).await {
        let content_type = if cached_data.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else {
            "image/png"
        };
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "public, max-age=86400")
            .body(Body::from(cached_data))
            .unwrap());
    }

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    // 1. Fenêtrage dynamique DICOM (pour scanners CT/IRM bruts non compressés avec unités Hounsfield)
    if (ext == "dcm" || ext == "dicom") && (params.wc.is_some() || params.ww.is_some()) {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(dataset) = crate::engine::converter::dicom::DicomConverter::parse_dicom(&bytes) {
                if !dataset.raw_pixels.is_empty() {
                    let gray_img = crate::engine::converter::dicom::DicomConverter::render_image(&dataset, params.wc, params.ww);
                    let mut buf = Vec::new();
                    let mut cursor = std::io::Cursor::new(&mut buf);
                    if gray_img.write_to(&mut cursor, image::ImageFormat::Jpeg).is_ok() {
                        state.cache.set(&cache_key, buf.clone()).await;
                        return Ok(Response::builder()
                            .header(header::CONTENT_TYPE, "image/jpeg")
                            .header(header::CACHE_CONTROL, "public, max-age=86400")
                            .body(Body::from(buf))
                            .unwrap());
                    }
                }
            }
        }
    }

    // 2. Filtrage dynamique des calques CAO/DAO
    if (ext == "dxf" || ext == "dwg") && params.layers.is_some() {
        if let Some(ref l_str) = params.layers {
            let active_set: std::collections::HashSet<String> = l_str.split(',').map(|s| s.trim().to_string()).collect();
            if let Ok(temp_pdf) = tempfile::Builder::new().suffix(".pdf").tempfile() {
                if crate::engine::converter::cad::CadConverter::convert_cad_to_pdf(&path, temp_pdf.path(), Some(&active_set)).is_ok() {
                    if let Ok(img) = PdfEngine::render_page_with_format(temp_pdf.path(), page, dpi, req_format) {
                        state.cache.set(&cache_key, img.clone()).await;
                        return Ok(Response::builder()
                            .header(header::CONTENT_TYPE, "image/jpeg")
                            .header(header::CACHE_CONTROL, "public, max-age=86400")
                            .body(Body::from(img))
                            .unwrap());
                    }
                }
            }
        }
    }

    let fmt_clone = params.format.clone();
    let image_bytes = tokio::task::spawn_blocking(move || {
        PdfEngine::render_page_with_format(&path, page, dpi, fmt_clone.as_deref())
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.cache.set(&cache_key, image_bytes.clone()).await;

    let content_type = if image_bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else {
        "image/png"
    };

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(image_bytes))
        .unwrap())
}

async fn get_cad_layers(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<Vec<crate::models::CadLayerInfo>>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let layers = crate::engine::converter::cad::CadConverter::extract_layers(&path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(layers))
}

async fn get_dicom_metadata(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<crate::models::DicomMetadata>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let meta = crate::engine::converter::dicom::DicomConverter::extract_metadata(&path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(meta))
}

async fn render_thumbnail(
    State(state): State<AppState>,
    Path((doc_id, page)): Path<(String, usize)>,
) -> Result<Response, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if page == 0 || page > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }

    let dpi = 36;
    let cache_key = CacheManager::compute_key(&doc_id, page, dpi, "thumb");

    if let Some(cached_data) = state.cache.get(&cache_key).await {
        let content_type = if cached_data.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else {
            "image/png"
        };
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "public, max-age=86400")
            .body(Body::from(cached_data))
            .unwrap());
    }

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let image_bytes = tokio::task::spawn_blocking(move || {
        PdfEngine::render_page(&path, page, dpi)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.cache.set(&cache_key, image_bytes.clone()).await;

    let content_type = if image_bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else {
        "image/png"
    };

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(image_bytes))
        .unwrap())
}

async fn get_page_text(
    State(state): State<AppState>,
    Path((doc_id, page)): Path<(String, usize)>,
) -> Result<Json<crate::models::PageText>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if page == 0 || page > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let text_data = tokio::task::spawn_blocking(move || {
        PdfEngine::get_page_text(&path, page)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(text_data))
}

async fn get_annotations(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<Vec<Annotation>>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if let Some(xfdf) = state.storage.get_annotations_xfdf(&doc_id) {
        let annots = AnnotationEngine::from_xfdf(&xfdf);
        return Ok(Json(annots));
    }
    Ok(Json(vec![]))
}

async fn save_annotations(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Json(annots): Json<Vec<Annotation>>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if annots.len() > 10000 {
        return Err((StatusCode::BAD_REQUEST, "Nombre d'annotations excessif (max 10000)".to_string()));
    }

    let xfdf_content = AnnotationEngine::to_xfdf(&annots);
    state
        .storage
        .save_annotations_xfdf(&doc_id, &xfdf_content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Diffusion de la mise à jour à tous les collaborateurs connectés (WebSockets / SSE)
    let event_payload = serde_json::json!({
        "type": "ANNOTATIONS_UPDATED",
        "doc_id": doc_id,
        "count": annots.len(),
    }).to_string();
    let _ = state.collab_tx.send(event_payload);

    // Propagation asynchrone vers le connecteur GED/ECM distant si configuré
    let ctx = SecurityContext::default();
    let _ = state
        .connectors
        .sync_annotations(&doc_id, &xfdf_content, &ctx, &state.storage)
        .await;

    Ok(StatusCode::OK)
}

async fn collab_stream(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    if crate::security::validate_doc_id(&doc_id).is_err() {
        return Json(serde_json::json!({ "error": "Invalid document ID" }));
    }

    let annots_count = state
        .storage
        .get_annotations_xfdf(&doc_id)
        .map(|x| AnnotationEngine::from_xfdf(&x).len())
        .unwrap_or(0);

    Json(serde_json::json!({
        "status": "connected",
        "doc_id": doc_id,
        "active_annotations": annots_count,
        "node_id": state.config.node_id,
    }))
}

async fn redact_document(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Json(order): Json<RedactionOrder>,
) -> Result<Json<DocumentMetadata>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if order.items.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "Nombre d'éléments de caviardage excessif (max 2000)".to_string()));
    }

    let source_path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let orig_name = state.storage.get_original_filename(&doc_id);
    let target_filename = if orig_name.to_lowercase().ends_with(".pdf") {
        format!("redacted_{}", orig_name)
    } else {
        format!("redacted_{}.pdf", orig_name)
    };
    let temp_dir = tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let temp_output = temp_dir.path().join("redacted.pdf");

    RedactionEngine::apply_redaction(&source_path, &order, &temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let redacted_bytes = std::fs::read(&temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (saved_id, saved_path) = state
        .storage
        .save_document(&target_filename, &redacted_bytes)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let meta = PdfEngine::get_metadata(&saved_id, &target_filename, &saved_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(meta))
}

async fn sign_document(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Json(req): Json<SignRequest>,
) -> Result<Json<SignResponse>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if req.page_number == 0 || req.page_number > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }

    let source_path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(&source_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let effective_path = &rendition.effective_path;

    let orig_name = state.storage.get_original_filename(&doc_id);
    let target_filename = if orig_name.to_lowercase().ends_with(".pdf") {
        format!("signed_{}", orig_name)
    } else {
        format!("signed_{}.pdf", orig_name)
    };

    let temp_dir = tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let temp_output = temp_dir.path().join("signed.pdf");

    let mut sign_res = DigitalSignatureEngine::sign_pdf(effective_path, &temp_output, &req)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let signed_bytes = std::fs::read(&temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (saved_id, _saved_path) = state
        .storage
        .save_document(&target_filename, &signed_bytes)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sign_res.signed_doc_id = saved_id;

    Ok(Json(sign_res))
}

async fn build_document(
    State(state): State<AppState>,
    Json(order): Json<DocumentBuildOrder>,
) -> Result<Json<DocumentMetadata>, (StatusCode, String)> {
    if order.source_document_ids.is_empty() || order.source_document_ids.len() > 50 {
        return Err((StatusCode::BAD_REQUEST, "Le nombre de documents sources doit être compris entre 1 et 50".to_string()));
    }
    if order.page_actions.len() > 5000 {
        return Err((StatusCode::BAD_REQUEST, "Nombre d'actions de page excessif (max 5000)".to_string()));
    }

    let mut source_paths = Vec::new();
    for id in &order.source_document_ids {
        crate::security::validate_doc_id(id)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        if let Some(path) = state.storage.get_document_path(id) {
            source_paths.push(path);
        } else {
            return Err((StatusCode::NOT_FOUND, format!("Document {} not found", id)));
        }
    }

    let temp_dir = tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let temp_output = temp_dir.path().join("built.pdf");

    DocumentBuilderEngine::execute_build(&source_paths, &order, &temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let built_bytes = std::fs::read(&temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (saved_id, saved_path) = state
        .storage
        .save_document("assembled_document.pdf", &built_bytes)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let meta = PdfEngine::get_metadata(&saved_id, "assembled_document.pdf", &saved_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(meta))
}

async fn compare_documents(
    State(state): State<AppState>,
    Json(req): Json<CompareRequest>,
) -> Result<Response, (StatusCode, String)> {
    crate::security::validate_doc_id(&req.doc_a_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::security::validate_doc_id(&req.doc_b_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if req.page_number == 0 || req.page_number > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }
    let path_a = state
        .storage
        .get_document_path(&req.doc_a_id)
        .ok_or((StatusCode::NOT_FOUND, "Document A not found".to_string()))?;

    let path_b = state
        .storage
        .get_document_path(&req.doc_b_id)
        .ok_or((StatusCode::NOT_FOUND, "Document B not found".to_string()))?;

    if req.mode == "text" {
        let text_res = ComparisonEngine::compare_text_pages(&path_a, &path_b, req.page_number)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let json_body = serde_json::to_string(&text_res)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Diff-Ratio", text_res.diff_ratio.to_string())
            .header(
                "X-Has-Differences",
                (text_res.additions_count + text_res.deletions_count > 0).to_string(),
            )
            .body(Body::from(json_body))
            .unwrap());
    }

    let result = ComparisonEngine::compare_pages(&path_a, &path_b, req.page_number, 150)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "image/png")
        .header("X-Diff-Ratio", result.diff_ratio.to_string())
        .header("X-Has-Differences", result.has_differences.to_string())
        .body(Body::from(result.diff_image_png))
        .unwrap())
}

async fn compare_documents_text(
    State(state): State<AppState>,
    Json(req): Json<CompareRequest>,
) -> Result<Json<TextDiffResult>, (StatusCode, String)> {
    crate::security::validate_doc_id(&req.doc_a_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::security::validate_doc_id(&req.doc_b_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if req.page_number == 0 || req.page_number > 50000 {
        return Err((StatusCode::BAD_REQUEST, "Numéro de page invalide".to_string()));
    }

    let path_a = state
        .storage
        .get_document_path(&req.doc_a_id)
        .ok_or((StatusCode::NOT_FOUND, "Document A not found".to_string()))?;

    let path_b = state
        .storage
        .get_document_path(&req.doc_b_id)
        .ok_or((StatusCode::NOT_FOUND, "Document B not found".to_string()))?;

    let result = ComparisonEngine::compare_text_pages(&path_a, &path_b, req.page_number)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct DownloadParams {
    watermark: Option<String>,
}

async fn download_document(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Query(params): Query<DownloadParams>,
    headers: axum::http::HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Contrôle RBAC : vérifie si l'utilisateur dispose de la permission de téléchargement
    if let Some(user_perms) = headers.get("X-User-Permissions").and_then(|v| v.to_str().ok()) {
        if user_perms.contains("no-download")
            || (user_perms.contains("read-only") && !user_perms.contains("download"))
        {
            return Err((
                StatusCode::FORBIDDEN,
                "Téléchargement interdit par la politique de sécurité (RBAC)".to_string(),
            ));
        }
    }

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let filename = state.storage.get_original_filename(&doc_id);

    // Applique un filigrane dynamique de sécurité si demandé
    let (data, out_filename) = if let Some(ref wm_text) = params.watermark {
        let trimmed = wm_text.trim();
        if !trimmed.is_empty() {
            let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(&path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut doc = lopdf::Document::load(&rendition.effective_path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let wm_opts = WatermarkOptions {
                text: trimmed.to_string(),
                opacity: 0.20,
                font_size: 32.0,
                rotation: 45.0,
                color: "#CC0000".to_string(),
            };
            crate::engine::builder::DocumentBuilderEngine::apply_watermark(&mut doc, &wm_opts)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut buf = Vec::new();
            doc.save_to(&mut buf)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let base_name = filename.strip_suffix(".pdf").unwrap_or(&filename);
            (buf, format!("{}_filigrane.pdf", base_name))
        } else {
            (
                std::fs::read(&path)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
                filename,
            )
        }
    } else {
        (
            std::fs::read(&path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
            filename,
        )
    };

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let content_type = match ext.as_str() {
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" | "ogg" => "video/ogg",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "dxf" => "application/dxf",
        "dwg" => "image/vnd.dwg",
        "dcm" | "dicom" => "application/dicom",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", out_filename),
        )
        .body(Body::from(data))
        .unwrap())
}

async fn stream_document_content(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    req: axum::extract::Request,
) -> Result<Response, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let service = tower_http::services::ServeFile::new(&path);
    use tower::ServiceExt;
    match service.oneshot(req).await {
        Ok(res) => Ok(res.into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Deserialize)]
struct SearchQueryParams {
    q: Option<String>,
}

async fn scan_document_pii(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<PiiScanResponse>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let filename = state.storage.get_original_filename(&doc_id);
    let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let scan_res = PiiEngine::scan_document(&path, meta.page_count)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(scan_res))
}

async fn search_document_text(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Query(params): Query<SearchQueryParams>,
) -> Result<Json<GlobalSearchResponse>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let query = params.q.unwrap_or_default();
    if query.len() > 500 {
        return Err((StatusCode::BAD_REQUEST, "Requête de recherche trop longue (max 500 caractères)".to_string()));
    }

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let filename = state.storage.get_original_filename(&doc_id);
    let meta = PdfEngine::get_metadata(&doc_id, &filename, &path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let res = PiiEngine::search_document(&path, meta.page_count, &query)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(res))
}

async fn get_document_forms(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Result<Json<FormFieldsSummary>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let forms = FormEngine::extract_form_fields(&path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(forms))
}

async fn fill_document_forms(
    State(state): State<AppState>,
    Path(doc_id): Path<String>,
    Json(req): Json<FormFillRequest>,
) -> Result<Json<FormFillResponse>, (StatusCode, String)> {
    crate::security::validate_doc_id(&doc_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if req.values.len() > 1000 {
        return Err((StatusCode::BAD_REQUEST, "Nombre excessif de champs de formulaire (max 1000)".to_string()));
    }

    let source_path = state
        .storage
        .get_document_path(&doc_id)
        .ok_or((StatusCode::NOT_FOUND, "Document not found".to_string()))?;

    let orig_name = state.storage.get_original_filename(&doc_id);

    let temp_dir = tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let temp_output = temp_dir.path().join("filled.pdf");

    let updated_count = FormEngine::fill_form_fields(&source_path, &temp_output, &req.values)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let filled_bytes = std::fs::read(&temp_output)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let target_doc_id = if req.save_as_new {
        let new_name = format!("filled_{}", orig_name);
        let (saved_id, _) = state
            .storage
            .save_document(&new_name, &filled_bytes)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        saved_id
    } else {
        std::fs::write(&source_path, &filled_bytes)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        doc_id.clone()
    };

    // Invalidation des caches pour forcer le rafraîchissement visuel des champs modifiés
    state.cache.invalidate_document(&target_doc_id).await;

    Ok(Json(FormFillResponse {
        document_id: target_doc_id,
        updated_fields_count: updated_count,
        message: format!("Successfully updated {} form field(s)", updated_count),
    }))
}

#[derive(Deserialize, Debug, Default)]
struct ConvertQueryParams {
    watermark: Option<String>,
    ephemeral: Option<bool>,
}

async fn convert_document_oneshot(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Query(params): Query<ConvertQueryParams>,
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, String)> {
    // 1. Extraction de la clé d'API et vérification des quotas horaires
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|auth_header| {
                    if auth_header.starts_with("Bearer ") {
                        Some(auth_header.trim_start_matches("Bearer ").trim().to_string())
                    } else {
                        None
                    }
                })
        });

    let quota_info = match state.auth.check_and_consume(api_key.as_deref()).await {
        Ok(info) => info,
        Err(QuotaError::InvalidKey) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                "Clé API invalide ou manquante (En-tête 'X-API-Key' ou 'Authorization: Bearer <key>' requis)".to_string(),
            ));
        }
        Err(QuotaError::QuotaExceeded { limit, reset_epoch_secs }) => {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "Quota d'appels de conversion dépassé (Limite: {} req/heure, réinitialisation à {} epoch)",
                    limit, reset_epoch_secs
                ),
            ));
        }
        Err(QuotaError::BackendError(msg)) => {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Erreur système de quota: {}", msg)));
        }
    };

    // 2. Lecture du document transmis via multipart/form-data
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name == "document" {
            let filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "document.docx".to_string());
            let data = field
                .bytes()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Erreur de lecture du fichier: {}", e)))?;
            file_bytes = Some(data.to_vec());
            original_filename = Some(filename);
            break;
        }
    }

    let data = match file_bytes {
        Some(b) if !b.is_empty() => b,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Champ 'file' ou 'document' manquant ou vide dans la requête multipart".to_string(),
            ));
        }
    };

    let filename = original_filename.unwrap_or_else(|| "document.docx".to_string());

    // 3. Enregistrement temporaire pour le pipeline de conversion
    let (doc_id, source_path) = state
        .storage
        .save_document(&filename, &data)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Erreur de stockage: {}", e)))?;

    // 4. Conversion du document source en PDF
    let rendition = match crate::engine::converter::FormatConverter::ensure_pdf_rendition(&source_path) {
        Ok(res) => res,
        Err(e) => {
            // Purge immédiate en cas d'échec pour les documents éphémères
            let is_ephemeral = params.ephemeral.unwrap_or(true);
            if is_ephemeral {
                state.storage.delete_document(&doc_id);
            }
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Échec de conversion en PDF pour '{}': {}", filename, e),
            ));
        }
    };

    let effective_pdf_path = rendition.effective_path;

    // 5. Application dynamique d'un filigrane de sécurité si demandé via query param ou en-tête X-Watermark
    let watermark_text = params.watermark.or_else(|| {
        headers
            .get("X-Watermark")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });

    let (final_pdf_bytes, output_filename) = if let Some(ref wm_text) = watermark_text {
        let trimmed = wm_text.trim();
        if !trimmed.is_empty() {
            let mut doc = lopdf::Document::load(&effective_pdf_path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let wm_opts = WatermarkOptions {
                text: trimmed.to_string(),
                opacity: 0.20,
                font_size: 32.0,
                rotation: 45.0,
                color: "#CC0000".to_string(),
            };
            crate::engine::builder::DocumentBuilderEngine::apply_watermark(&mut doc, &wm_opts)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let mut buf = Vec::new();
            doc.save_to(&mut buf)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            let stem = std::path::Path::new(&filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            (buf, format!("{}_converted.pdf", stem))
        } else {
            let bytes = std::fs::read(&effective_pdf_path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let stem = std::path::Path::new(&filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            (bytes, format!("{}.pdf", stem))
        }
    } else {
        let bytes = std::fs::read(&effective_pdf_path)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let stem = std::path::Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        (bytes, format!("{}.pdf", stem))
    };

    // 6. Purge immédiate si mode éphémère activé (par défaut = true pour garantir le secret des données)
    let is_ephemeral = params.ephemeral.unwrap_or(true);
    if is_ephemeral {
        state.storage.delete_document(&doc_id);
    }

    info!(
        "One-shot conversion successful for '{}' -> '{}' (key='{}', remaining={})",
        filename, output_filename, quota_info.key, quota_info.remaining
    );

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", output_filename),
        )
        .header("X-RateLimit-Limit", quota_info.limit.to_string())
        .header("X-RateLimit-Remaining", quota_info.remaining.to_string())
        .header("X-RateLimit-Reset", quota_info.reset_epoch_secs.to_string())
        .header("X-Converted-By", concat!("Oxid-Converter/", env!("CARGO_PKG_VERSION")))
        .body(Body::from(final_pdf_bytes))
        .unwrap())
}

