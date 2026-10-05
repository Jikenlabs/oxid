use crate::models::{Bookmark, DocumentMetadata, PageMetadata, PageText, TextSpan};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ImageBox {
    pub x: f64,
    pub top_y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct PdfEngine;

impl PdfEngine {
    pub fn get_metadata(doc_id: &str, filename: &str, path: &Path) -> Result<DocumentMetadata> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(path)?;
        let effective_path = &rendition.effective_path;

        let extension = effective_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Prise en charge des formats vidéo (extraction de poster miniature et métadonnées)
        if crate::engine::converter::FormatConverter::is_video(&rendition.original_format) {
            let (w, h) = if effective_path.exists() && (extension == "jpg" || extension == "jpeg" || extension == "png") {
                image::image_dimensions(effective_path).unwrap_or((1280, 720))
            } else {
                (1280, 720)
            };

            let mime_type = match rendition.original_format.as_str() {
                "mp4" | "m4v" => "video/mp4",
                "webm" => "video/webm",
                "ogv" | "ogg" => "video/ogg",
                "mov" => "video/quicktime",
                "avi" => "video/x-msvideo",
                "mkv" => "video/x-matroska",
                _ => "video/mp4",
            };

            return Ok(DocumentMetadata {
                id: doc_id.to_string(),
                filename: filename.to_string(),
                mime_type: mime_type.to_string(),
                file_size,
                page_count: 1,
                pages: vec![PageMetadata {
                    page_number: 1,
                    width: w as f64,
                    height: h as f64,
                    rotation: 0,
                }],
                bookmarks: vec![],
                attachments: vec![],
            });
        }

        // Prise en charge des flux audio
        if crate::engine::converter::FormatConverter::is_audio(&rendition.original_format) {
            let mime_type = match rendition.original_format.as_str() {
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                "flac" => "audio/flac",
                "aac" => "audio/aac",
                "m4a" => "audio/mp4",
                _ => "audio/mpeg",
            };

            return Ok(DocumentMetadata {
                id: doc_id.to_string(),
                filename: filename.to_string(),
                mime_type: mime_type.to_string(),
                file_size,
                page_count: 1,
                pages: vec![PageMetadata {
                    page_number: 1,
                    width: 800.0,
                    height: 200.0,
                    rotation: 0,
                }],
                bookmarks: vec![],
                attachments: vec![],
            });
        }

        if extension == "png"
            || extension == "jpg"
            || extension == "jpeg"
            || extension == "webp"
            || extension == "tiff"
            || extension == "tif"
            || extension == "bmp"
            || extension == "gif"
        {
            let (w, h) = image::image_dimensions(effective_path).unwrap_or((800, 600));
            return Ok(DocumentMetadata {
                id: doc_id.to_string(),
                filename: filename.to_string(),
                mime_type: format!("image/{}", extension),
                file_size,
                page_count: 1,
                pages: vec![PageMetadata {
                    page_number: 1,
                    width: w as f64,
                    height: h as f64,
                    rotation: 0,
                }],
                bookmarks: vec![],
                attachments: rendition.attachments,
            });
        }

        // Handle Audio formats
        if crate::engine::converter::FormatConverter::is_audio(&rendition.original_format) {
            let mime_type = match rendition.original_format.as_str() {
                "mp3" => "audio/mpeg",
                "wav" => "audio/wav",
                "flac" => "audio/flac",
                "aac" => "audio/aac",
                "m4a" => "audio/mp4",
                _ => "audio/mpeg",
            };

            return Ok(DocumentMetadata {
                id: doc_id.to_string(),
                filename: filename.to_string(),
                mime_type: mime_type.to_string(),
                file_size,
                page_count: 1,
                pages: vec![PageMetadata {
                    page_number: 1,
                    width: 800.0,
                    height: 200.0,
                    rotation: 0,
                }],
                bookmarks: vec![],
                attachments: vec![],
            });
        }

        // Handle PDF via lopdf
        match lopdf::Document::load(effective_path) {
            Ok(doc) => {
                let pages = doc.get_pages();
                let mut page_metas = Vec::with_capacity(pages.len());

                for (page_num, (&_p_num, &page_id)) in pages.iter().enumerate() {
                    let mut width = 595.0; // Format A4 par défaut
                    let mut height = 842.0;
                    let mut rotation = 0;

                    if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
                        // Angle de rotation
                        if let Ok(r) = page_dict.get(b"Rotate").and_then(|o| o.as_i64()) {
                            rotation = r as i32;
                        }

                        // Boîte de découpe CropBox ou boîte physique MediaBox
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
                                width = (x2 - x1).abs();
                                height = (y2 - y1).abs();
                            }
                        }
                    }

                    if rotation == 90 || rotation == 270 {
                        std::mem::swap(&mut width, &mut height);
                    }

                    page_metas.push(PageMetadata {
                        page_number: page_num + 1,
                        width,
                        height,
                        rotation,
                    });
                }

                let page_count = page_metas.len();

                // Extraction du plan du document (signets / outlines)
                let bookmarks = Self::extract_outlines(&doc);

                Ok(DocumentMetadata {
                    id: doc_id.to_string(),
                    filename: filename.to_string(),
                    mime_type: "application/pdf".to_string(),
                    file_size,
                    page_count,
                    pages: page_metas,
                    bookmarks,
                    attachments: rendition.attachments,
                })
            }
            Err(e) => {
                bail!("Échec d'analyse du document PDF {}: {}", path.display(), e);
            }
        }
    }

    fn extract_outlines(doc: &lopdf::Document) -> Vec<Bookmark> {
        let mut bookmarks = Vec::new();
        if let Ok(catalog) = doc.catalog() {
            if let Ok(outlines_ref) = catalog.get(b"Outlines").and_then(|o| o.as_reference()) {
                if let Ok(outlines) = doc.get_object(outlines_ref).and_then(|o| o.as_dict()) {
                    let mut current = outlines.get(b"First").and_then(|o| o.as_reference()).ok();
                    while let Some(item_ref) = current {
                        if let Ok(item) = doc.get_object(item_ref).and_then(|o| o.as_dict()) {
                            let title = item
                                .get(b"Title")
                                .and_then(|o| o.as_str())
                                .map(|s| String::from_utf8_lossy(s).to_string())
                                .unwrap_or_else(|_| "Section".to_string());

                            bookmarks.push(Bookmark {
                                title,
                                page_number: 1, // Page par défaut
                                children: vec![],
                            });

                            current = item.get(b"Next").and_then(|o| o.as_reference()).ok();
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        bookmarks
    }

    pub fn render_page(path: &Path, page: usize, dpi: u32) -> Result<Vec<u8>> {
        Self::render_page_with_format(path, page, dpi, Some("png"))
    }

    pub fn render_page_with_format(path: &Path, page: usize, dpi: u32, format: Option<&str>) -> Result<Vec<u8>> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(path)?;
        let effective_path = &rendition.effective_path;

        let extension = effective_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Rendu direct des fichiers image matriciels
        if extension == "png"
            || extension == "jpg"
            || extension == "jpeg"
            || extension == "webp"
            || extension == "tiff"
            || extension == "tif"
            || extension == "bmp"
            || extension == "gif"
        {
            if let Ok(img) = image::open(effective_path) {
                let mut buf = std::io::Cursor::new(Vec::new());
                img.write_to(&mut buf, image::ImageFormat::Png)?;
                return Ok(buf.into_inner());
            }

            // Repli via ImageMagick / convert si disponible
            let temp_dir = tempfile::tempdir().context("Échec de création du répertoire temporaire")?;
            let out_png = temp_dir.path().join("converted.png");
            if let Ok(status) = Command::new("convert")
                .arg(effective_path)
                .arg(&out_png)
                .status()
            {
                if status.success() && out_png.exists() {
                    return Ok(std::fs::read(&out_png)?);
                }
            }

            // Repli : lecture brute si déjà PNG
            return Ok(std::fs::read(effective_path)?);
        }

        // Rendu vectoriel de page PDF via pdftoppm
        let temp_dir = tempfile::tempdir().context("Échec de création du répertoire temporaire")?;
        let output_prefix = temp_dir.path().join("page");

        // Utilisation du JPEG haute performance par défaut (70x plus rapide et 6x plus léger que PNG)
        let requested_format = format.unwrap_or("jpeg").to_lowercase();
        let use_png = requested_format == "png";

        let mut cmd = Command::new("pdftoppm");
        if use_png {
            cmd.arg("-png");
        } else {
            let quality = if dpi <= 72 { "80" } else { "85" };
            cmd.arg("-jpeg").arg("-jpegopt").arg(format!("quality={}", quality));
        }

        let status = cmd
            .arg("-r")
            .arg(dpi.to_string())
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg(effective_path)
            .arg(&output_prefix)
            .status()
            .context("Échec d'exécution de pdftoppm")?;

        if !status.success() {
            bail!("pdftoppm a échoué avec le code de sortie : {}", status);
        }

        // Lecture de l'image matricielle générée
        let mut rendered_bytes = None;
        if let Ok(entries) = std::fs::read_dir(temp_dir.path()) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg") {
                    rendered_bytes = Some(std::fs::read(entry.path())?);
                    break;
                }
            }
        }

        rendered_bytes.context("L'image rendue est introuvable dans le répertoire temporaire")
    }

    pub fn get_page_text(path: &Path, page: usize) -> Result<PageText> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(path)?;
        let effective_path = &rendition.effective_path;

        let extension = effective_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if extension != "pdf" {
            return Ok(PageText {
                page_number: page,
                spans: vec![],
            });
        }

        let image_boxes = Self::extract_image_boxes(effective_path, page);

        // Appel à pdftotext avec l'option de positionnement -bbox-layout
        let output = Command::new("pdftotext")
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg("-bbox-layout")
            .arg(effective_path)
            .arg("-")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let text_xml = String::from_utf8_lossy(&out.stdout);
                let spans = Self::parse_bbox_xml(&text_xml, &image_boxes);
                return Ok(PageText {
                    page_number: page,
                    spans,
                });
            }
        }

        // Repli texte vide
        Ok(PageText {
            page_number: page,
            spans: vec![],
        })
    }

    pub fn get_page_plain_text(path: &Path, page: usize) -> Result<String> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(path)?;
        let effective_path = &rendition.effective_path;

        let extension = effective_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if extension != "pdf" {
            return Ok(String::new());
        }

        let output = Command::new("pdftotext")
            .arg("-f")
            .arg(page.to_string())
            .arg("-l")
            .arg(page.to_string())
            .arg(effective_path)
            .arg("-")
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                return Ok(String::from_utf8_lossy(&out.stdout).to_string());
            }
        }

        Ok(String::new())
    }

    pub fn extract_image_boxes(effective_path: &Path, page: usize) -> Vec<ImageBox> {
        let mut boxes = Vec::new();
        let doc = match lopdf::Document::load(effective_path) {
            Ok(d) => d,
            Err(_) => return boxes,
        };

        let pages = doc.get_pages();
        let page_id = match pages.get(&(page as u32)) {
            Some(&id) => id,
            None => return boxes,
        };

        let mut page_height = 842.0;
        if let Ok(page_dict) = doc.get_object(page_id).and_then(|o| o.as_dict()) {
            let box_obj = page_dict.get(b"CropBox").or_else(|_| page_dict.get(b"MediaBox"));
            if let Ok(box_arr) = box_obj.and_then(|o| o.as_array()) {
                if box_arr.len() == 4 {
                    let to_f64 = |o: &lopdf::Object| match o {
                        lopdf::Object::Real(f) => Some(*f as f64),
                        lopdf::Object::Integer(i) => Some(*i as f64),
                        _ => None,
                    };
                    let y1 = to_f64(&box_arr[1]).unwrap_or(0.0);
                    let y2 = to_f64(&box_arr[3]).unwrap_or(842.0);
                    page_height = (y2 - y1).abs();
                }
            }
        }

        if let Ok(content) = doc.get_page_content(page_id) {
            let content_str = String::from_utf8_lossy(&content);
            let tokens: Vec<&str> = content_str.split_whitespace().collect();
            for i in 6..tokens.len() {
                if tokens[i] == "cm" {
                    let end_idx = (i + 10).min(tokens.len());
                    let has_image = tokens[i + 1..end_idx]
                        .windows(2)
                        .any(|w| w[0].starts_with("/Image") && w[1] == "Do");
                    if has_image {
                        if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                            tokens[i - 6].parse::<f64>(),
                            tokens[i - 3].parse::<f64>(),
                            tokens[i - 2].parse::<f64>(),
                            tokens[i - 1].parse::<f64>(),
                        ) {
                            if w > 1.0 && h > 1.0 {
                                let top_y = page_height - y - h;
                                boxes.push(ImageBox {
                                    x,
                                    top_y,
                                    width: w,
                                    height: h,
                                });
                            }
                        }
                    }
                }
            }
        }
        boxes
    }

    fn parse_bbox_xml(xml_content: &str, image_boxes: &[ImageBox]) -> Vec<TextSpan> {
        let mut spans = Vec::new();

        let extract_attr = |line: &str, attr: &str| -> Option<f64> {
            let pattern = format!("{}=\"", attr);
            if let Some(start) = line.find(&pattern) {
                let sub = &line[start + pattern.len()..];
                if let Some(end) = sub.find('"') {
                    return sub[..end].parse::<f64>().ok();
                }
            }
            None
        };

        struct RawWord {
            text: String,
            x_min: f64,
            y_min: f64,
            x_max: f64,
            y_max: f64,
        }

        // Découpage par balises <line pour regrouper les mots par ligne
        for line_chunk in xml_content.split("<line ") {
            if !line_chunk.contains("</line>") {
                continue;
            }

            let mut words = Vec::new();
            for word_line in line_chunk.lines() {
                if word_line.contains("<word ") && word_line.contains("</word>") {
                    let wx_min = extract_attr(word_line, "xMin").unwrap_or(0.0);
                    let wy_min = extract_attr(word_line, "yMin").unwrap_or(0.0);
                    let wx_max = extract_attr(word_line, "xMax").unwrap_or(0.0);
                    let wy_max = extract_attr(word_line, "yMax").unwrap_or(0.0);

                    if let (Some(tag_end), Some(close_start)) = (word_line.find('>'), word_line.rfind("</word>")) {
                        if tag_end < close_start {
                            let text = word_line[tag_end + 1..close_start].to_string();
                            words.push(RawWord {
                                text,
                                x_min: wx_min,
                                y_min: wy_min,
                                x_max: wx_max,
                                y_max: wy_max,
                            });
                        }
                    }
                }
            }

            if words.is_empty() {
                continue;
            }

            let mut i = 0;
            while i < words.len() {
                let w_curr = &words[i];

                // Recherche de la boîte d'image correspondante pour w_curr
                let matched_img = image_boxes.iter().find(|b| {
                    (b.x - w_curr.x_min).abs() < 12.0
                        && (b.top_y - w_curr.y_min).abs() < 25.0
                });

                if let Some(img) = matched_img {
                    // Collecte les mots appartenant à cette boîte d'image
                    let mut j = i;
                    while j + 1 < words.len() {
                        let next_word = &words[j + 1];

                        // Si le mot suivant démarre une boîte d'image différente, arrête le groupe actuel
                        let starts_other_img = image_boxes.iter().any(|b2| {
                            (b2.x - img.x).abs() > 2.0
                                && (b2.x - next_word.x_min).abs() < 8.0
                                && (b2.top_y - next_word.y_min).abs() < 25.0
                        });
                        if starts_other_img {
                            break;
                        }

                        // Vérifie si le mot suivant dépasse largement de la boîte d'image
                        if next_word.x_max > img.x + img.width + 15.0 {
                            break;
                        }

                        j += 1;
                    }

                    let word_count = j - i + 1;
                    let group_x_min = words[i].x_min;
                    let group_x_max = words[j].x_max;
                    let group_width = (group_x_max - group_x_min).abs();

                    // Prend en compte le rembourrage transparent des bordures dans les images tramées de PowerPoint
                    let trim_factor = if word_count == 1 {
                        0.855
                    } else {
                        0.935
                    };

                    let target_w = img.width * trim_factor;
                    let raw_scale = if group_width > 0.0 {
                        target_w / group_width
                    } else {
                        1.0
                    };

                    // Limite le facteur d'échelle à des bornes réalistes (ne jamais réduire sous 1.0, plafonner à 1.25 pour éviter les débordements)
                    let scale = if words[i].text.len() <= 1 && word_count == 1 {
                        1.0
                    } else if raw_scale > 1.0 {
                        raw_scale.min(1.25)
                    } else {
                        1.0
                    };

                    for k in i..=j {
                        let w = &words[k];
                        let orig_w = (w.x_max - w.x_min).abs();
                        let orig_h = (w.y_max - w.y_min).abs();

                        let (x, width, font_size) = if scale > 1.0 {
                            let scaled_x = group_x_min + (w.x_min - group_x_min) * scale;
                            let scaled_w = orig_w * scale;
                            let scaled_font = (orig_h * scale.min(1.20)).max(10.0);
                            (scaled_x, scaled_w, scaled_font)
                        } else {
                            (w.x_min, orig_w, orig_h.max(10.0))
                        };

                        spans.push(TextSpan {
                            text: w.text.clone(),
                            x,
                            y: w.y_min,
                            width,
                            height: orig_h,
                            font_size,
                        });
                    }

                    i = j + 1;
                } else {
                    let w = &words[i];
                    let orig_w = (w.x_max - w.x_min).abs();
                    let orig_h = (w.y_max - w.y_min).abs();
                    spans.push(TextSpan {
                        text: w.text.clone(),
                        x: w.x_min,
                        y: w.y_min,
                        width: orig_w,
                        height: orig_h,
                        font_size: orig_h.max(10.0),
                    });
                    i += 1;
                }
            }
        }

        // Repli (fallback) si aucune ligne n'a été détectée
        if spans.is_empty() {
            for line in xml_content.lines() {
                if line.contains("<word ") && line.contains("</word>") {
                    let x_min = extract_attr(line, "xMin").unwrap_or(0.0);
                    let y_min = extract_attr(line, "yMin").unwrap_or(0.0);
                    let x_max = extract_attr(line, "xMax").unwrap_or(0.0);
                    let y_max = extract_attr(line, "yMax").unwrap_or(0.0);

                    if let (Some(tag_end), Some(close_start)) = (line.find('>'), line.rfind("</word>")) {
                        if tag_end < close_start {
                            let text = line[tag_end + 1..close_start].to_string();
                            let width = (x_max - x_min).abs();
                            let height = (y_max - y_min).abs();
                            let font_size = height.max(10.0);

                            spans.push(TextSpan {
                                text,
                                x: x_min,
                                y: y_min,
                                width,
                                height,
                                font_size,
                            });
                        }
                    }
                }
            }
        }

        spans
    }
}
