use crate::models::{DocumentBuildOrder, PageAction, WatermarkOptions};
use anyhow::{bail, Context, Result};
use lopdf::content::Content;
use lopdf::{Object, Stream};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

pub struct DocumentBuilderEngine;

impl DocumentBuilderEngine {
    pub fn execute_build(
        source_paths: &[PathBuf],
        build_order: &DocumentBuildOrder,
        output_path: &Path,
    ) -> Result<()> {
        if source_paths.is_empty() {
            bail!("No source documents provided");
        }

        // 1. If single document or multiple, start with base
        let temp_dir = tempfile::tempdir()?;
        let working_pdf = temp_dir.path().join("working.pdf");

        if source_paths.len() == 1 {
            std::fs::copy(&source_paths[0], &working_pdf)?;
        } else {
            // Merge sources using pdfunite
            let mut cmd = Command::new("pdfunite");
            for p in source_paths {
                cmd.arg(p);
            }
            cmd.arg(&working_pdf);

            let status = cmd.status().context("Failed to execute pdfunite")?;
            if !status.success() {
                bail!("pdfunite failed during document merge");
            }
        }

        // 2. Open document with lopdf for page actions and watermarks
        let mut doc = lopdf::Document::load(&working_pdf)?;

        // Execute PageActions: Deletes and Rotates
        let mut pages_to_delete: Vec<u32> = Vec::new();

        for action in &build_order.page_actions {
            match action {
                PageAction::Delete { page_number } => {
                    pages_to_delete.push(*page_number as u32);
                }
                PageAction::Rotate { page_number, degrees } => {
                    Self::apply_rotation(&mut doc, *page_number as u32, *degrees)?;
                }
                PageAction::Keep { page_number, rotation } => {
                    if let Some(r) = rotation {
                        Self::apply_rotation(&mut doc, *page_number as u32, *r)?;
                    }
                }
                PageAction::InsertBlank { .. } => {
                    // Future blank page insertion
                }
            }
        }

        if !pages_to_delete.is_empty() {
            doc.delete_pages(&pages_to_delete);
        }

        // 3. Apply Watermark if requested
        if let Some(ref wm) = build_order.watermark {
            Self::apply_watermark(&mut doc, wm)?;
        }

        // 4. Save to final output
        doc.save(output_path)?;
        info!("Built document saved to {}", output_path.display());

        Ok(())
    }

