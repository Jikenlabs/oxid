use anyhow::{Context, Result};
use lopdf::content::Content;
use lopdf::{dictionary, Document, Object, Stream};
use std::fs;
use std::path::Path;

pub struct TextConverter;

impl TextConverter {
    pub fn convert_text_to_pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        let text = fs::read_to_string(input_path)
            .context("Failed to read text file for PDF conversion")?;

        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        // Split text into pages of ~50 lines each
        let lines: Vec<&str> = text.lines().collect();
        let lines_per_page = 48;
        let mut page_objects = Vec::new();

        let chunks: Vec<&[&str]> = lines.chunks(lines_per_page).collect();
        let total_pages = chunks.len().max(1);

        for (page_idx, chunk) in chunks.iter().enumerate() {
            let mut stream_content = String::new();
            stream_content.push_str("BT\n/F1 10 Tf\n");

            let mut y = 800.0;
            for line in *chunk {
                let truncated = crate::security::safe_truncate_str(line, 95);
                let sanitized = crate::security::escape_pdf_str(truncated);

                stream_content.push_str(&format!("1 0 0 1 50 {:.2} Tm\n", y));
                stream_content.push_str(&format!("({}) Tj\n", sanitized));
                y -= 15.0;
            }

            // Footer with page number
            let footer = format!("Page {} / {}", page_idx + 1, total_pages);
            stream_content.push_str(&format!("1 0 0 1 270 30 Tm\n({}) Tj\n", footer));
            stream_content.push_str("ET\n");


            let content_id = doc.add_object(Stream::new(
                lopdf::Dictionary::new(),
                stream_content.as_bytes().to_vec(),
            ));

            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Resources" => resources_id,
                "Contents" => content_id,
            });

            page_objects.push(page_id.into());
        }

        if page_objects.is_empty() {
            // Empty page fallback
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                "Resources" => resources_id,
            });
            page_objects.push(page_id.into());
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

        Ok(())
    }

    pub fn convert_markdown_to_pdf(input_path: &Path, output_path: &Path) -> Result<()> {
        if let Ok(()) = Self::convert_markdown_via_chromium(input_path, output_path) {
            return Ok(());
        }
        Self::convert_text_to_pdf(input_path, output_path)
    }

    fn convert_markdown_via_chromium(input_path: &Path, output_path: &Path) -> Result<()> {
        use std::process::Command;

        let abs_input = input_path
            .canonicalize()
            .unwrap_or_else(|_| input_path.to_path_buf());

        let template_path = Path::new("data/documents/template_mermaid.html");
        let mermaid_js_path = Path::new("data/documents/mermaid.min.js");

        if !template_path.exists() || !mermaid_js_path.exists() {
            anyhow::bail!("Mermaid template or JS missing");
        }

        let abs_template = template_path.canonicalize().unwrap_or_else(|_| template_path.to_path_buf());
        let abs_mermaid = mermaid_js_path.canonicalize().unwrap_or_else(|_| mermaid_js_path.to_path_buf());

        let parent_dir = abs_input.parent().unwrap_or_else(|| Path::new("."));

        let container_name = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}"])
            .output()
            .ok()
            .and_then(|out| {
                let s = String::from_utf8_lossy(&out.stdout);
                s.lines()
                    .map(|l| l.trim().to_string())
                    .find(|l| l == "oxid-gotenberg" || l == "oxidrender-gotenberg")
            })
            .unwrap_or_else(|| "oxid-gotenberg".to_string());

        let output = Command::new("docker")
            .args(&[
                "exec",
                "-w",
                parent_dir.to_str().unwrap_or("."),
                &container_name,
                "curl",
                "-s",
                "http://localhost:3000/forms/chromium/convert/markdown",
                "-F",
                &format!("files=@{};filename=index.html", abs_template.to_string_lossy()),
                "-F",
                &format!("files=@{};filename=index.md", abs_input.to_string_lossy()),
                "-F",
                &format!("files=@{};filename=mermaid.min.js", abs_mermaid.to_string_lossy()),
                "-F",
                "waitForExpression=window.mermaidDone === true",
            ])
            .output()?;

        if output.status.success() && output.stdout.starts_with(b"%PDF") {
            fs::write(output_path, output.stdout)?;
            return Ok(());
        }

        anyhow::bail!("Gotenberg Chromium markdown conversion failed");
    }
}

