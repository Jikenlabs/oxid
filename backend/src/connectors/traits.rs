use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SecurityContext {
    pub user_id: Option<String>,
    pub auth_token: Option<String>,
    pub basic_auth: Option<(String, String)>,
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentReference {
    pub connector: String,
    pub resource_id: String,
    pub extra_params: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct DocumentPayload {
    pub filename: String,
    pub data: Vec<u8>,
    pub mime_type: String,
    pub etag: Option<String>,
}

#[async_trait]
pub trait DocumentConnector: Send + Sync {
    fn name(&self) -> &'static str;

    async fn fetch_document(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<DocumentPayload>;

    async fn fetch_annotations(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<Option<String>>;

    async fn save_annotations(
        &self,
        doc_ref: &DocumentReference,
        xfdf: &str,
        ctx: &SecurityContext,
    ) -> Result<()>;

    async fn save_new_version(
        &self,
        doc_ref: &DocumentReference,
        data: &[u8],
        filename: &str,
        ctx: &SecurityContext,
    ) -> Result<String>;
}
