use anyhow::{bail, Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use tracing::info;
use zip::ZipArchive;

#[derive(Debug)]
pub enum DocxBlock {
    Paragraph {
        text: String,
        is_heading: bool,
    },
    Image {
        rel_id: String,
        cx_pt: Option<f64>,
        cy_pt: Option<f64>,
    },
}

struct PreloadedImage {
    obj_id: lopdf::ObjectId,
    width_px: u32,
    height_px: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeEnginePreference {
    Hybrid,
    Office2Pdf,
    Gotenberg,
}

impl OfficeEnginePreference {
    pub fn from_env() -> Self {
        match std::env::var("OXID_OFFICE_ENGINE")
            .unwrap_or_else(|_| "hybrid".to_string())
            .to_lowercase()
            .as_str()
        {
            "office2pdf" | "typst" => OfficeEnginePreference::Office2Pdf,
            "gotenberg" | "libreoffice" | "soffice" => OfficeEnginePreference::Gotenberg,
            _ => OfficeEnginePreference::Hybrid,
        }
    }
}

pub struct OfficeConverter;

impl OfficeConverter {
    pub fn convert_to_pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let engine = OfficeEnginePreference::from_env();
        let is_office2pdf_compatible = ext == "docx" || ext == "xlsx" || ext == "pptx";

        // Mode 1: Hybrid (default) -> office2pdf for standard office docs, Gotenberg for complex/large layouts
        if engine == OfficeEnginePreference::Hybrid {
            let file_len = fs::metadata(input_path).map(|m| m.len()).unwrap_or(0);
            let has_complex_wrapping = ext == "docx" && Self::docx_has_complex_wrapping(input_path);

            // Pour les documents bureautiques standards (< 5 Mo et sans habillage vectoriel complexe) -> office2pdf
            if is_office2pdf_compatible && file_len < 5 * 1024 * 1024 && !has_complex_wrapping {
                if let Ok(()) = Self::convert_via_office2pdf(input_path, output_path) {
                    info!("Successfully converted .{} to PDF via ultra-fast office2pdf engine (Rust/Typst)", ext);
                    return Ok(());
                }
                info!("office2pdf unavailable or skipped for .{}; falling back to Gotenberg/LibreOffice", ext);
            } else if is_office2pdf_compatible && (file_len >= 5 * 1024 * 1024 || has_complex_wrapping) {
                info!("Document .{} contains complex layout/wrapping or large media ({} bytes); routing directly to high-fidelity Gotenberg/LibreOffice", ext, file_len);
            }

            if let Ok(()) = Self::convert_via_soffice(input_path, output_path) {
                info!("Successfully converted .{} to PDF via high-fidelity LibreOffice engine", ext);
                return Ok(());
            }

            // Fallback ultime sur office2pdf si soffice échoue
            if is_office2pdf_compatible {
                if let Ok(()) = Self::convert_via_office2pdf(input_path, output_path) {
                    info!("Successfully converted .{} to PDF via office2pdf (fallback after soffice)", ext);
                    return Ok(());
                }
            }
        }
        // Mode 2: Force Office2Pdf
        else if engine == OfficeEnginePreference::Office2Pdf {
            if is_office2pdf_compatible {
                if let Ok(()) = Self::convert_via_office2pdf(input_path, output_path) {
                    info!("Successfully converted .{} to PDF via office2pdf engine", ext);
                    return Ok(());
                }
            }
            // If office2pdf can't handle legacy formats like .doc, fall back to soffice
            if let Ok(()) = Self::convert_via_soffice(input_path, output_path) {
                info!("Successfully converted .{} to PDF via LibreOffice engine (fallback for unhandled format)", ext);
                return Ok(());
            }
        }
        // Mode 3: Force Gotenberg / LibreOffice
        else if engine == OfficeEnginePreference::Gotenberg {
            if let Ok(()) = Self::convert_via_soffice(input_path, output_path) {
                info!("Successfully converted .{} to PDF via high-fidelity LibreOffice engine", ext);
                return Ok(());
            }
        }

        // Pure Rust native fallback parsers for DOCX, XLSX, ODG, VSDX, ODT
        if ext == "docx" {
            return Self::convert_docx_native(input_path, output_path);
        }

        if ext == "xlsx" {
            return Self::convert_xlsx_native(input_path, output_path);
        }

        if ext == "odg" || ext == "vsdx" || ext == "odt" || ext == "ods" || ext == "odp" {
            if let Ok(()) = Self::convert_archive_xml_native(input_path, output_path, &ext) {
                return Ok(());
            }
        }

        bail!("Office/Diagram document format .{} requires office2pdf, LibreOffice or native parser", ext);
    }

    pub fn get_office2pdf_runner() -> Option<String> {
        // 1. bin/office2pdf in project or relative paths
        for candidate in &[
            "bin/office2pdf",
            "./bin/office2pdf",
            "../bin/office2pdf",
            "/usr/local/bin/office2pdf",
            "/tmp/office2pdf_install/office2pdf-v0.6.8-x86_64-unknown-linux-gnu/office2pdf",
        ] {
            if Path::new(candidate).exists() {
                if Command::new(candidate)
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return Some(candidate.to_string());
                }
            }
        }

        // 2. System PATH
        if Command::new("office2pdf")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some("office2pdf".to_string());
        }

        None
    }

    /// Détecte si le DOCX utilise un habillage de texte complexe (Tight, Square, Polygon, Anchor)
    /// qui nécessite le moteur de calcul graphique complet de LibreOffice.
    pub fn docx_has_complex_wrapping(path: &Path) -> bool {
        if let Ok(file) = File::open(path) {
            if let Ok(mut archive) = ZipArchive::new(file) {
                if let Ok(mut doc_xml) = archive.by_name("word/document.xml") {
                    let mut content = Vec::new();
                    if doc_xml.read_to_end(&mut content).is_ok() {
                        let text = String::from_utf8_lossy(&content);
                        if text.contains("wrapTight")
                            || text.contains("wrapPolygon")
                            || text.contains("wrapSquare")
                            || text.contains("wrapThrough")
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn convert_via_office2pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        let runner = Self::get_office2pdf_runner()
            .context("No office2pdf binary available")?;

        let abs_input = input_path
            .canonicalize()
            .unwrap_or_else(|_| input_path.to_path_buf());

        let temp_dir = tempfile::tempdir()?;
        let temp_pdf = temp_dir.path().join("output.pdf");

        let status = Command::new(&runner)
            .arg(&abs_input)
            .arg("-o")
            .arg(&temp_pdf)
            .status()
            .context("Failed to spawn office2pdf process")?;

        if !status.success() {
            bail!("office2pdf conversion exited with failure status: {}", status);
        }

        if !temp_pdf.exists() || fs::metadata(&temp_pdf)?.len() == 0 {
            bail!("office2pdf finished but did not produce a non-empty PDF file");
        }

        fs::copy(&temp_pdf, output_path)?;
        Ok(())
    }

    fn get_soffice_runner() -> Option<(String, Vec<String>)> {
        // 1. Host or PATH soffice
        if Command::new("soffice")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(("soffice".to_string(), vec![]));
        }

        // 2. Project bin/soffice wrapper script
        for candidate in &["bin/soffice", "../bin/soffice", "./bin/soffice"] {
            if Path::new(candidate).exists() {
                if Command::new(candidate)
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return Some((candidate.to_string(), vec![]));
                }
            }
        }

        // 3. Direct Docker exec with running oxid-gotenberg (or oxidrender-gotenberg) container
        if let Ok(output) = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}"])
            .output()
        {
            let names = String::from_utf8_lossy(&output.stdout);
            if let Some(container) = names
                .lines()
                .map(|l| l.trim())
                .find(|&l| l == "oxid-gotenberg" || l == "oxidrender-gotenberg")
            {
                let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
                let cwd_str = cwd.display().to_string();
                return Some((
                    "docker".to_string(),
                    vec![
                        "exec".to_string(),
                        "-w".to_string(),
                        cwd_str,
                        container.to_string(),
                        "libreoffice".to_string(),
                    ],
                ));
            }
        }

        // 4. Direct Docker run fallback with gotenberg/gotenberg:8
        if let Ok(output) = Command::new("docker")
            .args(["image", "inspect", "gotenberg/gotenberg:8"])
            .output()
        {
            if output.status.success() {
                let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
                let cwd_str = cwd.display().to_string();
                return Some((
                    "docker".to_string(),
                    vec![
                        "run".to_string(),
                        "--rm".to_string(),
                        "-v".to_string(),
                        format!("{}:{}", cwd_str, cwd_str),
                        "-v".to_string(),
                        "/tmp:/tmp".to_string(),
                        "gotenberg/gotenberg:8".to_string(),
                        "libreoffice".to_string(),
                    ],
                ));
            }
        }

        None
    }

    fn convert_via_soffice(input_path: &Path, output_path: &Path) -> Result<()> {
        let (runner_bin, runner_args) = Self::get_soffice_runner()
            .context("No LibreOffice, soffice wrapper or Docker container available")?;

        let abs_input = input_path
            .canonicalize()
            .unwrap_or_else(|_| input_path.to_path_buf());

        let temp_dir = tempfile::tempdir()?;
        let abs_temp = temp_dir
            .path()
            .canonicalize()
            .unwrap_or_else(|_| temp_dir.path().to_path_buf());

        let mut cmd = Command::new(&runner_bin);
        for arg in &runner_args {
            cmd.arg(arg);
        }

        let status = cmd
            .arg("--headless")
            .arg("--convert-to")
            .arg("pdf")
            .arg("--outdir")
            .arg(&abs_temp)
            .arg(&abs_input)
            .status()
            .context("Failed to spawn soffice/LibreOffice process")?;

        if !status.success() {
            bail!("soffice conversion exited with failure: {}", status);
        }

        // Locate the generated pdf in temp_dir
        for entry in fs::read_dir(&abs_temp)?.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("pdf") {
                fs::copy(entry.path(), output_path)?;
                return Ok(());
            }
        }

        bail!("soffice did not produce an output PDF");
    }

    pub fn convert_docx_native(input_path: &Path, output_path: &Path) -> Result<()> {
        let file = File::open(input_path).context("Failed to open docx file")?;
        let mut archive = ZipArchive::new(file).context("Failed to read docx as ZIP archive")?;

        let rel_map = Self::extract_docx_image_rels(&mut archive);

        let mut xml_content = String::new();
        {
            let document_xml = archive
                .by_name("word/document.xml")
                .context("word/document.xml not found inside docx")?;
            document_xml.take(30 * 1024 * 1024).read_to_string(&mut xml_content)?;
        }

        let blocks = Self::extract_docx_blocks(&xml_content)?;

        // Synthesize PDF
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_helv = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let font_bold = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica-Bold",
        });

        // Preload and convert all referenced images into PDF XObjects
        let mut loaded_images: HashMap<String, PreloadedImage> = HashMap::new();
        for (r_id, zip_path) in &rel_map {
            if let Ok(img_file) = archive.by_name(zip_path) {
                let mut img_bytes = Vec::new();
                if img_file.take(20 * 1024 * 1024).read_to_end(&mut img_bytes).is_ok() {
                    if let Ok(dyn_img) = image::load_from_memory(&img_bytes) {
                        let rgba = dyn_img.to_rgba8();
                        let (img_w, img_h) = rgba.dimensions();
                        if img_w == 0 || img_h == 0 || img_w > 4096 || img_h > 4096 {
                            continue;
                        }
                        let mut rgb_bytes = Vec::with_capacity((img_w * img_h * 3) as usize);
                        for pixel in rgba.pixels() {
                            let alpha = pixel[3] as f32 / 255.0;
                            let r = ((pixel[0] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            let g = ((pixel[1] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            let b = ((pixel[2] as f32 * alpha) + (255.0 * (1.0 - alpha))) as u8;
                            rgb_bytes.push(r);
                            rgb_bytes.push(g);
                            rgb_bytes.push(b);
                        }

                        let mut img_dict = lopdf::Dictionary::new();
                        img_dict.set("Type", Object::Name(b"XObject".to_vec()));
                        img_dict.set("Subtype", Object::Name(b"Image".to_vec()));
                        img_dict.set("Width", Object::Integer(img_w as i64));
                        img_dict.set("Height", Object::Integer(img_h as i64));
                        img_dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
                        img_dict.set("BitsPerComponent", Object::Integer(8));

                        let mut stream = Stream::new(img_dict, rgb_bytes);
                        let _ = stream.compress();
                        let obj_id = doc.add_object(Object::Stream(stream));

                        loaded_images.insert(
                            r_id.clone(),
                            PreloadedImage {
                                obj_id,
                                width_px: img_w,
                                height_px: img_h,
                            },
                        );
                    }
                }
            }
        }

        let mut page_objects = Vec::new();
        let clean = |s: &str| {
            s.replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)")
        };

        let mut current_stream = String::new();
        current_stream.push_str("BT\n");
        let mut y = 780.0;
        let mut page_images: HashMap<String, lopdf::ObjectId> = HashMap::new();

        let flush_page = |doc: &mut Document,
                              page_objects: &mut Vec<Object>,
                              stream_str: &mut String,
                              page_images: &mut HashMap<String, lopdf::ObjectId>,
                              pages_id: lopdf::ObjectId,
                              font_helv: lopdf::ObjectId,
                              font_bold: lopdf::ObjectId| {
            stream_str.push_str("ET\n");
            let content_id = doc.add_object(Stream::new(
                lopdf::Dictionary::new(),
                stream_str.as_bytes().to_vec(),
            ));

            let mut xobject_dict = lopdf::Dictionary::new();
            for (alias, id) in page_images.iter() {
                xobject_dict.set(alias.as_bytes().to_vec(), Object::Reference(*id));
            }

            let resources_id = doc.add_object(dictionary! {
                "Font" => dictionary! {
                    "F1" => font_helv,
                    "F2" => font_bold,
                },
                "XObject" => xobject_dict,
            });

            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Resources" => resources_id,
                "Contents" => content_id,
            });
            page_objects.push(page_id.into());

            stream_str.clear();
            stream_str.push_str("BT\n");
            page_images.clear();
        };

        for block in blocks {
            match block {
                DocxBlock::Paragraph { text, is_heading } => {
                    let font_tag = if is_heading { "/F2 13 Tf" } else { "/F1 10 Tf" };
                    let line_height = if is_heading { 20.0 } else { 14.0 };
                    let lines = Self::wrap_text(&text, if is_heading { 55 } else { 80 });

                    for line in lines {
                        if y < 60.0 {
                            flush_page(
                                &mut doc,
                                &mut page_objects,
                                &mut current_stream,
                                &mut page_images,
                                pages_id,
                                font_helv,
                                font_bold,
                            );
                            y = 780.0;
                        }
                        let safe_line = clean(&line);
                        current_stream.push_str(&format!(
                            "{}\n1 0 0 1 50 {:.2} Tm\n({}) Tj\n",
                            font_tag, y, safe_line
                        ));
                        y -= line_height;
                    }
                    y -= 6.0;
                }
                DocxBlock::Image { rel_id, cx_pt, cy_pt } => {
                    if let Some(img) = loaded_images.get(&rel_id) {
                        let mut w = cx_pt.unwrap_or(img.width_px as f64 * 0.75);
                        let mut h = cy_pt.unwrap_or(img.height_px as f64 * 0.75);

                        let max_w = 495.0;
                        let max_h = 550.0;
                        if w > max_w {
                            let s = max_w / w;
                            w = max_w;
                            h *= s;
                        }
                        if h > max_h {
                            let s = max_h / h;
                            h = max_h;
                            w *= s;
                        }

                        if y - h < 50.0 {
                            flush_page(
                                &mut doc,
                                &mut page_objects,
                                &mut current_stream,
                                &mut page_images,
                                pages_id,
                                font_helv,
                                font_bold,
                            );
                            y = 780.0;
                        }

                        let img_x = 50.0 + (495.0 - w) / 2.0;
                        let img_y = y - h;
                        let img_alias = format!("Im{}", page_images.len() + 1);

                        current_stream.push_str("ET\nq\n");
                        current_stream.push_str(&format!(
                            "{:.2} 0 0 {:.2} {:.2} {:.2} cm\n",
                            w, h, img_x, img_y
                        ));
                        current_stream.push_str(&format!("/{} Do\nQ\nBT\n", img_alias));

                        page_images.insert(img_alias, img.obj_id);
                        y = img_y - 16.0;
                    }
                }
            }
        }

        flush_page(
            &mut doc,
            &mut page_objects,
            &mut current_stream,
            &mut page_images,
            pages_id,
            font_helv,
            font_bold,
        );

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

        info!("Converted DOCX with images to PDF natively at {}", output_path.display());
        Ok(())
    }

    fn extract_docx_image_rels(archive: &mut ZipArchive<File>) -> HashMap<String, String> {
        let mut rel_map = HashMap::new();
        if let Ok(mut rels_file) = archive.by_name("word/_rels/document.xml.rels") {
            let mut xml = String::new();
            if rels_file.read_to_string(&mut xml).is_ok() {
                let mut reader = Reader::from_str(&xml);
                reader.config_mut().trim_text(true);
                while let Ok(event) = reader.read_event() {
                    match event {
                        Event::Empty(ref e) | Event::Start(ref e)
                            if e.name().local_name().as_ref() == b"Relationship" =>
                        {
                            let mut id = None;
                            let mut target = None;
                            let mut is_image = false;
                            for attr in e.attributes().flatten() {
                                let key = attr.key.local_name();
                                if key.as_ref() == b"Id" {
                                    id = Some(String::from_utf8_lossy(&attr.value).to_string());
                                } else if key.as_ref() == b"Target" {
                                    target =
                                        Some(String::from_utf8_lossy(&attr.value).to_string());
                                } else if key.as_ref() == b"Type"
                                    && String::from_utf8_lossy(&attr.value).contains("image")
                                {
                                    is_image = true;
                                }
                            }
                            if is_image {
                                if let (Some(id), Some(target)) = (id, target) {
                                    let path = if target.starts_with("word/") {
                                        target
                                    } else if target.starts_with('/') {
                                        format!("word{}", target)
                                    } else if target.starts_with("../") {
                                        target.replace("../", "")
                                    } else {
                                        format!("word/{}", target)
                                    };
                                    rel_map.insert(id, path);
                                }
                            }
                        }
                        Event::Eof => break,
                        _ => {}
                    }
                }
            }
        }
        rel_map
    }

    fn extract_docx_blocks(xml: &str) -> Result<Vec<DocxBlock>> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut blocks = Vec::new();
        let mut current_p = String::new();
        let mut in_text_node = false;
        let mut last_cx: Option<f64> = None;
        let mut last_cy: Option<f64> = None;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) if e.name().local_name().as_ref() == b"p" => {
                    current_p.clear();
                }
                Ok(Event::End(ref e)) if e.name().local_name().as_ref() == b"p" => {
                    let trimmed = current_p.trim();
                    if !trimmed.is_empty() {
                        let is_heading = trimmed.len() < 50
                            && (trimmed.starts_with("1.")
                                || trimmed.starts_with("2.")
                                || trimmed.starts_with("3.")
                                || trimmed.starts_with("4.")
                                || trimmed.starts_with("5.")
                                || trimmed.starts_with("6.")
                                || trimmed
                                    .chars()
                                    .all(|c| !c.is_alphabetic() || c.is_uppercase()));
                        blocks.push(DocxBlock::Paragraph {
                            text: trimmed.to_string(),
                            is_heading,
                        });
                    }
                    current_p.clear();
                }
                Ok(Event::Start(ref e)) if e.name().local_name().as_ref() == b"t" => {
                    in_text_node = true;
                }
                Ok(Event::End(ref e)) if e.name().local_name().as_ref() == b"t" => {
                    in_text_node = false;
                }
                Ok(Event::Text(ref e)) if in_text_node => {
                    if let Ok(t) = e.unescape() {
                        current_p.push_str(&t);
                    }
                }
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let local = e.name().local_name();
                    if local.as_ref() == b"extent" {
                        for attr in e.attributes().flatten() {
                            let k = attr.key.local_name();
                            if k.as_ref() == b"cx" {
                                if let Ok(val) = std::str::from_utf8(&attr.value)
                                    .unwrap_or("0")
                                    .parse::<f64>()
                                {
                                    last_cx = Some(val / 12700.0);
                                }
                            } else if k.as_ref() == b"cy" {
                                if let Ok(val) = std::str::from_utf8(&attr.value)
                                    .unwrap_or("0")
                                    .parse::<f64>()
                                {
                                    last_cy = Some(val / 12700.0);
                                }
                            }
                        }
                    } else if local.as_ref() == b"blip" {
                        let mut rel_id = None;
                        for attr in e.attributes().flatten() {
                            let k = attr.key.local_name();
                            if k.as_ref() == b"embed" || k.as_ref() == b"link" {
                                rel_id = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                        if let Some(id) = rel_id {
                            let trimmed = current_p.trim();
                            if !trimmed.is_empty() {
                                blocks.push(DocxBlock::Paragraph {
                                    text: trimmed.to_string(),
                                    is_heading: false,
                                });
                                current_p.clear();
                            }
                            blocks.push(DocxBlock::Image {
                                rel_id: id,
                                cx_pt: last_cx.take(),
                                cy_pt: last_cy.take(),
                            });
                        }
                    } else if local.as_ref() == b"imagedata" {
                        let mut rel_id = None;
                        for attr in e.attributes().flatten() {
                            let k = attr.key.local_name();
                            if k.as_ref() == b"id" {
                                rel_id = Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                        if let Some(id) = rel_id {
                            let trimmed = current_p.trim();
                            if !trimmed.is_empty() {
                                blocks.push(DocxBlock::Paragraph {
                                    text: trimmed.to_string(),
                                    is_heading: false,
                                });
                                current_p.clear();
                            }
                            blocks.push(DocxBlock::Image {
                                rel_id: id,
                                cx_pt: last_cx.take(),
                                cy_pt: last_cy.take(),
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => bail!("XML parsing error inside docx: {}", e),
                _ => {}
            }
        }

        Ok(blocks)
    }

    fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current_line = String::new();
        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.len() + 1 + word.len() <= max_chars {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        lines
    }

    pub fn convert_xlsx_native(input_path: &Path, output_path: &Path) -> Result<()> {
        let file = File::open(input_path).context("Failed to open xlsx file")?;
        let mut archive = ZipArchive::new(file).context("Failed to read xlsx as ZIP archive")?;

        // 1. Read shared strings if available
        let mut shared_strings = Vec::new();
        if let Ok(ss_entry) = archive.by_name("xl/sharedStrings.xml") {
            let mut ss_xml = String::new();
            let _ = ss_entry.take(30 * 1024 * 1024).read_to_string(&mut ss_xml);
            let mut reader = Reader::from_str(&ss_xml);
            reader.config_mut().trim_text(true);
            let mut in_t = false;
            while let Ok(event) = reader.read_event() {
                match event {
                    Event::Start(ref e) if e.name().as_ref() == b"t" => in_t = true,
                    Event::End(ref e) if e.name().as_ref() == b"t" => in_t = false,
                    Event::Text(ref e) if in_t => {
                        if let Ok(txt) = e.unescape() {
                            shared_strings.push(txt.to_string());
                        }
                    }
                    Event::Eof => break,
                    _ => {}
                }
            }
        }

        // 2. Read sheet1.xml
        let mut sheet_xml = String::new();
        if let Ok(sheet_entry) = archive.by_name("xl/worksheets/sheet1.xml") {
            let _ = sheet_entry.take(30 * 1024 * 1024).read_to_string(&mut sheet_xml);
        }

        let mut lines = Vec::new();
        lines.push("=== CLASSEUR EXCEL (XLSX) ===".to_string());
        if !shared_strings.is_empty() {
            lines.push("Données extraites des feuilles :".to_string());
            for chunk in shared_strings.chunks(5) {
                lines.push(chunk.join("  |  "));
            }
        } else {
            lines.push("Feuille de calcul (aucune chaîne partagée détectée)".to_string());
        }

        Self::render_lines_to_pdf(&lines, output_path)
    }

    pub fn convert_archive_xml_native(input_path: &Path, output_path: &Path, ext: &str) -> Result<()> {
        let file = File::open(input_path).context("Failed to open archive file")?;
        let mut archive = ZipArchive::new(file).context("Failed to read as ZIP archive")?;

        let mut extracted_text = Vec::new();
        let title = match ext {
            "odg" => "=== DESSIN / SCHEMA VECTORIEL OPENDOCUMENT (ODG) ===",
            "vsdx" => "=== DIAGRAMME MICROSOFT VISIO (VSDX) ===",
            "odt" => "=== DOCUMENT TEXTE OPENDOCUMENT (ODT) ===",
            "ods" => "=== TABLEUR OPENDOCUMENT (ODS) ===",
            "odp" => "=== PRESENTATION OPENDOCUMENT (ODP) ===",
            _ => "=== DOCUMENT ARCHIVE (XML) ===",
        };
        extracted_text.push(title.to_string());

        // For OpenDocument files, content is in content.xml
        let candidate_files = ["content.xml", "visio/pages/page1.xml", "visio/document.xml"];
        for candidate in candidate_files {
            if let Ok(entry) = archive.by_name(candidate) {
                let mut xml_content = String::new();
                if entry.take(30 * 1024 * 1024).read_to_string(&mut xml_content).is_ok() {
                    let mut reader = Reader::from_str(&xml_content);
                    reader.config_mut().trim_text(true);
                    while let Ok(event) = reader.read_event() {
                        match event {
                            Event::Text(ref e) => {
                                if let Ok(txt) = e.unescape() {
                                    let s = txt.trim();
                                    if !s.is_empty() && s.len() > 1 && !s.chars().all(|c| c.is_numeric()) {
                                        extracted_text.push(s.to_string());
                                    }
                                }
                            }
                            Event::Eof => break,
                            _ => {}
                        }
                    }
                }
            }
        }

        if extracted_text.len() <= 1 {
            extracted_text.push(format!("Document diagramme .{} prêt pour affichage vectoriel.", ext.to_uppercase()));
        }

        Self::render_lines_to_pdf(&extracted_text, output_path)
    }

    fn render_lines_to_pdf(lines: &[String], output_path: &Path) -> Result<()> {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_helv = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let font_bold = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica-Bold",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_helv,
                "F2" => font_bold,
            },
        });

        let mut page_objects = Vec::new();
        let clean = |s: &str| {
            crate::security::escape_pdf_str(s)
        };

        let mut current_stream = String::new();
        current_stream.push_str("BT\n");
        let mut y = 780.0;

        for (idx, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                y -= 12.0;
                continue;
            }

            let is_heading = idx == 0;
            let font_tag = if is_heading { "/F2 12 Tf" } else { "/F1 10 Tf" };
            let line_height = if is_heading { 24.0 } else { 16.0 };

            let truncated = crate::security::safe_truncate_str(line, 80);
            let safe_p = clean(truncated);
            current_stream.push_str(&format!("{}\n1 0 0 1 50 {:.2} Tm\n({}) Tj\n", font_tag, y, safe_p));
            y -= line_height;

            if y < 60.0 {
                current_stream.push_str("ET\n");
                let content_id = doc.add_object(Stream::new(
                    lopdf::Dictionary::new(),
                    current_stream.as_bytes().to_vec(),
                ));

                let page_id = doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                    "Resources" => resources_id,
                    "Contents" => content_id,
                });
                page_objects.push(page_id.into());

                current_stream = String::new();
                current_stream.push_str("BT\n");
                y = 780.0;
            }
        }

        current_stream.push_str("ET\n");
        let content_id = doc.add_object(Stream::new(
            lopdf::Dictionary::new(),
            current_stream.as_bytes().to_vec(),
        ));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => resources_id,
            "Contents" => content_id,
        });
        page_objects.push(page_id.into());

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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_office_engine_preference_parsing() {
        std::env::set_var("OXID_OFFICE_ENGINE", "hybrid");
        assert_eq!(OfficeEnginePreference::from_env(), OfficeEnginePreference::Hybrid);

        std::env::set_var("OXID_OFFICE_ENGINE", "office2pdf");
        assert_eq!(OfficeEnginePreference::from_env(), OfficeEnginePreference::Office2Pdf);

        std::env::set_var("OXID_OFFICE_ENGINE", "typst");
        assert_eq!(OfficeEnginePreference::from_env(), OfficeEnginePreference::Office2Pdf);

        std::env::set_var("OXID_OFFICE_ENGINE", "gotenberg");
        assert_eq!(OfficeEnginePreference::from_env(), OfficeEnginePreference::Gotenberg);

        std::env::set_var("OXID_OFFICE_ENGINE", "libreoffice");
        assert_eq!(OfficeEnginePreference::from_env(), OfficeEnginePreference::Gotenberg);
    }

    #[test]
    fn test_office2pdf_runner_discovery() {
        let runner = OfficeConverter::get_office2pdf_runner();
        assert!(runner.is_some(), "Expected office2pdf runner to be found in bin/office2pdf or /tmp");
    }

    #[test]
    fn test_convert_odg_and_vsdx_diagrams() {
        let odg_path = Path::new("../data/documents/cas_architecture.odg");
        let vsdx_path = Path::new("../data/documents/cas_diagramme_visio.vsdx");
        
        let out_odg_pdf = Path::new("/tmp/test_odg_output.pdf");
        let out_vsdx_pdf = Path::new("/tmp/test_vsdx_output.pdf");

        if odg_path.exists() {
            let res = OfficeConverter::convert_to_pdf(odg_path, out_odg_pdf);
            assert!(res.is_ok(), "Conversion ODG to PDF failed: {:?}", res);
            assert!(out_odg_pdf.exists(), "Output ODG PDF not generated");
        }

        if vsdx_path.exists() {
            let res = OfficeConverter::convert_to_pdf(vsdx_path, out_vsdx_pdf);
            assert!(res.is_ok(), "Conversion VSDX to PDF failed: {:?}", res);
            assert!(out_vsdx_pdf.exists(), "Output VSDX PDF not generated");
        }
    }

    #[test]
    fn test_convert_xlsx_and_ods_spreadsheets() {
        let xlsx_path = Path::new("../data/documents/cas_budget_cloud.xlsx");
        let ods_path = Path::new("../data/documents/cas_inventaire_infra.ods");
        
        let out_xlsx_pdf = Path::new("/tmp/test_xlsx_output.pdf");
        let out_ods_pdf = Path::new("/tmp/test_ods_output.pdf");

        if xlsx_path.exists() {
            let res = OfficeConverter::convert_to_pdf(xlsx_path, out_xlsx_pdf);
            assert!(res.is_ok(), "Conversion XLSX to PDF failed: {:?}", res);
            assert!(out_xlsx_pdf.exists(), "Output XLSX PDF not generated");
        }

        if ods_path.exists() {
            let res = OfficeConverter::convert_to_pdf(ods_path, out_ods_pdf);
            assert!(res.is_ok(), "Conversion ODS to PDF failed: {:?}", res);
            assert!(out_ods_pdf.exists(), "Output ODS PDF not generated");
        }
    }
}