    fn apply_rotation(doc: &mut lopdf::Document, page_num: u32, degrees: i32) -> Result<()> {
        let pages = doc.get_pages();
        if let Some(&page_id) = pages.get(&page_num) {
            if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
                let current_rot = page_dict
                    .get(b"Rotate")
                    .and_then(|o| o.as_i64())
                    .unwrap_or(0) as i32;
                let new_rot = (current_rot + degrees).rem_euclid(360);
                page_dict.set("Rotate", Object::Integer(new_rot as i64));
            }
        }
        Ok(())
    }

    pub fn apply_watermark(doc: &mut lopdf::Document, wm: &WatermarkOptions) -> Result<()> {
        let pages = doc.get_pages();
        let watermark_text = &wm.text;
        if watermark_text.trim().is_empty() {
            return Ok(());
        }

        let escaped_text = crate::security::escape_pdf_str(watermark_text);

        // 1. Register Helvetica-Bold Type1 Font in Document
        let mut font_dict = lopdf::Dictionary::new();
        font_dict.set("Type", "Font");
        font_dict.set("Subtype", "Type1");
        font_dict.set("BaseFont", "Helvetica-Bold");
        let font_id = doc.add_object(Object::Dictionary(font_dict));

        // 2. Register ExtGState for transparency
        let opacity = if wm.opacity.is_finite() { wm.opacity.clamp(0.05, 1.0) } else { 0.2 };
        let mut gs_dict = lopdf::Dictionary::new();
        gs_dict.set("Type", "ExtGState");
        gs_dict.set("ca", Object::Real(opacity as f32));
        gs_dict.set("CA", Object::Real(opacity as f32));
        let gs_id = doc.add_object(Object::Dictionary(gs_dict));

        let (cr, cg, cb) = parse_hex_color(&wm.color);
        let safe_rot = if wm.rotation.is_finite() { wm.rotation } else { 45.0 };
        let angle_rad = (if safe_rot.abs() > 0.1 { safe_rot } else { 45.0 }).to_radians();
        let cos = angle_rad.cos();
        let sin = angle_rad.sin();

        for (_p_num, &page_id) in pages.iter() {
            // Register font and extgstate in page resources
            if let Ok(res_dict) = get_or_create_resources(doc, page_id) {
                if !res_dict.has(b"Font") {
                    res_dict.set("Font", lopdf::Dictionary::new());
                }
                if let Ok(font_dict) = res_dict.get_mut(b"Font").and_then(|o| o.as_dict_mut()) {
                    font_dict.set("WmFont", Object::Reference(font_id));
                }
                if !res_dict.has(b"ExtGState") {
                    res_dict.set("ExtGState", lopdf::Dictionary::new());
                }
                if let Ok(gs_dict) = res_dict.get_mut(b"ExtGState").and_then(|o| o.as_dict_mut()) {
                    gs_dict.set("WmGS", Object::Reference(gs_id));
                }
            }

            // Get page dimensions
            let mut page_w = 595.0;
            let mut page_h = 842.0;
            if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
                let box_obj = page_dict.get(b"CropBox").or_else(|_| page_dict.get(b"MediaBox"));
                if let Ok(box_arr) = box_obj.and_then(|o| o.as_array()) {
                    if box_arr.len() == 4 {
                        let to_f64 = |o: &lopdf::Object| match o {
                            lopdf::Object::Real(f) => Some(*f as f64),
                            lopdf::Object::Integer(i) => Some(*i as f64),
                            _ => None,
                        };
                        let x1 = to_f64(&box_arr[0]).unwrap_or(0.0);
                        let y1 = to_f64(&box_arr[1]).unwrap_or(0.0);
                        let x2 = to_f64(&box_arr[2]).unwrap_or(595.0);
                        let y2 = to_f64(&box_arr[3]).unwrap_or(842.0);
                        page_w = (x2 - x1).abs();
                        page_h = (y2 - y1).abs();
                    }
                }
            }

            let cx = page_w / 2.0;
            let cy = page_h / 2.0;

            let font_size = if wm.font_size >= 10.0 {
                wm.font_size
            } else {
                (page_w.min(page_h) * 0.08).max(28.0)
            };

            let half_w = escaped_text.len() as f64 * font_size * 0.28;
            let half_h = font_size * 0.35;

            let stream_str = format!(
                "\nq\n/WmGS gs\n{:.3} {:.3} {:.3} rg\nBT\n/WmFont {:.1} Tf\n{:.4} {:.4} {:.4} {:.4} {:.2} {:.2} cm\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
                cr, cg, cb, font_size, cos, sin, -sin, cos, cx, cy, -half_w, -half_h, escaped_text
            );

            if let Ok(content) = Content::decode(stream_str.as_bytes()) {
                let _ = doc.add_to_page_content(page_id, content);
            }
        }

        Ok(())
    }
}

fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
    let clean = hex.trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        }
    }
    // Default semi-transparent bold red/gray
    (0.85, 0.15, 0.15)
}

fn get_or_create_resources(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<&mut lopdf::Dictionary> {
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
            page_dict.set("Resources", lopdf::Dictionary::new());
        }
        Ok(page_dict.get_mut(b"Resources")?.as_dict_mut()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_watermark_in_memory() {
        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.new_object_id();
        let content_id = doc.new_object_id();
        let page_id = doc.new_object_id();

        let mut page = lopdf::Dictionary::new();
        page.set("Type", "Page");
        page.set("Parent", pages_id);
        page.set("Contents", content_id);
        page.set("MediaBox", vec![0.into(), 0.into(), 595.into(), 842.into()]);
        doc.objects.insert(page_id, Object::Dictionary(page));

        let mut pages = lopdf::Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Kids", vec![page_id.into()]);
        pages.set("Count", 1);
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let mut catalog = lopdf::Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = doc.new_object_id();
        doc.objects.insert(catalog_id, Object::Dictionary(catalog));
        doc.trailer.set("Root", catalog_id);

        let content = lopdf::content::Content { operations: vec![] };
        doc.objects.insert(content_id, Object::Stream(Stream::new(lopdf::Dictionary::new(), content.encode().unwrap())));

        let wm = WatermarkOptions {
            text: "CONFIDENTIEL - Jean DUPONT".to_string(),
            opacity: 0.25,
            font_size: 36.0,
            rotation: 45.0,
            color: "#CC0000".to_string(),
        };

        let res = DocumentBuilderEngine::apply_watermark(&mut doc, &wm);
        assert!(res.is_ok(), "Watermark injection should succeed");

        let mut out_bytes = Vec::new();
        let save_res = doc.save_to(&mut out_bytes);
        assert!(save_res.is_ok(), "Document save_to should succeed");
        assert!(!out_bytes.is_empty(), "Output bytes should not be empty");
        let content_str = String::from_utf8_lossy(&out_bytes);
        assert!(content_str.contains("CONFIDENTIEL - Jean DUPONT"), "Output PDF should contain watermark text");
    }
}

