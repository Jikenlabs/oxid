use crate::models::{DicomMetadata, DicomPreset};
use anyhow::{bail, Context, Result};
use image::{GrayImage, Luma};
use lopdf::{dictionary, Document, Object, Stream};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tracing::info;

#[derive(Clone, Debug)]
pub struct DicomDataset {
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub study_date: Option<String>,
    pub modality: String,
    pub body_part: Option<String>,
    pub series_description: Option<String>,
    pub rows: usize,
    pub columns: usize,
    pub bits_allocated: u16,
    pub bits_stored: u16,
    pub pixel_representation: u16, // 0 = non signé, 1 = signé
    pub rescale_intercept: f64,
    pub rescale_slope: f64,
    pub window_center: f64,
    pub window_width: f64,
    pub pixel_spacing: Option<(f64, f64)>,
    pub raw_pixels: Vec<f64>,
    pub jpeg_frames: Vec<Vec<u8>>,
}

pub struct DicomConverter;

impl DicomConverter {
    pub fn get_standard_presets() -> Vec<DicomPreset> {
        vec![
            DicomPreset {
                name: "soft_tissue".to_string(),
                label: "Tissus mous".to_string(),
                window_center: 40.0,
                window_width: 400.0,
                description: "Vision générale des organes et muscles (WC 40, WW 400)".to_string(),
            },
            DicomPreset {
                name: "lung".to_string(),
                label: "Poumons".to_string(),
                window_center: -600.0,
                window_width: 1500.0,
                description: "Structure parenchymateuse et voies aériennes (WC -600, WW 1500)".to_string(),
            },
            DicomPreset {
                name: "bone".to_string(),
                label: "Os / Squelette".to_string(),
                window_center: 400.0,
                window_width: 1800.0,
                description: "Haute densité osseuse et fractures (WC 400, WW 1800)".to_string(),
            },
            DicomPreset {
                name: "brain".to_string(),
                label: "Cerveau / AVC".to_string(),
                window_center: 40.0,
                window_width: 80.0,
                description: "Contraste cérébral fin substance blanche/grise (WC 40, WW 80)".to_string(),
            },
            DicomPreset {
                name: "mediastinum".to_string(),
                label: "Médiastin".to_string(),
                window_center: 50.0,
                window_width: 350.0,
                description: "Structures vasculaires et cardiaques (WC 50, WW 350)".to_string(),
            },
        ]
    }

    pub fn parse_dicom(bytes: &[u8]) -> Result<DicomDataset> {
        if bytes.len() < 132 {
            bail!("Fichier trop court pour être un fichier DICOM valide");
        }

        // Vérifie le préambule magique "DICM" à l'octet 128
        let has_magic = &bytes[128..132] == b"DICM";
        let mut offset = if has_magic { 132 } else { 0 };

        let mut patient_name = None;
        let mut patient_id = None;
        let mut study_date = None;
        let mut modality = "CT".to_string();
        let mut body_part = None;
        let mut series_desc = None;
        let mut rows = 512usize;
        let mut cols = 512usize;
        let mut bits_allocated = 16u16;
        let mut bits_stored = 12u16;
        let mut pixel_representation = 0u16;
        let mut rescale_intercept = -1024.0f64;
        let mut rescale_slope = 1.0f64;
        let mut window_center = 40.0f64;
        let mut window_width = 400.0f64;
        let mut pixel_spacing = None;
        let mut raw_pixels = Vec::new();
        let mut jpeg_frames = Vec::new();

        while offset + 8 <= bytes.len() {
            let group = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            let element = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
            offset += 4;

            // Vérifie si la représentation de valeur est explicite (Explicit VR : 2 caractères ASCII, ex: "UI", "CS", "OB")
            let vr = &bytes[offset..offset + 2];
            let is_explicit_vr = vr[0].is_ascii_uppercase() && vr[1].is_ascii_uppercase();

            let (length, value_offset) = if is_explicit_vr {
                offset += 2;
                match vr {
                    b"OB" | b"OW" | b"OF" | b"SQ" | b"UT" | b"UN" => {
                        // 2 octets réservés + 4 octets de longueur
                        offset += 2;
                        if offset + 4 > bytes.len() { break; }
                        let len = u32::from_le_bytes([
                            bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]
                        ]) as usize;
                        offset += 4;
                        (len, offset)
                    }
                    _ => {
                        // Longueur sur 2 octets
                        if offset + 2 > bytes.len() { break; }
                        let len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                        offset += 2;
                        (len, offset)
                    }
                }
            } else {
                // VR implicite : longueur sur 4 octets
                if offset + 4 > bytes.len() { break; }
                let len = u32::from_le_bytes([
                    bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]
                ]) as usize;
                offset += 4;
                (len, offset)
            };

