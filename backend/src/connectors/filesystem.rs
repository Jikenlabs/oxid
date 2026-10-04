use super::traits::{DocumentConnector, DocumentPayload, DocumentReference, SecurityContext};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

pub struct FilesystemConnector {
    base_dir: PathBuf,
}

impl FilesystemConnector {
    pub fn new(base_dir: PathBuf) -> Self {
        if !base_dir.exists() {
            let _ = fs::create_dir_all(&base_dir);
        }
        Self { base_dir }
    }

    fn resolve_path(&self, resource_id: &str) -> Result<PathBuf> {
        crate::security::safe_join_path(&self.base_dir, resource_id)
    }
}

#[async_trait]
impl DocumentConnector for FilesystemConnector {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    async fn fetch_document(
        &self,
        doc_ref: &DocumentReference,
        _ctx: &SecurityContext,
    ) -> Result<DocumentPayload> {
        let path = self.resolve_path(&doc_ref.resource_id)?;
        if !path.exists() {
            bail!("File not found in filesystem connector: {}", path.display());
        }

        let data = fs::read(&path).context("Failed to read file from filesystem")?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("pdf")
            .to_lowercase();
        let mime_type = match ext.as_str() {
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        }
        .to_string();

        Ok(DocumentPayload {
            filename,
            data,
            mime_type,
            etag: None,
        })
    }

    async fn fetch_annotations(
        &self,
        doc_ref: &DocumentReference,
        _ctx: &SecurityContext,
    ) -> Result<Option<String>> {
        let path = self.resolve_path(&doc_ref.resource_id)?;
        let xfdf_path = path.with_extension("xfdf");
        if xfdf_path.exists() {
            let content = fs::read_to_string(xfdf_path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    async fn save_annotations(
        &self,
        doc_ref: &DocumentReference,
        xfdf: &str,
        _ctx: &SecurityContext,
    ) -> Result<()> {
        let path = self.resolve_path(&doc_ref.resource_id)?;
        let xfdf_path = path.with_extension("xfdf");
        fs::write(xfdf_path, xfdf).context("Failed to save XFDF file")?;
        Ok(())
    }

    async fn save_new_version(
        &self,
        doc_ref: &DocumentReference,
        data: &[u8],
        filename: &str,
        _ctx: &SecurityContext,
    ) -> Result<String> {
        let path = self.resolve_path(&doc_ref.resource_id)?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let new_path = parent.join(filename);
        fs::write(&new_path, data).context("Failed to write new version")?;
        Ok(new_path.to_string_lossy().to_string())
    }
}
