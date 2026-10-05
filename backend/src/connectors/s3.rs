use super::traits::{DocumentConnector, DocumentPayload, DocumentReference, SecurityContext};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::header::AUTHORIZATION;
use std::env;

pub struct S3Connector {
    endpoint: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    client: reqwest::Client,
}

impl S3Connector {
    pub fn new() -> Self {
        let endpoint = env::var("OXID_S3_ENDPOINT")
            .unwrap_or_else(|_| "https://s3.amazonaws.com".to_string())
            .trim_end_matches('/')
            .to_string();

        let access_key = env::var("OXID_S3_ACCESS_KEY").ok();
        let secret_key = env::var("OXID_S3_SECRET_KEY").ok();

        Self {
            endpoint,
            access_key,
            secret_key,
            client: reqwest::Client::builder().build().unwrap(),
        }
    }

    fn split_bucket_key(&self, resource_id: &str) -> Result<(String, String)> {
        let clean = resource_id.trim_start_matches('/');
        if let Some((bucket, key)) = clean.split_once('/') {
            Ok((bucket.to_string(), key.to_string()))
        } else {
            bail!("Invalid S3 resource_id format, expected 'bucket/key'");
        }
    }

    fn build_url(&self, bucket: &str, key: &str) -> String {
        format!("{}/{}/{}", self.endpoint, bucket, key)
    }
}

#[async_trait]
impl DocumentConnector for S3Connector {
    fn name(&self) -> &'static str {
        "s3"
    }

    async fn fetch_document(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<DocumentPayload> {
        let (bucket, key) = self.split_bucket_key(&doc_ref.resource_id)?;
        let url = self.build_url(&bucket, &key);

        let mut req = self.client.get(&url);

        // Applique l'en-tête d'authentification si fourni dans le SecurityContext ou l'environnement
        if let Some(ref token) = ctx.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = req
            .send()
            .await
            .context(format!("Failed to connect to S3 endpoint: {}", url))?;

        if !resp.status().is_success() {
            bail!("S3 fetch failed with HTTP status: {}", resp.status());
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"').to_string());

        let data = resp.bytes().await?.to_vec();
        let filename = key
            .rsplit('/')
            .next()
            .unwrap_or("document.pdf")
            .to_string();

        let mime_type = if filename.ends_with(".pdf") {
            "application/pdf"
        } else if filename.ends_with(".png") {
            "image/png"
        } else {
            "application/octet-stream"
        }
        .to_string();

        Ok(DocumentPayload {
            filename,
            data,
            mime_type,
            etag,
        })
    }

    async fn fetch_annotations(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<Option<String>> {
        let (bucket, key) = self.split_bucket_key(&doc_ref.resource_id)?;
        let xfdf_key = format!("{}.xfdf", key);
        let url = self.build_url(&bucket, &xfdf_key);

        let mut req = self.client.get(&url);
        if let Some(ref token) = ctx.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = req.send().await?;
        if resp.status().is_success() {
            let xfdf = resp.text().await?;
            Ok(Some(xfdf))
        } else {
            Ok(None)
        }
    }

    async fn save_annotations(
        &self,
        doc_ref: &DocumentReference,
        xfdf: &str,
        ctx: &SecurityContext,
    ) -> Result<()> {
        let (bucket, key) = self.split_bucket_key(&doc_ref.resource_id)?;
        let xfdf_key = format!("{}.xfdf", key);
        let url = self.build_url(&bucket, &xfdf_key);

        let mut req = self
            .client
            .put(&url)
            .header("Content-Type", "application/vnd.adobe.xfdf")
            .body(xfdf.to_string());

        if let Some(ref token) = ctx.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            bail!("Failed to save XFDF to S3: {}", resp.status());
        }

        Ok(())
    }

    async fn save_new_version(
        &self,
        doc_ref: &DocumentReference,
        data: &[u8],
        filename: &str,
        ctx: &SecurityContext,
    ) -> Result<String> {
        let (bucket, key) = self.split_bucket_key(&doc_ref.resource_id)?;
        let parent_key = key.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let new_key = if parent_key.is_empty() {
            filename.to_string()
        } else {
            format!("{}/{}", parent_key, filename)
        };

        let url = self.build_url(&bucket, &new_key);
        let mut req = self
            .client
            .put(&url)
            .header("Content-Type", "application/pdf")
            .body(data.to_vec());

        if let Some(ref token) = ctx.auth_token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            bail!("Failed to upload new version to S3: {}", resp.status());
        }

        Ok(format!("{}/{}", bucket, new_key))
    }
}