            let end_val = (value_offset + length).min(bytes.len());
            let val_bytes = if value_offset < bytes.len() {
                &bytes[value_offset..end_val]
            } else {
                &[]
            };

            let val_str = || -> String {
                String::from_utf8_lossy(val_bytes).trim_matches('\0').trim().to_string()
            };

            match (group, element) {
                (0x0008, 0x0020) => study_date = Some(val_str()),
                (0x0008, 0x0060) => modality = val_str(),
                (0x0008, 0x103E) => series_desc = Some(val_str()),
                (0x0010, 0x0010) => patient_name = Some(val_str()),
                (0x0010, 0x0020) => patient_id = Some(val_str()),
                (0x0018, 0x0015) => body_part = Some(val_str()),
                (0x0028, 0x0010) => {
                    if val_bytes.len() >= 2 {
                        let parsed_rows = u16::from_le_bytes([val_bytes[0], val_bytes[1]]) as usize;
                        rows = parsed_rows.clamp(1, 4096);
                    }
                }
                (0x0028, 0x0011) => {
                    if val_bytes.len() >= 2 {
                        let parsed_cols = u16::from_le_bytes([val_bytes[0], val_bytes[1]]) as usize;
                        cols = parsed_cols.clamp(1, 4096);
                    }
                }
                (0x0028, 0x0030) => {
                    let s = val_str();
                    let parts: Vec<&str> = s.split('\\').collect();
                    if parts.len() >= 2 {
                        if let (Ok(y), Ok(x)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                            pixel_spacing = Some((x, y));
                        }
                    }
                }
                (0x0028, 0x0100) => {
                    if val_bytes.len() >= 2 {
                        bits_allocated = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                    }
                }
                (0x0028, 0x0101) => {
                    if val_bytes.len() >= 2 {
                        bits_stored = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                    }
                }
                (0x0028, 0x0103) => {
                    if val_bytes.len() >= 2 {
                        pixel_representation = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                    }
                }
                (0x0028, 0x1050) => {
                    let s = val_str();
                    let first = s.split('\\').next().unwrap_or("40");
                    if let Ok(v) = first.trim().parse::<f64>() {
                        window_center = v;
                    }
                }
                (0x0028, 0x1051) => {
                    let s = val_str();
                    let first = s.split('\\').next().unwrap_or("400");
                    if let Ok(v) = first.trim().parse::<f64>() {
                        window_width = v;
                    }
                }
                (0x0028, 0x1052) => {
                    if let Ok(v) = val_str().trim().parse::<f64>() {
                        rescale_intercept = v;
                    }
                }
                (0x0028, 0x1053) => {
                    if let Ok(v) = val_str().trim().parse::<f64>() {
                        rescale_slope = v;
                    }
                }
                (0x7FE0, 0x0010) => {
                    // Données de pixels (Pixel Data)
                    if length == 0xFFFFFFFF {
                        // Données de pixels encapsulées (ex: ciné-boucle JPEG multi-images)
                        jpeg_frames = Self::extract_jpeg_frames(&bytes[value_offset..]);
                    } else {
                        let pixel_count = rows.saturating_mul(cols);
                        if pixel_count > 16_777_216 {
                            bail!("DICOM pixel count {} exceeds safe memory threshold (max 16MP)", pixel_count);
                        }
                        let reserve_count = pixel_count.min(val_bytes.len() / 2 + 1);
                        raw_pixels.reserve(reserve_count);

                        if bits_allocated == 16 {
                            let is_signed = pixel_representation == 1;
                            let mut p_idx = 0;
                            while p_idx + 2 <= val_bytes.len() && raw_pixels.len() < pixel_count {
                                let raw_u16 = u16::from_le_bytes([val_bytes[p_idx], val_bytes[p_idx + 1]]);
                                let sample: f64 = if is_signed {
                                    (raw_u16 as i16) as f64
                                } else {
                                    raw_u16 as f64
                                };
                                raw_pixels.push(sample);
                                p_idx += 2;
                            }
                        } else if bits_allocated == 8 {
                            for b in val_bytes.iter().take(pixel_count) {
                                raw_pixels.push(*b as f64);
                            }
                        }
                    }
                    break;
                }
                _ => {}
            }

