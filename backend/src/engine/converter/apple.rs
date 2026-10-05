use anyhow::{bail, Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::info;

pub struct AppleImageConverter;

impl AppleImageConverter {
    pub fn is_apple_format(ext: &str) -> bool {
        matches!(
            ext,
            "heic" | "heif" | "icns" | "dng" | "pict" | "pct"
        )
    }

    pub fn convert_to_pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        info!("Converting Apple/Mac format .{} to PDF rendition: {}", ext, input_path.display());

        // 1. Tente la commande native macOS 'sips' si disponible ou sur environnement macOS
        if let Ok(status) = Command::new("sips")
            .arg("-s")
            .arg("format")
            .arg("pdf")
            .arg(input_path)
            .arg("--out")
            .arg(output_path)
            .status()
        {
            if status.success() && output_path.exists() {
                return Ok(());
            }
        }

        // 2. Gestionnaire spécifique pour le format d'icônes ICNS en pur Rust
        if ext == "icns" {
            if let Ok(()) = Self::convert_icns_to_pdf(input_path, output_path) {
                return Ok(());
            }
        }

        // 3. Pour HEIC / HEIF / DNG / PICT : conversion en PNG/JPEG intermédiaire puis intégration dans un PDF
        let temp_dir = tempfile::tempdir().context("Failed to create temporary directory for Apple image conversion")?;
        let temp_img = temp_dir.path().join("converted.png");

        // Tente sips (vers PNG)
        let sips_ok = Command::new("sips")
            .arg("-s")
            .arg("format")
            .arg("png")
            .arg(input_path)
            .arg("--out")
            .arg(&temp_img)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        // Tente heif-convert (pour HEIC/HEIF)
        let heif_ok = if !sips_ok && (ext == "heic" || ext == "heif") {
            Command::new("heif-convert")
                .arg(input_path)
                .arg(&temp_img)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };

        // Tente gdk-pixbuf-thumbnailer (pour les formats HEIC/HEIF/Apple sous Linux via heif-gdk-pixbuf)
        let gdk_ok = if !sips_ok && !heif_ok {
            Command::new("gdk-pixbuf-thumbnailer")
                .arg("-s")
                .arg("4096")
                .arg(input_path)
                .arg(&temp_img)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };

        // Tente ffmpeg
        let ffmpeg_ok = if !sips_ok && !heif_ok && !gdk_ok {
            Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(input_path)
                .arg(&temp_img)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };

        // Tente ImageMagick 'convert'
        let convert_ok = if !sips_ok && !heif_ok && !gdk_ok && !ffmpeg_ok {
            Command::new("convert")
                .arg(input_path)
                .arg(&temp_img)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };

        if (sips_ok || heif_ok || gdk_ok || ffmpeg_ok || convert_ok) && temp_img.exists() {
            Self::embed_image_file_to_pdf(&temp_img, output_path)?;
            return Ok(());
        }

        bail!(
            "Impossible de convertir l'image Apple/Mac '{}' (format .{}). Veuillez installer 'libheif-examples' (heif-convert) ou 'ffmpeg'.",
            input_path.display(),
            ext
        )
    }

    /// Extrait l'image PNG de plus haute résolution intégrée dans le conteneur d'icônes Apple ICNS.
    fn convert_icns_to_pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        let bytes = fs::read(input_path).context("Failed to read ICNS file")?;
        if bytes.len() < 8 || &bytes[0..4] != b"icns" {
            bail!("Invalid ICNS header");
        }

        let mut offset = 8;
        let mut best_png: Option<Vec<u8>> = None;
        let mut max_len = 0;

        while offset + 8 <= bytes.len() {
            if offset + 8 > bytes.len() {
                break;
            }
            let chunk_len = u32::from_be_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            if chunk_len < 8 || offset + chunk_len > bytes.len() {
                break;
            }

            let chunk_data = &bytes[offset + 8..offset + chunk_len];

            // Vérifie si le bloc de données est une image PNG (\x89PNG\r\n\x1a\n)
            if chunk_data.len() > 8 && &chunk_data[0..4] == b"\x89PNG" {
                if chunk_data.len() > max_len {
                    max_len = chunk_data.len();
                    best_png = Some(chunk_data.to_vec());
                }
            }

            offset += chunk_len;
        }

        if let Some(png_bytes) = best_png {
            let temp_dir = tempfile::tempdir()?;
            let temp_png = temp_dir.path().join("icon.png");
            fs::write(&temp_png, png_bytes)?;
            Self::embed_image_file_to_pdf(&temp_png, output_path)?;
            return Ok(());
        }

        bail!("No valid PNG icons found inside ICNS")
    }

    /// Intègre un fichier image en tant que page unique d'un document PDF.
    pub fn embed_image_file_to_pdf(img_path: &Path, output_path: &Path) -> Result<()> {
        let dynamic_img = image::open(img_path).context("Failed to load converted image for PDF embedding")?;
        let rgba = dynamic_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        if w == 0 || h == 0 || w > 8192 || h > 8192 {
            bail!("Dimensions d'image invalides ou excessives pour l'incorporation PDF: {}x{}", w, h);
        }

        let total_bytes = (w as usize).saturating_mul(h as usize).saturating_mul(3);
        let mut rgb_bytes = Vec::with_capacity(total_bytes);
        for pixel in rgba.pixels() {
            let alpha = pixel[3] as f32 / 255.0;
            let r = ((pixel[0] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
            let g = ((pixel[1] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
            let b = ((pixel[2] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
            rgb_bytes.push(r);
            rgb_bytes.push(g);
            rgb_bytes.push(b);
        }

        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let mut img_dict = lopdf::Dictionary::new();
        img_dict.set("Type", Object::Name(b"XObject".to_vec()));
        img_dict.set("Subtype", Object::Name(b"Image".to_vec()));
        img_dict.set("Width", Object::Integer(w as i64));
        img_dict.set("Height", Object::Integer(h as i64));
        img_dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
        img_dict.set("BitsPerComponent", Object::Integer(8));

        let mut stream = Stream::new(img_dict, rgb_bytes);
        let _ = stream.compress();
        let img_id = doc.add_object(Object::Stream(stream));

        let res_dict = dictionary! {
            "XObject" => dictionary! {
                "Im1" => img_id,
            },
        };
        let resources_id = doc.add_object(res_dict);

        // Dimensionnement en points standard (adaptation au ratio d'aspect naturel)
        let page_w = 595.0f64; // Largeur A4
        let scale = page_w / (w as f64).max(1.0);
        let page_h = ((h as f64) * scale).clamp(10.0, 14400.0);

        let content_stream = format!(
            "q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im1 Do\nQ\n",
            page_w, page_h
        );
        let content_id = doc.add_object(Stream::new(dictionary!(), content_stream.into_bytes()));

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        };
        let page_id = doc.add_object(page_dict);

        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        doc.save(output_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_icns_extraction_and_conversion() {
        let temp_dir = tempfile::tempdir().unwrap();
        let icns_path = temp_dir.path().join("app.icns");
        let pdf_out = temp_dir.path().join("app_icon.pdf");

        // 1. Generate a small valid PNG in memory
        let img = image::RgbaImage::new(32, 32);
        let mut png_buf = Cursor::new(Vec::new());
        img.write_to(&mut png_buf, image::ImageFormat::Png).unwrap();
        let png_bytes = png_buf.into_inner();

        // 2. Wrap it inside a valid ICNS file
        let chunk_type = b"ic05"; // 32x32 PNG icon
        let chunk_len = (8 + png_bytes.len()) as u32;
        let total_file_len = (8 + 8 + png_bytes.len()) as u32;

        let mut icns_data = Vec::new();
        icns_data.extend_from_slice(b"icns");
        icns_data.extend_from_slice(&total_file_len.to_be_bytes());
        icns_data.extend_from_slice(chunk_type);
        icns_data.extend_from_slice(&chunk_len.to_be_bytes());
        icns_data.extend_from_slice(&png_bytes);

        fs::write(&icns_path, &icns_data).unwrap();

        // 3. Test convert_to_pdf
        AppleImageConverter::convert_to_pdf(&icns_path, &pdf_out).expect("Conversion ICNS -> PDF failed");

        assert!(pdf_out.exists(), "Output PDF must exist");
        let meta = std::fs::metadata(&pdf_out).unwrap();
        assert!(meta.len() > 200, "PDF must not be empty");
    }
}
