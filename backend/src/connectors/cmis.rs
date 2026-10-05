use super::traits::{DocumentConnector, DocumentPayload, DocumentReference, SecurityContext};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::env;

pub struct CmisConnector {
    atom_pub_url: String,
    client: reqwest::Client,
}

impl CmisConnector {
    pub fn new() -> Self {
        let atom_pub_url = env::var("OXID_CMIS_URL")
            .or_else(|_| env::var("OXID_ALFRESCO_CMIS_URL"))
            .unwrap_or_else(|_| {
                "http://localhost:8082/cmis/atom11/test".to_string()
            });

        Self {
            atom_pub_url,
            client: reqwest::Client::builder().build().unwrap(),
        }
    }

    fn build_content_url_with_base(base_atom_url: &str, object_id: &str, ticket: Option<&str>) -> String {
        let base = if base_atom_url.ends_with('/') {
            format!("{}content", base_atom_url)
        } else {
            format!("{}/content", base_atom_url)
        };

        let encode = |s: &str| -> String {
            let mut res = String::new();
            for b in s.bytes() {
                if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                    res.push(b as char);
                } else {
                    res.push_str(&format!("%{:02X}", b));
                }
            }
            res
        };

        if let Some(t) = ticket {
            format!("{}?id={}&alf_ticket={}", base, encode(object_id), encode(t))
        } else {
            format!("{}?id={}", base, encode(object_id))
        }
    }

    fn build_content_url(&self, object_id: &str, ticket: Option<&str>) -> String {
        Self::build_content_url_with_base(&self.atom_pub_url, object_id, ticket)
    }
}

#[async_trait]
impl DocumentConnector for CmisConnector {
    fn name(&self) -> &'static str {
        "cmis"
    }

    async fn fetch_document(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<DocumentPayload> {
        let object_id = &doc_ref.resource_id;
        let ticket = ctx
            .auth_token
            .as_deref()
            .or_else(|| doc_ref.extra_params.get("ticket").map(|s| s.as_str()));

        let base_url = doc_ref
            .extra_params
            .get("cmis_url")
            .cloned()
            .unwrap_or_else(|| self.atom_pub_url.clone());
        let url = Self::build_content_url_with_base(&base_url, object_id, ticket);
        let mut req = self.client.get(&url);

        if let Some((ref u, ref p)) = ctx.basic_auth {
            req = req.basic_auth(u, Some(p));
        }

        let resp = req
            .send()
            .await
            .context(format!("Failed to connect to CMIS endpoint at {}", url))?;

        if !resp.status().is_success() {
            bail!("CMIS server returned HTTP status: {}", resp.status());
        }

        let filename = resp
            .headers()
            .get("content-disposition")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                if let Some(idx) = s.find("filename=") {
                    let raw = &s[idx + 9..];
                    let chunk = raw.split(';').next().unwrap_or("").trim();
                    Some(
                        chunk
                            .trim_matches('"')
                            .trim_matches('\'')
                            .replace(['"', '\'', '\r', '\n'], "")
                            .to_string(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                doc_ref
                    .extra_params
                    .get("filename")
                    .cloned()
                    .unwrap_or_else(|| "cmis_document.pdf".to_string())
            });

        let data = resp.bytes().await?.to_vec();

        Ok(DocumentPayload {
            filename,
            data,
            mime_type: "application/pdf".to_string(),
            etag: None,
        })
    }

    async fn fetch_annotations(
        &self,
        doc_ref: &DocumentReference,
        ctx: &SecurityContext,
    ) -> Result<Option<String>> {
        // Interroge le nœud enfant pour les annotations du document (chemins standard et hérité)
        let object_id = &doc_ref.resource_id;
        let ticket = ctx.auth_token.as_deref();

        // Essaie d'abord le chemin standard, puis le chemin hérité
        for path_suffix in &["annotations", "arender-annotations"] {
            let annot_child_id = format!("{}/{}", object_id, path_suffix);
            let url = self.build_content_url(&annot_child_id, ticket);
            let mut req = self.client.get(&url);
            if let Some((ref u, ref p)) = ctx.basic_auth {
                req = req.basic_auth(u, Some(p));
            }

            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    let xml = resp.text().await?;
                    return Ok(Some(xml));
                }
            }
        }

        Ok(None)
    }

    async fn save_annotations(
        &self,
        doc_ref: &DocumentReference,
        xfdf: &str,
        ctx: &SecurityContext,
    ) -> Result<()> {
        let object_id = &doc_ref.resource_id;
        let annot_child_id = format!("{}/annotations", object_id);
        let ticket = ctx.auth_token.as_deref();

        let url = self.build_content_url(&annot_child_id, ticket);
        let mut req = self
            .client
            .put(&url)
            .header("Content-Type", "application/vnd.adobe.xfdf")
            .body(xfdf.to_string());

        if let Some((ref u, ref p)) = ctx.basic_auth {
            req = req.basic_auth(u, Some(p));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            bail!("Failed to save annotations to CMIS node: {}", resp.status());
        }

        Ok(())
    }

    async fn save_new_version(
        &self,
        doc_ref: &DocumentReference,
        data: &[u8],
        _filename: &str,
        ctx: &SecurityContext,
    ) -> Result<String> {
        let object_id = &doc_ref.resource_id;
        let ticket = ctx.auth_token.as_deref();
        let url = self.build_content_url(object_id, ticket);

        let mut req = self
            .client
            .put(&url)
            .header("Content-Type", "application/pdf")
            .body(data.to_vec());

        if let Some((ref u, ref p)) = ctx.basic_auth {
            req = req.basic_auth(u, Some(p));
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            bail!("Failed to update CMIS version: {}", resp.status());
        }

        Ok(format!("{}:vNew", object_id))
    }
}
