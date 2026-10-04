pub mod cmis;
pub mod filesystem;
pub mod s3;
pub mod traits;

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use traits::{DocumentConnector, DocumentPayload, DocumentReference, SecurityContext};
use uuid::Uuid;

#[derive(Clone)]
pub struct DocumentStorage {
    storage_dir: PathBuf,
}

impl DocumentStorage {
    pub fn new(storage_dir: PathBuf) -> Self {
        if !storage_dir.exists() {
            let _ = fs::create_dir_all(&storage_dir);
        }
        Self { storage_dir }
    }

    pub fn save_document(&self, filename: &str, data: &[u8]) -> Result<(String, PathBuf)> {
        use sha2::{Digest, Sha256};
        let sanitized_name = crate::security::sanitize_filename(filename);

        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash_hex = hex::encode(hasher.finalize());
        let hash_prefix = &hash_hex[..16]; // 16 caractères uniques

        let extension = Path::new(&sanitized_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("pdf");

        // Déduplication de contenu : si le même fichier exact est renvoyé, on réutilise le fichier existant
        if let Ok(entries) = fs::read_dir(&self.storage_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(hash_prefix) && name.ends_with(extension) {
                    let existing_doc_id = name.trim_end_matches(&format!(".{}", extension)).to_string();
                    let existing_path = entry.path();
                    let meta_path = self.storage_dir.join(format!("{}.meta", existing_doc_id));
                    let _ = fs::write(&meta_path, format!("{}\n{}", sanitized_name, extension));
                    return Ok((existing_doc_id, existing_path));
                }
            }
        }

        let doc_id = format!("{}-{}", hash_prefix, &Uuid::new_v4().to_string()[..8]);
        let stored_filename = format!("{}.{}", doc_id, extension);
        let path = self.storage_dir.join(&stored_filename);

        fs::write(&path, data).context("Failed to write document file")?;

        let meta_path = self.storage_dir.join(format!("{}.meta", doc_id));
        let meta_content = format!("{}\n{}", sanitized_name, extension);
        let _ = fs::write(&meta_path, meta_content);

        Ok((doc_id, path))
    }

    pub fn get_document_path(&self, doc_id: &str) -> Option<PathBuf> {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return None;
        }