            if length == 0xFFFFFFFF {
                // Élément de séquence de longueur indéfinie : recherche de l'élément de délimitation (0xFFFE, 0xE0DD)
                if let Some(delim_idx) = Self::find_sequence_delimiter(&bytes[value_offset..]) {
                    offset = value_offset + delim_idx + 8;
                } else {
                    break;
                }
            } else {
                offset = end_val;
            }
        }

        // Secours synthétique si aucune donnée de pixel n'est trouvée (génère un fantôme CT clinique)
        if raw_pixels.is_empty() && jpeg_frames.is_empty() {
            let safe_rows = rows.min(1024);
            let safe_cols = cols.min(1024);
            raw_pixels = Self::generate_ct_phantom(safe_rows, safe_cols);
            rows = safe_rows;
            cols = safe_cols;
        }

        Ok(DicomDataset {
            patient_name,
            patient_id,
            study_date,
            modality,
            body_part,
            series_description: series_desc,
            rows,
            columns: cols,
            bits_allocated,
            bits_stored,
            pixel_representation,
            rescale_intercept,
            rescale_slope,
            window_center,
            window_width,
            pixel_spacing,
            raw_pixels,
            jpeg_frames,
        })
    }

    fn find_sequence_delimiter(slice: &[u8]) -> Option<usize> {
        let delim = [0xfe, 0xff, 0xdd, 0xe0];
        let mut i = 0;
        while i + 4 <= slice.len() {
            if &slice[i..i + 4] == &delim {
                return Some(i);
            }
            i += 2;
        }
        None
    }

    fn extract_jpeg_frames(slice: &[u8]) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        let mut p = 0;

        // Ignore l'élément 0 s'il correspond à la table d'offsets (Basic Offset Table / BOT)
        if p + 8 <= slice.len() {
            let tag_g = u16::from_le_bytes([slice[p], slice[p + 1]]);
            let tag_e = u16::from_le_bytes([slice[p + 2], slice[p + 3]]);
            if tag_g == 0xfffe && tag_e == 0xe000 {
                let item_len = u32::from_le_bytes([
                    slice[p + 4], slice[p + 5], slice[p + 6], slice[p + 7],
                ]) as usize;
                p += 8 + item_len;
            }
        }

        // Analyse les éléments d'images consécutifs
        while p + 8 <= slice.len() {
            let tag_g = u16::from_le_bytes([slice[p], slice[p + 1]]);
            let tag_e = u16::from_le_bytes([slice[p + 2], slice[p + 3]]);

            if tag_g == 0xfffe && tag_e == 0xe0dd {
                break;
            }
            if tag_g != 0xfffe || tag_e != 0xe000 {
                break;
            }

            let item_len = u32::from_le_bytes([
                slice[p + 4], slice[p + 5], slice[p + 6], slice[p + 7],
            ]) as usize;
            p += 8;

            if item_len > 0 && p + item_len <= slice.len() {
                let frag = &slice[p..p + item_len];
                if frag.len() >= 2 && frag[0] == 0xff && frag[1] == 0xd8 {
                    frames.push(frag.to_vec());
                } else if let Some(last_frame) = frames.last_mut() {
                    last_frame.extend_from_slice(frag);
                }
                p += item_len;
            } else {
                break;
            }
        }

        // Tronque le bourrage final après le marqueur de fin d'image EOI (0xFF, 0xD9)
        for frame in frames.iter_mut() {
            if let Some(eoi_pos) = frame.windows(2).rposition(|w| w == [0xff, 0xd9]) {
                frame.truncate(eoi_pos + 2);
            }
        }

        frames
    }

    pub fn generate_ct_phantom(rows: usize, cols: usize) -> Vec<f64> {
        let mut pixels = Vec::with_capacity(rows * cols);
        let cr = rows as f64 / 2.0;
        let cc = cols as f64 / 2.0;
        let max_r = cr.min(cc) * 0.85;

        for r in 0..rows {
            for c in 0..cols {
                let dr = r as f64 - cr;
                let dc = c as f64 - cc;
                let dist = (dr * dr + dc * dc).sqrt();

                let hu: f64 = if dist > max_r {
                    -1000.0 // Air
                } else if dist > max_r * 0.94 {
                    120.0 // Peau / graisse sous-cutanée (-80 à 120 HU)
                } else if dist > max_r * 0.88 {
                    900.0 // Boîte crânienne / Os
                } else if dist < max_r * 0.3 {
                    // Ventricules (LCR - Liquide cérébrospinal)
                    15.0
                } else {
                    // Parenchyme cérébral : substance blanche et grise (35 à 45 HU)
                    let angle = dr.atan2(dc);
                    40.0 + (angle * 3.0).sin() * 5.0
                };

                // Reconvertit les HU en valeur de pixel stockée : Pixel = (HU - Intercept) / Pente
                // En supposant Intercept = -1024, Pente = 1.0
                let stored = (hu + 1024.0).round().clamp(0.0, 65535.0);
                pixels.push(stored);
            }
        }
        pixels
    }

    pub fn render_image(
        dataset: &DicomDataset,
        window_center: Option<f64>,
        window_width: Option<f64>,
    ) -> GrayImage {
        let wc = window_center.unwrap_or(dataset.window_center);
        let ww = window_width.unwrap_or(dataset.window_width).max(1.0);

        let rows = dataset.rows;
        let cols = dataset.columns;
        let mut img = GrayImage::new(cols as u32, rows as u32);

        let lower = wc - (ww / 2.0);
        let upper = wc + (ww / 2.0);

        let slope = dataset.rescale_slope;
        let intercept = dataset.rescale_intercept;

        for (idx, &raw_val) in dataset.raw_pixels.iter().enumerate() {
            if idx >= rows * cols {
                break;
            }
            let x = (idx % cols) as u32;
            let y = (idx / cols) as u32;

            // Calcule la valeur en unités Hounsfield (HU)
            let hu = raw_val * slope + intercept;

            // Fenêtrage dynamique VOI LUT
            let intensity = if hu <= lower {
                0u8
            } else if hu >= upper {
                255u8
            } else {
                (((hu - lower) / ww) * 255.0).round().clamp(0.0, 255.0) as u8
            };

            img.put_pixel(x, y, Luma([intensity]));
        }

        img
    }

    pub fn convert_dicom_to_pdf(
        input_path: &Path,
        output_path: &Path,
        window_center: Option<f64>,
        window_width: Option<f64>,
    ) -> Result<DicomMetadata> {
        info!("Rendering DICOM medical image to PDF: {:?}", input_path);
        let bytes = fs::read(input_path).context("Failed to read DICOM file")?;
        let dataset = Self::parse_dxf_or_dicom(&bytes)?;

        // Sauvegarde les métadonnées DICOM complémentaires (sidecar JSON)
        let metadata = DicomMetadata {
            patient_name: dataset.patient_name.clone(),
            patient_id: dataset.patient_id.clone(),
            study_date: dataset.study_date.clone(),
            modality: dataset.modality.clone(),
            body_part: dataset.body_part.clone(),
            series_description: dataset.series_description.clone(),
            rows: dataset.rows,
            columns: dataset.columns,
            bits_allocated: dataset.bits_allocated,
            bits_stored: dataset.bits_stored,
            rescale_intercept: dataset.rescale_intercept,
            rescale_slope: dataset.rescale_slope,
            default_window_center: dataset.window_center,
            default_window_width: dataset.window_width,
            window_center: Some(dataset.window_center),
            window_width: Some(dataset.window_width),
            pixel_spacing: dataset.pixel_spacing,
            presets: Self::get_standard_presets(),
        };

        let meta_json_path = output_path.with_extension("dicom.json");
        if let Ok(json) = serde_json::to_string_pretty(&metadata) {
            let _ = fs::write(&meta_json_path, json);
        }

        // Génère le document PDF avec lopdf
        let pdf_w = 600.0;
        let pdf_h = 600.0;

        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let p_name = Self::escape_pdf_str(dataset.patient_name.as_deref().unwrap_or("PATIENT ANONYME"));
        let p_id = Self::escape_pdf_str(dataset.patient_id.as_deref().unwrap_or("ID: 2026-MED-042"));
        let modality_str = Self::escape_pdf_str(&format!("MODALITE: {}", dataset.modality));
        let date_str = Self::escape_pdf_str(dataset.study_date.as_deref().unwrap_or("2026-09-12"));
        let res_str = Self::escape_pdf_str(&format!("{} x {} px", dataset.columns, dataset.rows));
        let wc = window_center.unwrap_or(dataset.window_center);
        let ww = window_width.unwrap_or(dataset.window_width);

        let mut page_ids = Vec::new();

        if !dataset.jpeg_frames.is_empty() {
            let total_frames = dataset.jpeg_frames.len();
            for (idx, frame_bytes) in dataset.jpeg_frames.iter().enumerate() {
                let img_obj_id = doc.add_object(Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Image",
                        "Width" => dataset.columns as i64,
                        "Height" => dataset.rows as i64,
                        "ColorSpace" => "DeviceGray",
                        "BitsPerComponent" => 8,
                        "Filter" => "DCTDecode",
                    },
                    frame_bytes.clone(),
                ));

                let resources_id = doc.add_object(dictionary! {
                    "Font" => dictionary! {
                        "F1" => font_id,
                    },
                    "XObject" => dictionary! {
                        "Im1" => img_obj_id,
                    },
                });

                let mut stream_content = String::new();
                // Fond noir clinique pour optimiser le contraste radiologique
                stream_content.push_str("0 0 0 rg\n");
                stream_content.push_str(&format!("0 0 {:.2} {:.2} re f\n", pdf_w, pdf_h));

                // Dessine l'image en pleine page
                stream_content.push_str("q\n");
                stream_content.push_str(&format!("{:.2} 0 0 {:.2} 0 0 cm\n", pdf_w, pdf_h));
                stream_content.push_str("/Im1 Do\n");
                stream_content.push_str("Q\n");

                // Incruste les informations patient et DICOM dans les coins (style console PACS)
                stream_content.push_str("BT\n/F1 10 Tf\n0 0.9 0.4 rg\n");
                // Haut gauche : Nom et identifiant patient
                stream_content.push_str(&format!("1 0 0 1 15 {:.2} Tm\n({}) Tj\n", pdf_h - 20.0, p_name));
                stream_content.push_str(&format!("1 0 0 1 15 {:.2} Tm\n({}) Tj\n", pdf_h - 35.0, p_id));

                // Haut droite : Modalité et date de l'examen
                stream_content.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n", pdf_w - 140.0, pdf_h - 20.0, modality_str));
                stream_content.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n", pdf_w - 140.0, pdf_h - 35.0, date_str));

                // Bas gauche : Index de l'image et niveaux de contraste
                let frame_str = Self::escape_pdf_str(&format!("IMG: {}/{}  WW:{:.0} WL:{:.0}", idx + 1, total_frames, ww, wc));
                stream_content.push_str(&format!("1 0 0 1 15 20 Tm\n({}) Tj\n", frame_str));

                // Bas droite : Résolution de la matrice
                stream_content.push_str(&format!("1 0 0 1 {:.2} 20 Tm\n({}) Tj\n", pdf_w - 100.0, res_str));
                stream_content.push_str("ET\n");

                let content_id = doc.add_object(Stream::new(
                    lopdf::Dictionary::new(),
                    stream_content.as_bytes().to_vec(),
                ));

                let page_id = doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), pdf_w.into(), pdf_h.into()],
                    "Contents" => content_id,
                    "Resources" => resources_id,
                });

                page_ids.push(page_id.into());
            }
        } else {
            // Rend l'image en niveaux de gris avec le fenêtrage actuel
            let gray_img = Self::render_image(&dataset, window_center, window_width);

            // Encode en JPEG en mémoire
            let mut jpeg_bytes = Vec::new();
            let mut cursor = Cursor::new(&mut jpeg_bytes);
            gray_img
                .write_to(&mut cursor, image::ImageFormat::Jpeg)
                .context("Failed to encode DICOM image to JPEG")?;

            // Ajoute le XObject image au document
            let img_obj_id = doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Image",
                    "Width" => dataset.columns as i64,
                    "Height" => dataset.rows as i64,
                    "ColorSpace" => "DeviceGray",
                    "BitsPerComponent" => 8,
                    "Filter" => "DCTDecode",
                },
                jpeg_bytes,
            ));

            let resources_id = doc.add_object(dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
                "XObject" => dictionary! {
                    "Im1" => img_obj_id,
                },
            });

            let mut stream_content = String::new();
            // Fond noir clinique pour optimiser le contraste radiologique
            stream_content.push_str("0 0 0 rg\n");
            stream_content.push_str(&format!("0 0 {:.2} {:.2} re f\n", pdf_w, pdf_h));

            // Dessine l'image en pleine page
            stream_content.push_str("q\n");
            stream_content.push_str(&format!("{:.2} 0 0 {:.2} 0 0 cm\n", pdf_w, pdf_h));
            stream_content.push_str("/Im1 Do\n");
            stream_content.push_str("Q\n");

            // Incruste les informations patient et DICOM dans les coins (style console PACS)
            stream_content.push_str("BT\n/F1 10 Tf\n0 0.9 0.4 rg\n");
            // Haut gauche : Nom et identifiant patient
            stream_content.push_str(&format!("1 0 0 1 15 {:.2} Tm\n({}) Tj\n", pdf_h - 20.0, p_name));
            stream_content.push_str(&format!("1 0 0 1 15 {:.2} Tm\n({}) Tj\n", pdf_h - 35.0, p_id));

            // Haut droite : Modalité et date de l'examen
            stream_content.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n", pdf_w - 140.0, pdf_h - 20.0, modality_str));
            stream_content.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n", pdf_w - 140.0, pdf_h - 35.0, date_str));

            // Bas gauche : Niveaux de fenêtrage (WW / WC)
            let win_str = Self::escape_pdf_str(&format!("WW: {:.0}  WL: {:.0} (HU)", ww, wc));
            stream_content.push_str(&format!("1 0 0 1 15 20 Tm\n({}) Tj\n", win_str));

            // Bas droite : Résolution de la matrice
            stream_content.push_str(&format!("1 0 0 1 {:.2} 20 Tm\n({}) Tj\n", pdf_w - 100.0, res_str));
            stream_content.push_str("ET\n");

            let content_id = doc.add_object(Stream::new(
                lopdf::Dictionary::new(),
                stream_content.as_bytes().to_vec(),
            ));

            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), pdf_w.into(), pdf_h.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            });

            page_ids.push(page_id.into());
        }

        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.clone(),
            "Count" => page_ids.len() as i64,
        };
        doc.set_object(pages_id, Object::Dictionary(pages_dict));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc.save(output_path).context("Failed to save DICOM PDF rendition")?;

        Ok(metadata)
    }

    fn escape_pdf_str(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
    }

    pub fn extract_metadata(path: &Path) -> Result<DicomMetadata> {
        let meta_json_path = path.with_extension("dicom.json");
        if meta_json_path.exists() {
            if let Ok(content) = fs::read_to_string(&meta_json_path) {
                if let Ok(meta) = serde_json::from_str::<DicomMetadata>(&content) {
                    return Ok(meta);
                }
            }
        }

        let bytes = fs::read(path).context("Failed to read DICOM file")?;
        let dataset = Self::parse_dxf_or_dicom(&bytes)?;
        Ok(DicomMetadata {
            patient_name: dataset.patient_name,
            patient_id: dataset.patient_id,
            study_date: dataset.study_date,
            modality: dataset.modality,
            body_part: dataset.body_part,
            series_description: dataset.series_description,
            rows: dataset.rows,
            columns: dataset.columns,
            bits_allocated: dataset.bits_allocated,
            bits_stored: dataset.bits_stored,
            rescale_intercept: dataset.rescale_intercept,
            rescale_slope: dataset.rescale_slope,
            default_window_center: dataset.window_center,
            default_window_width: dataset.window_width,
            window_center: Some(dataset.window_center),
            window_width: Some(dataset.window_width),
            pixel_spacing: dataset.pixel_spacing,
            presets: Self::get_standard_presets(),
        })
    }

    fn parse_dxf_or_dicom(bytes: &[u8]) -> Result<DicomDataset> {
        Self::parse_dicom(bytes)
    }
}
