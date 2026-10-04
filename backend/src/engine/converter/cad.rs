use crate::models::{CadLayerInfo, CadMetadata};
use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, Stream};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tracing::info;

#[derive(Clone, Debug)]
pub struct CadLayer {
    pub name: String,
    pub color_aci: i32,
    pub color_hex: String,
    pub is_visible: bool,
    pub entity_count: usize,
}

#[derive(Clone, Debug)]
pub enum CadEntity {
    Line {
        layer: String,
        color_hex: String,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Circle {
        layer: String,
        color_hex: String,
        cx: f64,
        cy: f64,
        radius: f64,
    },
    Arc {
        layer: String,
        color_hex: String,
        cx: f64,
        cy: f64,
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    Polyline {
        layer: String,
        color_hex: String,
        points: Vec<(f64, f64)>,
        is_closed: bool,
    },
    Text {
        layer: String,
        color_hex: String,
        x: f64,
        y: f64,
        height: f64,
        text: String,
    },
    Solid {
        layer: String,
        color_hex: String,
        points: Vec<(f64, f64)>,
    },
}

#[derive(Clone, Debug)]
pub struct CadDrawing {
    pub layers: HashMap<String, CadLayer>,
    pub entities: Vec<CadEntity>,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

pub struct CadConverter;

impl CadConverter {
    pub fn aci_to_hex(aci: i32) -> String {
        let abs_aci = aci.abs();
        match abs_aci {
            1 => "#FF0000".to_string(), // Red
            2 => "#FFFF00".to_string(), // Yellow
            3 => "#00FF00".to_string(), // Green
            4 => "#00FFFF".to_string(), // Cyan
            5 => "#0066FF".to_string(), // Blue
            6 => "#FF00FF".to_string(), // Magenta
            7 => "#FFFFFF".to_string(), // White / Black
            8 => "#808080".to_string(), // Dark Gray
            9 => "#C0C0C0".to_string(), // Light Gray
            10..=19 => "#FF3333".to_string(),
            20..=39 => "#FFA500".to_string(), // Orange
            40..=59 => "#FFD700".to_string(),
            60..=79 => "#7CFC00".to_string(),
            80..=99 => "#32CD32".to_string(),
            100..=119 => "#00CED1".to_string(),
            120..=139 => "#1E90FF".to_string(),
            140..=159 => "#4169E1".to_string(),
            160..=179 => "#8A2BE2".to_string(),
            180..=199 => "#9400D3".to_string(),
            200..=219 => "#FF1493".to_string(),
            220..=239 => "#DC143C".to_string(),
            240..=249 => "#A52A2A".to_string(),
            250..=255 => "#505050".to_string(),
            _ => "#0066CC".to_string(),
        }
    }

    pub fn parse_dxf(content: &str) -> Result<CadDrawing> {
        let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();
        let mut layers = HashMap::new();
        let mut entities = Vec::new();

        let mut i = 0;
        let len = lines.len();

        let mut in_tables = false;
        let mut in_layer_table = false;
        let mut in_entities = false;

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        // Default Layer "0"
        layers.insert(
            "0".to_string(),
            CadLayer {
                name: "0".to_string(),
                color_aci: 7,
                color_hex: "#333333".to_string(),
                is_visible: true,
                entity_count: 0,
            },
        );

        while i + 1 < len {
            let code_str = lines[i];
            let val = lines[i + 1];
            i += 2;

            let code: i32 = match code_str.parse() {
                Ok(c) => c,
                Err(_) => continue,
            };

            if code == 0 {
                match val {
                    "SECTION" => {
                        // Check section name
                        if i + 1 < len && lines[i] == "2" {
                            let sec_name = lines[i + 1];
                            in_tables = sec_name == "TABLES";
                            in_entities = sec_name == "ENTITIES";
                            i += 2;
                        }
                    }
                    "ENDSEC" => {
                        in_tables = false;
                        in_layer_table = false;
                        in_entities = false;
                    }
                    "TABLE" => {
                        if in_tables && i + 1 < len && lines[i] == "2" {
                            in_layer_table = lines[i + 1] == "LAYER";
                            i += 2;
                        }
                    }
                    "ENDTAB" => {
                        in_layer_table = false;
                    }
                    "LAYER" if in_layer_table => {
                        // Read LAYER definition
                        let mut name = "Layer".to_string();
                        let mut color_aci = 7;
                        let mut is_visible = true;

                        while i + 1 < len && lines[i] != "0" {
                            let item_code: i32 = lines[i].parse().unwrap_or(-1);
                            let item_val = lines[i + 1];
                            i += 2;

                            match item_code {
                                2 => name = item_val.to_string(),
                                62 => {
                                    if let Ok(c) = item_val.parse::<i32>() {
                                        color_aci = c;
                                        if c < 0 {
                                            is_visible = false;
                                        }
                                    }
                                }
                                70 => {
                                    if let Ok(f) = item_val.parse::<i32>() {
                                        if (f & 1) != 0 {
                                            // Frozen
                                            is_visible = false;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        let color_hex = Self::aci_to_hex(color_aci);
                        layers.insert(
                            name.clone(),
                            CadLayer {
                                name,
                                color_aci,
                                color_hex,
                                is_visible,
                                entity_count: 0,
                            },
                        );
                    }
                    entity_type if in_entities => {
                        // Read an entity
                        let mut layer = "0".to_string();
                        let mut entity_color_hex = None;
                        let mut x1 = 0.0;
                        let mut y1 = 0.0;
                        let mut x2 = 0.0;
                        let mut y2 = 0.0;
                        let mut radius = 0.0;
                        let mut start_ang = 0.0;
                        let mut end_ang = 360.0;
                        let mut height = 2.5;
                        let mut text = String::new();
                        let mut poly_pts = Vec::new();
                        let mut is_closed = false;
                        let mut curr_vx = 0.0;

                        while i + 1 < len && lines[i] != "0" {
                            let item_code: i32 = lines[i].parse().unwrap_or(-1);
                            let item_val = lines[i + 1];
                            i += 2;

                            match item_code {
                                8 => layer = item_val.to_string(),
                                62 => {
                                    if let Ok(c) = item_val.parse::<i32>() {
                                        entity_color_hex = Some(Self::aci_to_hex(c));
                                    }
                                }
                                10 => {
                                    if let Ok(x) = item_val.parse::<f64>() {
                                        x1 = x;
                                        curr_vx = x;
                                    }
                                }
                                20 => {
                                    if let Ok(y) = item_val.parse::<f64>() {
                                        y1 = y;
                                        poly_pts.push((curr_vx, y));
                                    }
                                }
                                11 => {
                                    if let Ok(x) = item_val.parse::<f64>() {
                                        x2 = x;
                                    }
                                }
                                21 => {
                                    if let Ok(y) = item_val.parse::<f64>() {
                                        y2 = y;
                                    }
                                }
                                40 => {
                                    if let Ok(r) = item_val.parse::<f64>() {
                                        radius = r;
                                        height = r;
                                    }
                                }
                                50 => {
                                    if let Ok(a) = item_val.parse::<f64>() {
                                        start_ang = a;
                                    }
                                }
                                51 => {
                                    if let Ok(a) = item_val.parse::<f64>() {
                                        end_ang = a;
                                    }
                                }
                                1 => text = item_val.to_string(),
                                70 => {
                                    if let Ok(flag) = item_val.parse::<i32>() {
                                        is_closed = (flag & 1) != 0;
                                    }
                                }
                                _ => {}
                            }
                        }

                        // Layer color lookup
                        let layer_color = layers
                            .get(&layer)
                            .map(|l| l.color_hex.clone())
                            .unwrap_or_else(|| "#333333".to_string());
                        let color_hex = entity_color_hex.unwrap_or(layer_color);

                        // Update entity count
                        if let Some(l) = layers.get_mut(&layer) {
                            l.entity_count += 1;
                        } else {
                            layers.insert(
                                layer.clone(),
                                CadLayer {
                                    name: layer.clone(),
                                    color_aci: 7,
                                    color_hex: color_hex.clone(),
                                    is_visible: true,
                                    entity_count: 1,
                                },
                            );
                        }

                        // Update bounding box helper
                        let mut track_pt = |x: f64, y: f64| {
                            if x < min_x { min_x = x; }
                            if x > max_x { max_x = x; }
                            if y < min_y { min_y = y; }
                            if y > max_y { max_y = y; }
                        };

                        match entity_type {
                            "LINE" => {
                                track_pt(x1, y1);
                                track_pt(x2, y2);
                                entities.push(CadEntity::Line {
                                    layer,
                                    color_hex,
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                });
                            }
                            "CIRCLE" => {
                                track_pt(x1 - radius, y1 - radius);
                                track_pt(x1 + radius, y1 + radius);
                                entities.push(CadEntity::Circle {
                                    layer,
                                    color_hex,
                                    cx: x1,
                                    cy: y1,
                                    radius,
                                });
                            }
                            "ARC" => {
                                track_pt(x1 - radius, y1 - radius);
                                track_pt(x1 + radius, y1 + radius);
                                entities.push(CadEntity::Arc {
                                    layer,
                                    color_hex,
                                    cx: x1,
                                    cy: y1,
                                    radius,
                                    start_angle_deg: start_ang,
                                    end_angle_deg: end_ang,
                                });
                            }
                            "LWPOLYLINE" | "POLYLINE" => {
                                for pt in &poly_pts {
                                    track_pt(pt.0, pt.1);
                                }
                                entities.push(CadEntity::Polyline {
                                    layer,
                                    color_hex,
                                    points: poly_pts,
                                    is_closed,
                                });
                            }
                            "TEXT" | "MTEXT" => {
                                track_pt(x1, y1);
                                track_pt(x1 + (text.len() as f64 * height * 0.6), y1 + height);
                                entities.push(CadEntity::Text {
                                    layer,
                                    color_hex,
                                    x: x1,
                                    y: y1,
                                    height,
                                    text,
                                });
                            }
                            "SOLID" | "TRACE" => {
                                track_pt(x1, y1);
                                track_pt(x2, y2);
                                entities.push(CadEntity::Solid {
                                    layer,
                                    color_hex,
                                    points: vec![(x1, y1), (x2, y2)],
                                });
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle empty or zero bounds
        if min_x >= max_x || min_y >= max_y {
            min_x = 0.0;
            min_y = 0.0;
            max_x = 800.0;
            max_y = 600.0;
        }

        Ok(CadDrawing {
            layers,
            entities,
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    pub fn extract_layers(path: &Path) -> Result<Vec<CadLayerInfo>> {
        let content = fs::read_to_string(path)
            .or_else(|_| fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).to_string()))
            .context("Failed to read CAD file")?;

        let drawing = Self::parse_dxf(&content)?;
        let mut list: Vec<CadLayerInfo> = drawing
            .layers
            .into_values()
            .map(|l| CadLayerInfo {
                name: l.name,
                color_hex: l.color_hex,
                is_visible: l.is_visible,
                entity_count: l.entity_count,
            })
            .collect();

        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    pub fn generate_svg(drawing: &CadDrawing, active_layers: Option<&HashSet<String>>) -> String {
        let margin = 40.0;
        let w = (drawing.max_x - drawing.min_x).abs() + margin * 2.0;
        let h = (drawing.max_y - drawing.min_y).abs() + margin * 2.0;

        let scale_w = 1200.0 / w.max(1.0);
        let scale_h = 900.0 / h.max(1.0);
        let scale = scale_w.min(scale_h).min(10.0).max(0.01);

        let canvas_w = (w * scale).round().max(400.0);
        let canvas_h = (h * scale).round().max(300.0);

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}" style="background: #1e1e1e; font-family: monospace;">
<style>
  .cad-line {{ stroke-linecap: round; stroke-linejoin: round; }}
  .cad-text {{ fill: #ffffff; font-size: 11px; }}
</style>
"#,
            canvas_w, canvas_h, canvas_w, canvas_h
        );

        // Group entities by layer
        let mut layer_groups: HashMap<&str, Vec<&CadEntity>> = HashMap::new();
        for entity in &drawing.entities {
            let layer_name = match entity {
                CadEntity::Line { layer, .. } => layer.as_str(),
                CadEntity::Circle { layer, .. } => layer.as_str(),
                CadEntity::Arc { layer, .. } => layer.as_str(),
                CadEntity::Polyline { layer, .. } => layer.as_str(),
                CadEntity::Text { layer, .. } => layer.as_str(),
                CadEntity::Solid { layer, .. } => layer.as_str(),
            };

            if let Some(active) = active_layers {
                if !active.contains(layer_name) {
                    continue;
                }
            }

            layer_groups.entry(layer_name).or_default().push(entity);
        }

        // Coordinate transformation: CAD has Y pointing UP, SVG has Y pointing DOWN
        let tx = |x: f64| -> f64 { (x - drawing.min_x + margin) * scale };
        let ty = |y: f64| -> f64 { canvas_h - ((y - drawing.min_y + margin) * scale) };

        for (layer_name, entities) in layer_groups {
            let layer_info = drawing.layers.get(layer_name);
            let layer_color = layer_info.map(|l| l.color_hex.as_str()).unwrap_or("#00CCFF");
            let is_visible = layer_info.map(|l| l.is_visible).unwrap_or(true);
            let display_attr = if is_visible { "" } else { " style=\"display:none;\"" };

            svg.push_str(&format!(
                "  <g id=\"cad-layer-{}\" data-layer=\"{}\"{}>\n",
                layer_name.replace(' ', "_"),
                layer_name,
                display_attr
            ));

            for entity in entities {
                match entity {
                    CadEntity::Line { x1, y1, x2, y2, color_hex, .. } => {
                        let stroke = if color_hex == "#FFFFFF" || color_hex == "#000000" { layer_color } else { color_hex };
                        svg.push_str(&format!(
                            "    <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"1.5\" class=\"cad-line\"/>\n",
                            tx(*x1), ty(*y1), tx(*x2), ty(*y2), stroke
                        ));
                    }
                    CadEntity::Circle { cx, cy, radius, color_hex, .. } => {
                        let stroke = if color_hex == "#FFFFFF" || color_hex == "#000000" { layer_color } else { color_hex };
                        svg.push_str(&format!(
                            "    <circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" stroke=\"{}\" stroke-width=\"1.5\" fill=\"none\" class=\"cad-line\"/>\n",
                            tx(*cx), ty(*cy), radius * scale, stroke
                        ));
                    }
                    CadEntity::Arc { cx, cy, radius, start_angle_deg, end_angle_deg, color_hex, .. } => {
                        let stroke = if color_hex == "#FFFFFF" || color_hex == "#000000" { layer_color } else { color_hex };
                        let r = radius * scale;
                        let rad1 = start_angle_deg.to_radians();
                        let rad2 = end_angle_deg.to_radians();
                        let ax1 = tx(*cx) + r * rad1.cos();
                        let ay1 = ty(*cy) - r * rad1.sin();
                        let ax2 = tx(*cx) + r * rad2.cos();
                        let ay2 = ty(*cy) - r * rad2.sin();
                        let large_arc = if (end_angle_deg - start_angle_deg).abs() > 180.0 { 1 } else { 0 };

                        svg.push_str(&format!(
                            "    <path d=\"M {:.2} {:.2} A {:.2} {:.2} 0 {} 0 {:.2} {:.2}\" stroke=\"{}\" stroke-width=\"1.5\" fill=\"none\" class=\"cad-line\"/>\n",
                            ax1, ay1, r, r, large_arc, ax2, ay2, stroke
                        ));
                    }
                    CadEntity::Polyline { points, is_closed, color_hex, .. } => {
                        if points.len() >= 2 {
                            let stroke = if color_hex == "#FFFFFF" || color_hex == "#000000" { layer_color } else { color_hex };
                            let mut d = format!("M {:.2} {:.2}", tx(points[0].0), ty(points[0].1));
                            for pt in &points[1..] {
                                d.push_str(&format!(" L {:.2} {:.2}", tx(pt.0), ty(pt.1)));
                            }
                            if *is_closed {
                                d.push_str(" Z");
                            }
                            svg.push_str(&format!(
                                "    <path d=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" fill=\"none\" class=\"cad-line\"/>\n",
                                d, stroke
                            ));
                        }
                    }
                    CadEntity::Text { x, y, text, height, .. } => {
                        let font_sz = (height * scale).clamp(9.0, 24.0);
                        let escaped = text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                        svg.push_str(&format!(
                            "    <text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.1}px\" class=\"cad-text\">{}</text>\n",
                            tx(*x), ty(*y), font_sz, escaped
                        ));
                    }
                    CadEntity::Solid { points, color_hex, .. } => {
                        if points.len() >= 2 {
                            svg.push_str(&format!(
                                "    <line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                                tx(points[0].0), ty(points[0].1), tx(points[1].0), ty(points[1].1), color_hex
                            ));
                        }
                    }
                }
            }
            svg.push_str("  </g>\n");
        }

        // Stamp and title in bottom corner
        svg.push_str(&format!(
            "  <rect x=\"10\" y=\"{:.2}\" width=\"220\" height=\"35\" fill=\"#2a2a2a\" stroke=\"#444\" rx=\"4\"/>\n",
            canvas_h - 45.0
        ));
        svg.push_str(&format!(
            "  <text x=\"20\" y=\"{:.2}\" fill=\"#00ccff\" font-size=\"12px\" font-weight=\"bold\">PLAN CAO / DAO (AUTOCAD)</text>\n",
            canvas_h - 28.0
        ));
        svg.push_str(&format!(
            "  <text x=\"20\" y=\"{:.2}\" fill=\"#888888\" font-size=\"10px\">Oxid Engine • Calques dynamiques</text>\n",
            canvas_h - 15.0
        ));

        svg.push_str("</svg>");
        svg
    }

    pub fn convert_cad_to_pdf(
        input_path: &Path,
        output_path: &Path,
        active_layers: Option<&HashSet<String>>,
    ) -> Result<CadMetadata> {
        info!("Rendering CAD drawing to vector PDF: {:?}", input_path);
        let content = fs::read_to_string(input_path)
            .or_else(|_| fs::read(input_path).map(|bytes| String::from_utf8_lossy(&bytes).to_string()))
            .context("Failed to read CAD drawing file")?;

        let drawing = Self::parse_dxf(&content)?;

        // Build SVG
        let svg_content = Self::generate_svg(&drawing, active_layers);
        let svg_path = output_path.with_extension("svg");
        let _ = fs::write(&svg_path, &svg_content);

        // Vector PDF generation via lopdf
        let margin = 40.0;
        let w = (drawing.max_x - drawing.min_x).abs() + margin * 2.0;
        let h = (drawing.max_y - drawing.min_y).abs() + margin * 2.0;

        let pdf_w = 842.0; // A4 landscape standard
        let pdf_h = 595.0;

        let scale_w = (pdf_w - 60.0) / w.max(1.0);
        let scale_h = (pdf_h - 60.0) / h.max(1.0);
        let scale = scale_w.min(scale_h).min(10.0).max(0.01);

        let tx = |x: f64| -> f64 { 30.0 + (x - drawing.min_x + margin) * scale };
        let ty = |y: f64| -> f64 { 30.0 + (y - drawing.min_y + margin) * scale };

        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let mut stream_content = String::new();
        // Background dark grid style
        stream_content.push_str("0.12 0.12 0.12 rg\n");
        stream_content.push_str(&format!("0 0 {:.2} {:.2} re f\n", pdf_w, pdf_h));

        // Render entities
        for entity in &drawing.entities {
            let (layer_name, color_hex) = match entity {
                CadEntity::Line { layer, color_hex, .. }
                | CadEntity::Circle { layer, color_hex, .. }
                | CadEntity::Arc { layer, color_hex, .. }
                | CadEntity::Polyline { layer, color_hex, .. }
                | CadEntity::Text { layer, color_hex, .. }
                | CadEntity::Solid { layer, color_hex, .. } => (layer, color_hex),
            };

            if let Some(active) = active_layers {
                if !active.contains(layer_name) {
                    continue;
                }
            }

            // Convert hex color to PDF RGB floats
            let (r, g, b) = Self::hex_to_rgb(color_hex);
            stream_content.push_str(&format!("{:.3} {:.3} {:.3} RG\n", r, g, b));
            stream_content.push_str("1.0 w 1 J 1 j\n");

            match entity {
                CadEntity::Line { x1, y1, x2, y2, .. } => {
                    stream_content.push_str(&format!(
                        "{:.2} {:.2} m {:.2} {:.2} l S\n",
                        tx(*x1), ty(*y1), tx(*x2), ty(*y2)
                    ));
                }
                CadEntity::Circle { cx, cy, radius, .. } => {
                    let r = radius * scale;
                    let c = 0.552284749831 * r;
                    let x = tx(*cx);
                    let y = ty(*cy);
                    stream_content.push_str(&format!(
                        "{:.2} {:.2} m {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                        x, y + r, x + c, y + r, x + r, y + c, x + r, y
                    ));
                    stream_content.push_str(&format!(
                        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                        x + r, y - c, x + c, y - r, x, y - r
                    ));
                    stream_content.push_str(&format!(
                        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                        x - c, y - r, x - r, y - c, x - r, y
                    ));
                    stream_content.push_str(&format!(
                        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c S\n",
                        x - r, y + c, x - c, y + r, x, y + r
                    ));
                }
                CadEntity::Polyline { points, is_closed, .. } => {
                    if points.len() >= 2 {
                        stream_content.push_str(&format!("{:.2} {:.2} m ", tx(points[0].0), ty(points[0].1)));
                        for pt in &points[1..] {
                            stream_content.push_str(&format!("{:.2} {:.2} l ", tx(pt.0), ty(pt.1)));
                        }
                        if *is_closed {
                            stream_content.push_str("h ");
                        }
                        stream_content.push_str("S\n");
                    }
                }
                CadEntity::Text { x, y, text, height, .. } => {
                    let sanitized = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
                    let sz = (height * scale).clamp(8.0, 16.0);
                    stream_content.push_str(&format!(
                        "BT\n/F1 {:.1} Tf\n{:.3} {:.3} {:.3} rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\n",
                        sz, r, g, b, tx(*x), ty(*y), sanitized
                    ));
                }
                _ => {}
            }
        }

        // Stamp in bottom-right corner
        stream_content.push_str("0.18 0.18 0.18 rg\n");
        stream_content.push_str(&format!("{:.2} 15 220 32 re f\n", pdf_w - 235.0));
        stream_content.push_str("0.4 0.4 0.4 RG 1 w\n");
        stream_content.push_str(&format!("{:.2} 15 220 32 re S\n", pdf_w - 235.0));
        stream_content.push_str(&format!(
            "BT\n/F1 10 Tf\n0 0.8 1 rg\n1 0 0 1 {:.2} 32 Tm\n(PLAN CAO / DAO - AUTOCAD) Tj\n/F1 8 Tf\n0.7 0.7 0.7 rg\n1 0 0 1 {:.2} 20 Tm\n(Oxid Engine - Calques dynamiques) Tj\nET\n",
            pdf_w - 225.0, pdf_w - 225.0
        ));

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

        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.set_object(pages_id, Object::Dictionary(pages_dict));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc.save(output_path).context("Failed to save CAD PDF rendition")?;

        let mut layer_infos: Vec<CadLayerInfo> = drawing
            .layers
            .into_values()
            .map(|l| CadLayerInfo {
                name: l.name,
                color_hex: l.color_hex,
                is_visible: l.is_visible,
                entity_count: l.entity_count,
            })
            .collect();
        layer_infos.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(CadMetadata {
            layers: layer_infos,
            width: pdf_w,
            height: pdf_h,
            units: "mm".to_string(),
        })
    }

    fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
        let clean = hex.trim_start_matches('#');
        if clean.len() == 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0) as f64 / 255.0;
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(204) as f64 / 255.0;
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255) as f64 / 255.0;
            (r, g, b)
        } else {
            (0.0, 0.8, 1.0)
        }
    }
}