        let meta_path = self.storage_dir.join(format!("{}.meta", doc_id));
        if let Ok(content) = fs::read_to_string(&meta_path) {
            let mut lines = content.lines();
            let _filename = lines.next();
            if let Some(ext) = lines.next() {
                let primary_path = self.storage_dir.join(format!("{}.{}", doc_id, ext));
                if primary_path.exists() {
                    if let (Ok(can_base), Ok(can_path)) = (self.storage_dir.canonicalize(), primary_path.canonicalize()) {
                        if can_path.starts_with(&can_base) {
                            return Some(primary_path);
                        }
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(&self.storage_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Match exact stem (e.g. "doc_id.pdf" or "doc_id.ext"), not arbitrary prefixes
                if (name == format!("{}.pdf", doc_id) || name.starts_with(&format!("{}.", doc_id)))
                    && !name.ends_with(".meta")
                    && !name.ends_with(".xfdf")
                    && !name.ends_with(".connector")
                    && !name.contains(".rendition.")
                {
                    let p = entry.path();
                    if let (Ok(can_base), Ok(can_path)) = (self.storage_dir.canonicalize(), p.canonicalize()) {
                        if can_path.starts_with(&can_base) {
                            return Some(p);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_original_filename(&self, doc_id: &str) -> String {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return "document.pdf".to_string();
        }
        let meta_path = self.storage_dir.join(format!("{}.meta", doc_id));
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Some(first_line) = content.lines().next() {
                return crate::security::sanitize_filename(first_line);
            }
        }
        format!("{}.pdf", doc_id)
    }

    pub fn save_annotations_xfdf(&self, doc_id: &str, xfdf_content: &str) -> Result<()> {
        crate::security::validate_doc_id(doc_id)?;
        let xfdf_path = self.storage_dir.join(format!("{}.xfdf", doc_id));
        fs::write(xfdf_path, xfdf_content).context("Failed to save XFDF annotations")?;
        Ok(())
    }

    pub fn get_annotations_xfdf(&self, doc_id: &str) -> Option<String> {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return None;
        }
        let xfdf_path = self.storage_dir.join(format!("{}.xfdf", doc_id));
        if xfdf_path.exists() {
            fs::read_to_string(xfdf_path).ok()
        } else {
            None
        }
    }

    // Associate remote connector metadata
    pub fn save_connector_mapping(&self, doc_id: &str, doc_ref: &DocumentReference) {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return;
        }
        let path = self.storage_dir.join(format!("{}.connector", doc_id));
        if let Ok(json) = serde_json::to_string(doc_ref) {
            let _ = fs::write(path, json);
        }
    }

    pub fn get_connector_mapping(&self, doc_id: &str) -> Option<DocumentReference> {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return None;
        }
        let path = self.storage_dir.join(format!("{}.connector", doc_id));
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                return serde_json::from_str(&content).ok();
            }
        }
        None
    }

    pub fn delete_document(&self, doc_id: &str) {
        if crate::security::validate_doc_id(doc_id).is_err() {
            return;
        }
        let meta_path = self.storage_dir.join(format!("{}.meta", doc_id));
        let _ = fs::remove_file(&meta_path);
        let xfdf_path = self.storage_dir.join(format!("{}.xfdf", doc_id));
        let _ = fs::remove_file(&xfdf_path);
        let conn_path = self.storage_dir.join(format!("{}.connector", doc_id));
        let _ = fs::remove_file(&conn_path);

        if let Ok(entries) = fs::read_dir(&self.storage_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("{}.", doc_id)) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    pub fn cleanup_expired_documents(&self, max_age_secs: u64) -> usize {
        let now = std::time::SystemTime::now();
        let mut purged = 0;
        if let Ok(entries) = fs::read_dir(&self.storage_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(duration) = now.duration_since(modified) {
                            if duration.as_secs() > max_age_secs {
                                let path = entry.path();
                                let name = entry.file_name().to_string_lossy().to_string();
                                if name.ends_with(".meta") {
                                    let doc_id = name.trim_end_matches(".meta");
                                    self.delete_document(doc_id);
                                    purged += 1;
                                } else if name.contains(".rendition.") || name.ends_with(".tmp") {
                                    let _ = fs::remove_file(path);
                                    purged += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        purged
    }
}

pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn DocumentConnector>>,
}

impl ConnectorRegistry {
    pub fn new(storage_base: PathBuf) -> Self {
        let mut connectors: HashMap<String, Arc<dyn DocumentConnector>> = HashMap::new();

        connectors.insert(
            "filesystem".to_string(),
            Arc::new(filesystem::FilesystemConnector::new(storage_base)),
        );
        connectors.insert("s3".to_string(), Arc::new(s3::S3Connector::new()));
        connectors.insert("cmis".to_string(), Arc::new(cmis::CmisConnector::new()));

        Self { connectors }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn DocumentConnector>> {
        self.connectors.get(name).cloned()
    }

    pub async fn open_document(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
        storage: &DocumentStorage,
    ) -> Result<String> {
        let connector = self
            .get(&doc_ref.connector)
            .context(format!("Unknown connector: {}", doc_ref.connector))?;

        let payload = connector.fetch_document(doc_ref, ctx).await?;

        // Save into local caching storage
        let (doc_id, _path) = storage.save_document(&payload.filename, &payload.data)?;

        // Store connector mapping
        storage.save_connector_mapping(&doc_id, doc_ref);

        // Fetch remote annotations if present
        if let Ok(Some(xfdf)) = connector.fetch_annotations(doc_ref, ctx).await {
            let _ = storage.save_annotations_xfdf(&doc_id, &xfdf);
        }

        Ok(doc_id)
    }

    pub async fn sync_annotations(
        &self,
        doc_id: &str,
        xfdf: &str,
        ctx: &SecurityContext,
        storage: &DocumentStorage,
    ) -> Result<()> {
        if let Some(doc_ref) = storage.get_connector_mapping(doc_id) {
            if let Some(connector) = self.get(&doc_ref.connector) {
                connector.save_annotations(&doc_ref, xfdf, ctx).await?;
            }
        }
        Ok(())
    }
}
