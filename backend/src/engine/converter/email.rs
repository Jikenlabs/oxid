use crate::models::EmailAttachment;
use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use mail_parser::{MessageParser, MimeHeaders};
use std::fs;
use std::path::Path;

pub struct EmailConversionResult {
    pub attachments: Vec<EmailAttachment>,
}

pub struct EmailConverter;

impl EmailConverter {
    pub fn convert_eml_to_pdf(
        input_path: &Path,
        output_path: &Path,
    ) -> Result<EmailConversionResult> {
        let raw_bytes = fs::read(input_path).context("Failed to read email file")?;
        let message = MessageParser::default()
            .parse(&raw_bytes)
            .context("Failed to parse email message")?;

        let subject = message.subject().unwrap_or("(Sans objet)").to_string();
        let from = message
            .from()
            .map(|f| format!("{:?}", f))
            .unwrap_or_else(|| "Inconnu".to_string());
        let to = message
            .to()
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|| "".to_string());
        let date = message
            .date()
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| "".to_string());

        let mut attachments = Vec::new();
        for (idx, att) in message.attachments().enumerate() {
            let filename = att
                .attachment_name()
                .unwrap_or(&format!("attachment_{}", idx + 1))
                .to_string();
            let mime_type = att
                .content_type()
                .map(|ct| ct.c_type.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            attachments.push(EmailAttachment {
                id: format!("att_{}", idx + 1),
                filename,
                mime_type,
                size: att.contents().len(),
            });
        }

        let body_text = message
            .body_text(0)
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                message
                    .body_html(0)
                    .map(|h| h.replace("<br>", "\n").replace("</p>", "\n\n"))
                    .unwrap_or_default()
            });

        // Génération du document PDF
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_helv = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let font_helv_bold = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica-Bold",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_helv,
                "F2" => font_helv_bold,
            },
        });

        let mut page_objects = Vec::new();
        let lines: Vec<&str> = body_text.lines().collect();

        // Page 1 : En-tête + début du corps du message
        let mut stream_p1 = String::new();

        // Dessine un encadré gris clair pour l'en-tête du courriel
        stream_p1.push_str("q\n0.95 0.95 0.95 rg\n40 700 515 110 re\nf\nQ\n");

        stream_p1.push_str("BT\n");
        // Éléments de l'en-tête
        let clean = |s: &str| {
            crate::security::escape_pdf_str(s)
        };

        let mut hy = 790.0;
        stream_p1.push_str(&format!("/F2 10 Tf\n1 0 0 1 50 {:.2} Tm\n(DE: ) Tj\n", hy));
        stream_p1.push_str(&format!("/F1 10 Tf\n1 0 0 1 80 {:.2} Tm\n({}) Tj\n", hy, clean(&from)));

        hy -= 18.0;
        stream_p1.push_str(&format!("/F2 10 Tf\n1 0 0 1 50 {:.2} Tm\n(A: ) Tj\n", hy));
        stream_p1.push_str(&format!("/F1 10 Tf\n1 0 0 1 80 {:.2} Tm\n({}) Tj\n", hy, clean(&to)));

        hy -= 18.0;
        stream_p1.push_str(&format!("/F2 10 Tf\n1 0 0 1 50 {:.2} Tm\n(DATE: ) Tj\n", hy));
        stream_p1.push_str(&format!("/F1 10 Tf\n1 0 0 1 95 {:.2} Tm\n({}) Tj\n", hy, clean(&date)));

        hy -= 18.0;
        stream_p1.push_str(&format!("/F2 10 Tf\n1 0 0 1 50 {:.2} Tm\n(OBJET: ) Tj\n", hy));
        stream_p1.push_str(&format!("/F2 11 Tf\n1 0 0 1 100 {:.2} Tm\n({}) Tj\n", hy, clean(&subject)));

        if !attachments.is_empty() {
            hy -= 18.0;
            let att_names = attachments
                .iter()
                .map(|a| a.filename.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            stream_p1.push_str(&format!("/F2 9 Tf\n1 0 0 1 50 {:.2} Tm\n(PJ ({}): ) Tj\n", hy, attachments.len()));
            stream_p1.push_str(&format!("/F1 9 Tf\n1 0 0 1 100 {:.2} Tm\n({}) Tj\n", hy, clean(&att_names)));
        }

        // Corps du message sur la page 1 (débute à y = 670)
        let mut by = 670.0;
        stream_p1.push_str("/F1 10 Tf\n");

        let mut line_idx = 0;
        while line_idx < lines.len() && by > 60.0 {
            let l = lines[line_idx];
            let safe_l = clean(crate::security::safe_truncate_str(l, 90));
            stream_p1.push_str(&format!("1 0 0 1 50 {:.2} Tm\n({}) Tj\n", by, safe_l));
            by -= 16.0;
            line_idx += 1;
        }
        stream_p1.push_str("ET\n");

        let content_p1 = doc.add_object(Stream::new(
            lopdf::Dictionary::new(),
            stream_p1.as_bytes().to_vec(),
        ));

        let p1_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => resources_id,
            "Contents" => content_p1,
        });
        page_objects.push(p1_id.into());

        // Pages suivantes si le corps du texte se prolonge
        while line_idx < lines.len() {
            let mut stream_pn = String::new();
            stream_pn.push_str("BT\n/F1 10 Tf\n");
            let mut y = 800.0;

            while line_idx < lines.len() && y > 60.0 {
                let l = lines[line_idx];
                let safe_l = clean(crate::security::safe_truncate_str(l, 90));
                stream_pn.push_str(&format!("1 0 0 1 50 {:.2} Tm\n({}) Tj\n", y, safe_l));
                y -= 16.0;
                line_idx += 1;
            }
            stream_pn.push_str("ET\n");

            let content_pn = doc.add_object(Stream::new(
                lopdf::Dictionary::new(),
                stream_pn.as_bytes().to_vec(),
            ));

            let pn_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Resources" => resources_id,
                "Contents" => content_pn,
            });
            page_objects.push(pn_id.into());
        }

        doc.set_object(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_objects.clone(),
                "Count" => page_objects.len() as i64,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        doc.trailer.set("Root", catalog_id);
        doc.save(output_path)?;

        Ok(EmailConversionResult { attachments })
    }
}
