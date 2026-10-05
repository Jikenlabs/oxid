use crate::models::{RedactionItem, RedactionOrder};
use anyhow::Result;
use std::path::Path;
use tracing::info;

pub struct RedactionEngine;

impl RedactionEngine {
    pub fn apply_redaction(
        input_path: &Path,
        order: &RedactionOrder,
        output_path: &Path,
    ) -> Result<()> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(input_path)?;
        let mut doc = lopdf::Document::load(&rendition.effective_path)?;
        let pages = doc.get_pages();


        // Regroupe les caviardages par numéro de page
        let mut by_page: std::collections::HashMap<usize, Vec<&RedactionItem>> =
            std::collections::HashMap::new();
        for item in &order.items {
            by_page.entry(item.page_number).or_default().push(item);
        }

        for (page_num, items) in by_page {
            if let Some(&page_id) = pages.get(&(page_num as u32)) {
                // Récupère la hauteur de page pour l'inversion des coordonnées (l'origine PDF étant en bas à gauche)
                let mut page_height = 842.0;
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

                // Construit le flux graphique avec rectangles noirs opaques et étiquettes textuelles
                let mut stream_content = String::new();
                for item in items {
                    let pdf_x = item.x;
                    let pdf_y = (page_height - (item.y + item.height)).max(0.0);
                    let pdf_w = item.width;
                    let pdf_h = item.height;

                    let label = item
                        .overlay_text
                        .as_deref()
                        .or(item.reason.as_deref())
                        .unwrap_or("CONFIDENTIEL");

                    let escaped_label = crate::security::escape_pdf_str(label);

                    // Dessine le rectangle noir plein et l'étiquette de texte blanche
                    stream_content.push_str(&format!(
                        "\nq\n0 0 0 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n",
                        pdf_x, pdf_y, pdf_w, pdf_h
                    ));

                    // Si la hauteur est suffisante, dessine l'étiquette blanche centrée
                    if pdf_h >= 10.0 {
                        let text_y = pdf_y + (pdf_h / 2.0) - 3.0;
                        let text_x = pdf_x + 4.0;
                        stream_content.push_str(&format!(
                            "1 1 1 rg\nBT\n/Helvetica 8 Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                            text_x, text_y, escaped_label
                        ));
                    }

                    stream_content.push_str("Q\n");
                }

                if let Ok(content) = lopdf::content::Content::decode(stream_content.as_bytes()) {
                    let _ = doc.add_to_page_content(page_id, content);
                }
            }
        }

        doc.save(output_path)?;
        info!("Redacted PDF successfully written to {}", output_path.display());
        Ok(())
    }
}
