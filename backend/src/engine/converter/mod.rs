pub mod apple;
pub mod cad;
pub mod dicom;
pub mod email;
pub mod office;
pub mod text;

use crate::models::EmailAttachment;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::info;

pub struct FormatConverter;

pub struct RenditionResult {
    pub effective_path: PathBuf,
    pub original_format: String,
    pub is_converted: bool,
    pub attachments: Vec<EmailAttachment>,
}

impl FormatConverter {
    pub fn ensure_pdf_rendition(path: &Path) -> Result<RenditionResult> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Format PDF natif ou images directes
        if ext == "pdf"
            || ext == "png"
            || ext == "jpg"
            || ext == "jpeg"
            || ext == "webp"
            || ext == "tiff"
            || ext == "tif"
            || ext == "bmp"
            || ext == "gif"
        {
            return Ok(RenditionResult {
                effective_path: path.to_path_buf(),
                original_format: ext,
                is_converted: false,
                attachments: vec![],
            });
        }

        // Multimédia : formats vidéo avec extraction automatique d'affiche (poster)
        if Self::is_video(&ext) {
            let poster_path = path.with_extension(format!("{}.poster.jpg", ext));
            if !poster_path.exists() {
                // Génère une vignette d'affiche vidéo à 1s (ou 0s) avec ffmpeg
                let _ = std::process::Command::new("ffmpeg")
                    .arg("-y")
                    .arg("-ss").arg("00:00:01")
                    .arg("-i").arg(path)
                    .arg("-vframes").arg("1")
                    .arg("-q:v").arg("2")
                    .arg(&poster_path)
                    .status();

                if !poster_path.exists() {
                    let _ = std::process::Command::new("ffmpeg")
                        .arg("-y")
                        .arg("-i").arg(path)
                        .arg("-vframes").arg("1")
                        .arg("-q:v").arg("2")
                        .arg(&poster_path)
                        .status();
                }
            }

            return Ok(RenditionResult {
                effective_path: if poster_path.exists() { poster_path } else { path.to_path_buf() },
                original_format: ext,
                is_converted: false,
                attachments: vec![],
            });
        }

        // Multimédia : formats audio
        if Self::is_audio(&ext) {
            return Ok(RenditionResult {
                effective_path: path.to_path_buf(),
                original_format: ext,
                is_converted: false,
                attachments: vec![],
            });
        }

        // Chemin du PDF de rendu en cache
        let rendition_path = path.with_extension(format!("{}.rendition.pdf", ext));
        if rendition_path.exists() {
            return Ok(RenditionResult {
                effective_path: rendition_path,
                original_format: ext,
                is_converted: true,
                attachments: vec![],
            });
        }

        info!("Converting document of type .{} to PDF rendition...", ext);

        let mut attachments = Vec::new();

        match ext.as_str() {
            "eml" | "msg" => {
                let res = email::EmailConverter::convert_eml_to_pdf(path, &rendition_path)?;
                attachments = res.attachments;
            }
            // Documents bureautiques et schémas (Visio VSD/VSDX, OpenDocument Draw ODG/ODT/ODS/ODP)
            "docx" | "doc" | "xlsx" | "xls" | "pptx" | "ppt"
            | "odt" | "ods" | "odp" | "odg" | "rtf"
            | "vsd" | "vsdx" => {
                office::OfficeConverter::convert_to_pdf(path, &rendition_path)?;
            }
            // CAO / DAO (AutoCAD DXF & DWG avec calques)
            "dxf" | "dwg" => {
                cad::CadConverter::convert_cad_to_pdf(path, &rendition_path, None)?;
            }
            // Imagerie médicale DICOM (scanner, IRM, radiographie avec fenêtrage Hounsfield)
            "dcm" | "dicom" => {
                dicom::DicomConverter::convert_dicom_to_pdf(path, &rendition_path, None, None)?;
            }
            // Dessin vectoriel SVG
            "svg" => {
                text::TextConverter::convert_text_to_pdf(path, &rendition_path)?;
            }
            // Formats d'image Apple et Mac (HEIC, HEIF, ICNS, DNG, PICT)
            "heic" | "heif" | "icns" | "dng" | "pict" | "pct" => {
                apple::AppleImageConverter::convert_to_pdf(path, &rendition_path)?;
            }
            // Markdown avec diagrammes Mermaid
            "md" | "markdown" => {
                text::TextConverter::convert_markdown_to_pdf(path, &rendition_path)?;
            }
            "txt" | "csv" | "tsv" | "log" | "json" | "xml" | "yaml" | "yml" => {
                text::TextConverter::convert_text_to_pdf(path, &rendition_path)?;
            }
            _ => {
                // Tentative de conversion textuelle de secours pour tout fichier textuel ou inconnu
                text::TextConverter::convert_text_to_pdf(path, &rendition_path)?;
            }
        }

        Ok(RenditionResult {
            effective_path: rendition_path,
            original_format: ext,
            is_converted: true,
            attachments,
        })
    }

    pub fn is_video(ext: &str) -> bool {
        matches!(
            ext,
            "mp4" | "webm" | "ogv" | "ogg" | "mov" | "avi" | "mkv" | "m4v" | "3gp"
        )
    }

    pub fn is_audio(ext: &str) -> bool {
        matches!(ext, "mp3" | "wav" | "flac" | "aac" | "m4a")
    }
}
