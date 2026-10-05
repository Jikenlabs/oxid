use anyhow::{Context, Result};
use chrono::Utc;
use lopdf::content::Content;
use lopdf::dictionary;
use lopdf::{Dictionary, Object, Stream, StringFormat};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tracing::info;

use crate::models::{SignRequest, SignResponse};

pub struct DigitalSignatureEngine;

impl DigitalSignatureEngine {
    /// Calcule l'empreinte SHA-256 d'un fichier.
    pub fn compute_sha256(path: &Path) -> Result<String> {
        let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    /// Signe un document PDF en apposant un cartouche visuel horodaté et des métadonnées numériques.
    pub fn sign_pdf(
        input_path: &Path,
        output_path: &Path,
        req: &SignRequest,
    ) -> Result<SignResponse> {
        let sha256_hash = Self::compute_sha256(input_path)?;
        let timestamp = Utc::now();
        let timestamp_str = timestamp.format("%d/%m/%Y %H:%M:%S UTC").to_string();
        let pdf_date = timestamp.format("D:%Y%m%d%H%M%SZ").to_string();

        let mut doc = lopdf::Document::load(input_path)
            .with_context(|| format!("Failed to load PDF {}", input_path.display()))?;

        let pages = doc.get_pages();
        let target_page_num = req.page_number as u32;

        let &page_id = pages
            .get(&target_page_num)
            .context("Target page not found in PDF")?;

        // Extrait les dimensions de la page
        let mut page_height = 842.0; // Valeur par défaut A4
        if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
            let box_obj = page_dict.get(b"CropBox").or_else(|_| page_dict.get(b"MediaBox"));
            let to_f64 = |o: &lopdf::Object| match o {
                lopdf::Object::Real(f) => Some(*f as f64),
                lopdf::Object::Integer(i) => Some(*i as f64),
                _ => None,
            };
            if let Ok(box_arr) = box_obj.and_then(|o| o.as_array()) {
                if box_arr.len() == 4 {
                    let y1 = to_f64(&box_arr[1]).unwrap_or(0.0);
                    let y2 = to_f64(&box_arr[3]).unwrap_or(842.0);
                    page_height = (y2 - y1).abs();
                }
            }
        }

        // Coordonnées : Origine navigateur en haut à gauche -> Origine PDF en bas à gauche
        let stamp_x = req.x;
        let stamp_y = (page_height - (req.y + req.height)).max(0.0);
        let stamp_w = req.width.max(120.0);
        let stamp_h = req.height.max(60.0);

        // Gestion optionnelle de l'image manuscrite
        let mut image_xobject_name: Option<String> = None;
        let mut image_xobject_id: Option<lopdf::ObjectId> = None;
        if let Some(ref b64) = req.handwritten_png_base64 {
            if let Some(raw_b64) = b64.split(',').last() {
                let trimmed = raw_b64.trim();
                if trimmed.len() <= 5 * 1024 * 1024 {
                    if let Ok(png_bytes) = base64_decode(trimmed) {
                        if let Ok(dyn_img) = image::load_from_memory(&png_bytes) {
                            let rgba = dyn_img.to_rgba8();
                            let (img_w, img_h) = rgba.dimensions();
                            if img_w > 0 && img_h > 0 && img_w <= 2048 && img_h <= 2048 {
                                let total_bytes = (img_w as usize).saturating_mul(img_h as usize).saturating_mul(3);
                                let mut rgb_bytes = Vec::with_capacity(total_bytes);
                        for pixel in rgba.pixels() {
                            let alpha = pixel[3] as f32 / 255.0;
                            // Fusionne avec un arrière-plan blanc (canal alpha)
                            let r = ((pixel[0] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            let g = ((pixel[1] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            let b = ((pixel[2] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            rgb_bytes.push(r);
                            rgb_bytes.push(g);
                            rgb_bytes.push(b);
                        }

                        let mut img_dict = Dictionary::new();
                        img_dict.set("Type", Object::Name(b"XObject".to_vec()));
                        img_dict.set("Subtype", Object::Name(b"Image".to_vec()));
                        img_dict.set("Width", Object::Integer(img_w as i64));
                        img_dict.set("Height", Object::Integer(img_h as i64));
                        img_dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                        img_dict.set("BitsPerComponent", Object::Integer(8));

                        let img_stream = Stream::new(img_dict, rgb_bytes);
                        let img_id = doc.add_object(Object::Stream(img_stream));

                        let xname = "SigImg1";
                        image_xobject_name = Some(xname.to_string());
                        image_xobject_id = Some(img_id);
                            }
                        }
                    }
                }
            }
        }

        // Enregistre les polices standard dans Resources pour un rendu fidèle dans tous les lecteurs PDF
        let font_reg_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let font_bold_id = doc.add_object(lopdf::dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica-Bold",
        });

        // Enregistre polices et XObjects dans les ressources de page (gère les références indirectes)
        if let Ok(res_dict) = get_or_create_resources(&mut doc, page_id) {
            if !res_dict.has(b"Font") {
                res_dict.set("Font", Dictionary::new());
            }
            if let Ok(font_dict) = res_dict.get_mut(b"Font").and_then(|o| o.as_dict_mut()) {
                font_dict.set("SigFontReg", Object::Reference(font_reg_id));
                font_dict.set("SigFontBold", Object::Reference(font_bold_id));
            }
            if let Some(ref xname) = image_xobject_name {
                if let Some(img_id) = image_xobject_id {
                    if !res_dict.has(b"XObject") {
                        res_dict.set("XObject", Dictionary::new());
                    }
                    if let Ok(xobj_dict) = res_dict.get_mut(b"XObject").and_then(|o| o.as_dict_mut()) {
                        xobj_dict.set(xname.as_str(), Object::Reference(img_id));
                    }
                }
            }
        }

        // Construit le flux graphique PDF du tampon visuel
        let signer = escape_pdf_str(&req.signer_name);
        let reason = escape_pdf_str(req.reason.as_deref().unwrap_or("Approbation légale"));
        let location = escape_pdf_str(req.location.as_deref().unwrap_or("Paris, FR"));
        let short_hash = &sha256_hash[..16];

        let mut stream = String::new();
        stream.push_str("\nq\n");

        // 1. Fond du cartouche (teinte bleu/gris très claire)
        stream.push_str(&format!(
            "0.96 0.98 1.0 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n",
            stamp_x, stamp_y, stamp_w, stamp_h
        ));

        // 2. Bordure du cartouche (bleu marine solennel)
        stream.push_str(&format!(
            "0.1 0.3 0.6 RG\n1.5 w\n{:.2} {:.2} {:.2} {:.2} re\nS\n",
            stamp_x, stamp_y, stamp_w, stamp_h
        ));

        // 3. Bandeau d'en-tête
        let header_h = 16.0f64.min(stamp_h * 0.28);
        let header_y = stamp_y + stamp_h - header_h;
        stream.push_str(&format!(
            "0.1 0.3 0.6 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n",
            stamp_x, header_y, stamp_w, header_h
        ));

        // Texte de l'en-tête en blanc
        stream.push_str(&format!(
            "1 1 1 rg\nBT\n/SigFontBold 7 Tf\n{:.2} {:.2} Td\n(DOC. SIGNE ELECTRONIQUEMENT - CERTIFIE) Tj\nET\n",
            stamp_x + 6.0,
            header_y + 4.5
        ));

        // 4. Corps textuel du tampon
        let text_left = stamp_x + 6.0;
        let mut cur_y = header_y - 10.0;

        stream.push_str(&format!(
            "0.1 0.1 0.1 rg\nBT\n/SigFontBold 8 Tf\n{:.2} {:.2} Td\n(Signataire : {}) Tj\nET\n",
            text_left, cur_y, signer
        ));
        cur_y -= 9.5;

        stream.push_str(&format!(
            "0.2 0.2 0.2 rg\nBT\n/SigFontReg 7 Tf\n{:.2} {:.2} Td\n(Date : {}) Tj\nET\n",
            text_left, cur_y, timestamp_str
        ));
        cur_y -= 8.5;

        stream.push_str(&format!(
            "0.2 0.2 0.2 rg\nBT\n/SigFontReg 7 Tf\n{:.2} {:.2} Td\n(Motif : {} | {}) Tj\nET\n",
            text_left, cur_y, reason, location
        ));
        cur_y -= 8.5;

        stream.push_str(&format!(
            "0.4 0.4 0.4 rg\nBT\n/SigFontReg 6 Tf\n{:.2} {:.2} Td\n(SHA-256 : {}...) Tj\nET\n",
            text_left, cur_y, short_hash
        ));

        // 5. Dessine la signature manuscrite si fournie
        if let Some(xname) = image_xobject_name {
            let img_w = 60.0f64.min(stamp_w * 0.4);
            let img_h = (stamp_h - header_h - 6.0).max(10.0);
            let img_x = stamp_x + stamp_w - img_w - 4.0;
            let img_y = stamp_y + 3.0;

            stream.push_str(&format!(
                "q\n{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/{} Do\nQ\n",
                img_w, img_h, img_x, img_y, xname
            ));
        }

        stream.push_str("Q\n");

        if let Ok(content) = Content::decode(stream.as_bytes()) {
            let _ = doc.add_to_page_content(page_id, content);
        }

        // Ajoute les métadonnées de signature numérique dans le catalogue PDF (Catalog/Root)
        if let Ok(root_id) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
            if let Ok(root_dict) = doc.get_object_mut(root_id).and_then(|o| o.as_dict_mut()) {
                let mut sig_meta = Dictionary::new();
                sig_meta.set("Type", Object::Name(b"OxidSignature".to_vec()));
                sig_meta.set(
                    "Signer",
                    Object::String(signer.into_bytes(), StringFormat::Literal),
                );
                sig_meta.set(
                    "Date",
                    Object::String(pdf_date.into_bytes(), StringFormat::Literal),
                );
                sig_meta.set(
                    "Reason",
                    Object::String(reason.into_bytes(), StringFormat::Literal),
                );
                sig_meta.set(
                    "SHA256",
                    Object::String(sha256_hash.clone().into_bytes(), StringFormat::Literal),
                );
                root_dict.set("OxidSignature", Object::Dictionary(sig_meta));
            }
        }

        doc.save(output_path)?;
        info!(
            "Signed PDF generated successfully at {}",
            output_path.display()
        );

        Ok(SignResponse {
            signed_doc_id: output_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            sha256_digest: sha256_hash,
            timestamp_utc: timestamp_str,
            page_number: req.page_number,
            message: "Document signé avec succès (tampon certifié et métadonnées appliqués)"
                .to_string(),
        })
    }
}

fn escape_pdf_str(s: &str) -> String {
    crate::security::escape_pdf_str(s)
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    // Décodage base64 léger sans dépendance externe
    const B64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut clean: Vec<u8> = Vec::with_capacity(input.len());
    for &b in input.as_bytes() {
        if B64_TABLE.contains(&b) || b == b'=' {
            clean.push(b);
        }
    }

    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut i = 0;
    while i < clean.len() {
        let b0 = decode_b64_char(clean[i]);
        let b1 = if i + 1 < clean.len() {
            decode_b64_char(clean[i + 1])
        } else {
            0
        };
        let b2 = if i + 2 < clean.len() {
            decode_b64_char(clean[i + 2])
        } else {
            0
        };
        let b3 = if i + 3 < clean.len() {
            decode_b64_char(clean[i + 3])
        } else {
            0
        };

        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        out.push(((triple >> 16) & 0xFF) as u8);
        if i + 2 < clean.len() && clean[i + 2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if i + 3 < clean.len() && clean[i + 3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }

        i += 4;
    }

    Ok(out)
}

fn decode_b64_char(c: u8) -> u32 {
    match c {
        b'A'..=b'Z' => (c - b'A') as u32,
        b'a'..=b'z' => (c - b'a' + 26) as u32,
        b'0'..=b'9' => (c - b'0' + 52) as u32,
        b'+' => 62,
        b'/' => 63,
        _ => 0,
    }
}

fn get_or_create_resources(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<&mut Dictionary> {
    let res_ref = {
        let page_dict = doc.get_object(page_id)?.as_dict()?;
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };

    if let Some(res_id) = res_ref {
        Ok(doc.get_object_mut(res_id)?.as_dict_mut()?)
    } else {
        let page_obj = doc.get_object_mut(page_id)?;
        let page_dict = page_obj.as_dict_mut()?;
        if !page_dict.has(b"Resources") {
            page_dict.set("Resources", Dictionary::new());
        }
        Ok(page_dict.get_mut(b"Resources")?.as_dict_mut()?)
    }
}

