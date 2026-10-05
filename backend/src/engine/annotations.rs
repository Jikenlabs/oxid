use crate::models::{Annotation, AnnotationType};

pub struct AnnotationEngine;

impl AnnotationEngine {
    pub fn to_xfdf(annotations: &[Annotation]) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n");
        xml.push_str("  <annots>\n");

        for ann in annotations {
            let page_idx = ann.page_number.saturating_sub(1);
            let rect = format!(
                "{:.2},{:.2},{:.2},{:.2}",
                ann.x,
                ann.y,
                ann.x + ann.width,
                ann.y + ann.height
            );

            let safe_color = crate::security::escape_xml_str(&ann.color);
            let safe_author = crate::security::escape_xml_str(&ann.author);
            let safe_date = crate::security::escape_xml_str(&ann.created_at);

            match ann.annotation_type {
                AnnotationType::Highlight => {
                    xml.push_str(&format!(
                        "    <highlight page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\">\n",
                        page_idx, rect, safe_color, safe_author, safe_date
                    ));
                    if let Some(ref text) = ann.content {
                        xml.push_str(&format!("      <contents>{}</contents>\n", crate::security::escape_xml_str(text)));
                    }
                    xml.push_str("    </highlight>\n");
                }
                AnnotationType::Note => {
                    xml.push_str(&format!(
                        "    <text page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\">\n",
                        page_idx, rect, safe_color, safe_author, safe_date
                    ));
                    if let Some(ref text) = ann.content {
                        xml.push_str(&format!("      <contents>{}</contents>\n", crate::security::escape_xml_str(text)));
                    }
                    xml.push_str("    </text>\n");
                }
                AnnotationType::Redact => {
                    let reason_attr = ann
                        .reason
                        .as_deref()
                        .map(|r| format!(" reason=\"{}\"", crate::security::escape_xml_str(r)))
                        .unwrap_or_default();
                    xml.push_str(&format!(
                        "    <redact page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\"{}>\n",
                        page_idx, rect, safe_color, safe_author, safe_date, reason_attr
                    ));
                    if let Some(ref text) = ann.content {
                        xml.push_str(&format!("      <contents>{}</contents>\n", crate::security::escape_xml_str(text)));
                    }
                    xml.push_str("    </redact>\n");
                }
                AnnotationType::Rectangle => {
                    xml.push_str(&format!(
                        "    <square page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\" />\n",
                        page_idx, rect, safe_color, safe_author, safe_date
                    ));
                }
                AnnotationType::Freehand => {
                    xml.push_str(&format!(
                        "    <ink page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\">\n",
                        page_idx, rect, safe_color, safe_author, safe_date
                    ));
                    if !ann.points.is_empty() {
                        xml.push_str("      <inklist>\n        <gesture>");
                        let points_str = ann
                            .points
                            .iter()
                            .map(|(px, py)| format!("{:.1},{:.1}", px, py))
                            .collect::<Vec<_>>()
                            .join(";");
                        xml.push_str(&points_str);
                        xml.push_str("</gesture>\n      </inklist>\n");
                    }
                    xml.push_str("    </ink>\n");
                }
                _ => {
                    // Annotation générique de secours (tampon)
                    xml.push_str(&format!(
                        "    <stamp page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\" />\n",
                        page_idx, rect, safe_color, safe_author, safe_date
                    ));
                }
            }
        }

        xml.push_str("  </annots>\n");
        xml.push_str("</xfdf>\n");
        xml
    }

    pub fn from_xfdf(xml_content: &str) -> Vec<Annotation> {
        let mut list = Vec::new();
        let lines: Vec<&str> = xml_content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("<highlight ")
                || trimmed.starts_with("<text ")
                || trimmed.starts_with("<square ")
                || trimmed.starts_with("<ink ")
                || trimmed.starts_with("<redact ")
                || trimmed.starts_with("<stamp ")
            {
                let tag_header = trimmed;
                let annot_type = if tag_header.starts_with("<highlight") {
                    AnnotationType::Highlight
                } else if tag_header.starts_with("<text") {
                    AnnotationType::Note
                } else if tag_header.starts_with("<square") {
                    AnnotationType::Rectangle
                } else if tag_header.starts_with("<ink") {
                    AnnotationType::Freehand
                } else if tag_header.starts_with("<redact") {
                    AnnotationType::Redact
                } else {
                    AnnotationType::Stamp
                };

                let extract_attr = |attr: &str| -> Option<String> {
                    let pattern = format!("{}=\"", attr);
                    if let Some(start) = tag_header.find(&pattern) {
                        let sub = &tag_header[start + pattern.len()..];
                        if let Some(end) = sub.find('"') {
                            return Some(sub[..end].to_string());
                        }
                    }
                    None
                };

                let page_idx: usize = extract_attr("page").and_then(|p| p.parse().ok()).unwrap_or(0);
                let color = extract_attr("color").unwrap_or_else(|| match annot_type {
                    AnnotationType::Redact => "#000000".to_string(),
                    AnnotationType::Note => "#f59e0b".to_string(),
                    _ => "#fef08a".to_string(),
                });
                let author = extract_attr("title").unwrap_or_else(|| "Utilisateur".to_string());
                let date = extract_attr("date").unwrap_or_default();
                let reason = extract_attr("reason");

                let mut x = 50.0;
                let mut y = 50.0;
                let mut width = 100.0;
                let mut height = 30.0;

                if let Some(rect_str) = extract_attr("rect") {
                    let parts: Vec<f64> = rect_str
                        .split(',')
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    if parts.len() == 4 {
                        x = parts[0];
                        y = parts[1];
                        width = (parts[2] - parts[0]).abs();
                        height = (parts[3] - parts[1]).abs();
                    }
                }

                // Recherche anticipée de la balise <contents> et de la fermeture si la balise n'est pas auto-fermante
                let mut content: Option<String> = None;
                if !tag_header.ends_with("/>") {
                    let mut j = i + 1;
                    while j < lines.len() {
                        let inner_line = lines[j].trim();
                        if inner_line.starts_with("<contents>") && inner_line.ends_with("</contents>") {
                            content = Some(inner_line[10..inner_line.len() - 11].to_string());
                        } else if inner_line.starts_with("</") {
                            i = j;
                            break;
                        }
                        j += 1;
                    }
                }

                let opacity = match annot_type {
                    AnnotationType::Redact => 1.0,
                    AnnotationType::Note => 1.0,
                    _ => 0.45,
                };

                list.push(Annotation {
                    id: uuid::Uuid::new_v4().to_string(),
                    page_number: page_idx + 1,
                    annotation_type: annot_type,
                    x,
                    y,
                    width,
                    height,
                    color,
                    opacity,
                    author,
                    created_at: date,
                    content,
                    points: vec![],
                    reason,
                });
            }
            i += 1;
        }
        list
    }
}
