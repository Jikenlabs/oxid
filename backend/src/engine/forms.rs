use crate::models::{FormField, FormFieldType, FormFieldsSummary};
use anyhow::{Context, Result};
use lopdf::{Dictionary, Document, Object};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

pub struct FormEngine;

impl FormEngine {
    /// Extract all AcroForm fields across pages in the given PDF document
    pub fn extract_form_fields(path: &Path) -> Result<FormFieldsSummary> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(path)?;
        let doc = Document::load(&rendition.effective_path)
            .with_context(|| format!("Failed to load PDF for forms extraction: {:?}", path))?;

        let pages = doc.get_pages();
        let mut fields: Vec<FormField> = Vec::new();

        for (page_num, &page_id) in &pages {
            let page_obj = match doc.get_object(page_id) {
                Ok(o) => o,
                Err(_) => continue,
            };
            let page_dict = match page_obj.as_dict() {
                Ok(d) => d,
                Err(_) => continue,
            };

            // Calculate page height for coordinate conversion
            let page_height = Self::get_page_height(page_dict);

            // Look for Annots array
            let annots_opt = page_dict.get(b"Annots").ok();
            let annots_arr = match annots_opt {
                Some(Object::Array(arr)) => arr.clone(),
                Some(Object::Reference(id)) => {
                    if let Ok(Object::Array(arr)) = doc.get_object(*id) {
                        arr.clone()
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };

            for annot_ref in annots_arr {
                let annot_dict = match annot_ref {
                    Object::Reference(id) => match doc.get_object(id) {
                        Ok(Object::Dictionary(d)) => d.clone(),
                        _ => continue,
                    },
                    Object::Dictionary(d) => d,
                    _ => continue,
                };

                // Check if Subtype is Widget
                let subtype = annot_dict
                    .get(b"Subtype")
                    .and_then(|o| o.as_name())
                    .unwrap_or(b"");

                if subtype != b"Widget" {
                    continue;
                }

                if let Some(form_field) = Self::parse_widget(&doc, &annot_dict, *page_num as usize, page_height) {
                    fields.push(form_field);
                }
            }
        }

        let fields_count = fields.len();
        let has_forms = fields_count > 0;

        info!(
            "Extracted {} form field(s) from document {:?}",
            fields_count, path
        );

        Ok(FormFieldsSummary {
            has_forms,
            fields_count,
            fields,
        })
    }

    /// Fill form fields in the PDF document and write to output_path
    pub fn fill_form_fields(
        input_path: &Path,
        output_path: &Path,
        values: &HashMap<String, serde_json::Value>,
    ) -> Result<usize> {
        let rendition = crate::engine::converter::FormatConverter::ensure_pdf_rendition(input_path)?;
        let mut doc = Document::load(&rendition.effective_path)
            .with_context(|| format!("Failed to load PDF for forms fill: {:?}", input_path))?;

        // 1. Enable /NeedAppearances true in Catalog /AcroForm so PDF viewers generate visuals
        Self::ensure_need_appearances(&mut doc);

        let pages = doc.get_pages();
        let mut updated_count = 0;

        // Traverse all pages and widgets
        for (_page_num, &page_id) in &pages {
            let page_dict = match doc.get_object(page_id).and_then(|o| o.as_dict()) {
                Ok(d) => d.clone(),
                Err(_) => continue,
            };

            let annots_opt = page_dict.get(b"Annots").ok();
            let annots_arr = match annots_opt {
                Some(Object::Array(arr)) => arr.clone(),
                Some(Object::Reference(id)) => {
                    if let Ok(Object::Array(arr)) = doc.get_object(*id) {
                        arr.clone()
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };

            for annot_ref in annots_arr {
                let widget_id = match annot_ref {
                    Object::Reference(id) => id,
                    _ => continue,
                };

                let (field_name, field_type) = {
                    let widget_dict = match doc.get_object(widget_id).and_then(|o| o.as_dict()) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let name = Self::resolve_field_name(&doc, widget_dict);
                    let ftype = Self::resolve_field_type(&doc, widget_dict);
                    (name, ftype)
                };

                if let Some(name) = field_name {
                    if let Some(new_val) = values.get(&name) {
                        if let Ok(widget_dict_mut) = doc.get_object_mut(widget_id).and_then(|o| o.as_dict_mut()) {
                            match field_type {
                                FormFieldType::Checkbox => {
                                    let is_checked = match new_val {
                                        serde_json::Value::Bool(b) => *b,
                                        serde_json::Value::String(s) => {
                                            s.eq_ignore_ascii_case("true")
                                                || s.eq_ignore_ascii_case("yes")
                                                || s == "1"
                                                || s.eq_ignore_ascii_case("on")
                                        }
                                        serde_json::Value::Number(n) => n.as_i64() == Some(1),
                                        _ => false,
                                    };
                                    let val_name = if is_checked { b"Yes".to_vec() } else { b"Off".to_vec() };
                                    widget_dict_mut.set("V", Object::Name(val_name.clone()));
                                    widget_dict_mut.set("AS", Object::Name(val_name));
                                }
                                FormFieldType::Radio => {
                                    let str_val = match new_val {
                                        serde_json::Value::String(s) => s.as_bytes().to_vec(),
                                        _ => new_val.to_string().into_bytes(),
                                    };
                                    widget_dict_mut.set("V", Object::Name(str_val.clone()));
                                    widget_dict_mut.set("AS", Object::Name(str_val));
                                }
                                _ => {
                                    let str_val = match new_val {
                                        serde_json::Value::String(s) => s.clone(),
                                        _ => new_val.to_string(),
                                    };
                                    widget_dict_mut.set("V", Object::string_literal(str_val));
                                }
                            }
                            updated_count += 1;
                        }
                    }
                }
            }
        }

        // Save output document
        doc.save(output_path)
            .with_context(|| format!("Failed to save filled PDF to {:?}", output_path))?;

        info!(
            "Form fill completed: updated {} field(s), saved to {:?}",
            updated_count, output_path
        );

        Ok(updated_count)
    }

    fn ensure_need_appearances(doc: &mut Document) {
        let root_id = match doc.trailer.get(b"Root").and_then(|r| r.as_reference()) {
            Ok(id) => id,
            Err(_) => return,
        };

        let acro_ref = if let Ok(root_dict) = doc.get_object(root_id).and_then(|o| o.as_dict()) {
            match root_dict.get(b"AcroForm") {
                Ok(Object::Reference(id)) => Some(*id),
                _ => None,
            }
        } else {
            None
        };

        if let Some(acro_id) = acro_ref {
            if let Ok(acro_dict) = doc.get_object_mut(acro_id).and_then(|o| o.as_dict_mut()) {
                acro_dict.set("NeedAppearances", Object::Boolean(true));
            }
        } else if let Ok(root_dict_mut) = doc.get_object_mut(root_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(acro_dict_mut) = root_dict_mut.get_mut(b"AcroForm").and_then(|o| o.as_dict_mut()) {
                acro_dict_mut.set("NeedAppearances", Object::Boolean(true));
            } else {
                let mut acro = Dictionary::new();
                acro.set("NeedAppearances", Object::Boolean(true));
                root_dict_mut.set("AcroForm", Object::Dictionary(acro));
            }
        }
    }

    fn parse_widget(
        doc: &Document,
        dict: &Dictionary,
        page_num: usize,
        page_height: f64,
    ) -> Option<FormField> {
        let rect_arr = dict.get(b"Rect").ok()?.as_array().ok()?;
        if rect_arr.len() < 4 {
            return None;
        }

        let to_f64 = |o: &Object| match o {
            Object::Real(f) => Some(*f as f64),
            Object::Integer(i) => Some(*i as f64),
            _ => None,
        };

        let x1 = to_f64(&rect_arr[0])?;
        let y1 = to_f64(&rect_arr[1])?;
        let x2 = to_f64(&rect_arr[2])?;
        let y2 = to_f64(&rect_arr[3])?;

        let pdf_x = x1.min(x2);
        let pdf_y_top = y1.max(y2);
        let width = (x2 - x1).abs();
        let height = (y2 - y1).abs();

        // Convert bottom-left to top-left viewer coordinate space
        let x = (pdf_x * 100.0).round() / 100.0;
        let y = ((page_height - pdf_y_top) * 100.0).round() / 100.0;
        let width = (width * 100.0).round() / 100.0;
        let height = (height * 100.0).round() / 100.0;

        let name = Self::resolve_field_name(doc, dict)
            .unwrap_or_else(|| format!("field_p{}_{}_{}", page_num, x as u32, y as u32));

        let field_type = Self::resolve_field_type(doc, dict);

        // Extract value /V
        let value = Self::resolve_field_value(doc, dict).unwrap_or_default();
        let default_value = dict.get(b"DV").ok().and_then(Self::object_to_string);

        // Flags /Ff
        let flags = Self::resolve_flags(doc, dict);
        let read_only = (flags & 1) != 0;
        let required = (flags & 2) != 0;
        let multiline = (flags & (1 << 12)) != 0;

        // Options /Opt for choice fields
        let options = dict.get(b"Opt").ok().and_then(|opt_obj| {
            if let Ok(arr) = opt_obj.as_array() {
                let mut opts = Vec::new();
                for item in arr {
                    if let Some(s) = Self::object_to_string(item) {
                        opts.push(s);
                    } else if let Ok(item_arr) = item.as_array() {
                        if let Some(s) = item_arr.get(1).and_then(Self::object_to_string) {
                            opts.push(s);
                        } else if let Some(s) = item_arr.get(0).and_then(Self::object_to_string) {
                            opts.push(s);
                        }
                    }
                }
                if !opts.is_empty() {
                    Some(opts)
                } else {
                    None
                }
            } else {
                None
            }
        });

        Some(FormField {
            id: format!("fld_{}_{}", page_num, name),
            name,
            page_number: page_num,
            field_type,
            x,
            y,
            width,
            height,
            value,
            default_value,
            read_only,
            required,
            multiline,
            options,
        })
    }

    fn resolve_field_name(doc: &Document, dict: &Dictionary) -> Option<String> {
        if let Ok(obj) = dict.get(b"T") {
            if let Some(s) = Self::object_to_string(obj) {
                return Some(s);
            }
        }
        // Check Parent if inherited
        if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
            if let Ok(parent_dict) = doc.get_object(*parent_id).and_then(|o| o.as_dict()) {
                return Self::resolve_field_name(doc, parent_dict);
            }
        }
        None
    }

    fn resolve_field_type(doc: &Document, dict: &Dictionary) -> FormFieldType {
        let ft_name = if let Ok(obj) = dict.get(b"FT") {
            obj.as_name().unwrap_or(b"")
        } else if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
            if let Ok(parent_dict) = doc.get_object(*parent_id).and_then(|o| o.as_dict()) {
                parent_dict.get(b"FT").and_then(|o| o.as_name()).unwrap_or(b"")
            } else {
                b""
            }
        } else {
            b""
        };

        let flags = Self::resolve_flags(doc, dict);

        match ft_name {
            b"Tx" => FormFieldType::Text,
            b"Btn" => {
                let is_radio = (flags & (1 << 15)) != 0;
                if is_radio {
                    FormFieldType::Radio
                } else {
                    FormFieldType::Checkbox
                }
            }
            b"Ch" => FormFieldType::Choice,
            b"Sig" => FormFieldType::Signature,
            _ => FormFieldType::Text,
        }
    }

    fn resolve_field_value(doc: &Document, dict: &Dictionary) -> Option<String> {
        if let Ok(obj) = dict.get(b"V") {
            if let Some(s) = Self::object_to_string(obj) {
                return Some(s);
            }
        }
        // Check Parent if inherited
        if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
            if let Ok(parent_dict) = doc.get_object(*parent_id).and_then(|o| o.as_dict()) {
                return Self::resolve_field_value(doc, parent_dict);
            }
        }
        None
    }

    fn resolve_flags(doc: &Document, dict: &Dictionary) -> i64 {
        if let Ok(Object::Integer(i)) = dict.get(b"Ff") {
            return *i;
        }
        if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
            if let Ok(parent_dict) = doc.get_object(*parent_id).and_then(|o| o.as_dict()) {
                if let Ok(Object::Integer(i)) = parent_dict.get(b"Ff") {
                    return *i;
                }
            }
        }
        0
    }

    fn get_page_height(page_dict: &Dictionary) -> f64 {
        let box_obj = page_dict.get(b"CropBox").or_else(|_| page_dict.get(b"MediaBox"));
        let to_f64 = |o: &Object| match o {
            Object::Real(f) => Some(*f as f64),
            Object::Integer(i) => Some(*i as f64),
            _ => None,
        };

        if let Ok(box_arr) = box_obj.and_then(|o| o.as_array()) {
            if box_arr.len() == 4 {
                let y1 = to_f64(&box_arr[1]).unwrap_or(0.0);
                let y2 = to_f64(&box_arr[3]).unwrap_or(842.0);
                return (y2 - y1).abs();
            }
        }
        842.0 // Default A4 height
    }

    fn object_to_string(obj: &Object) -> Option<String> {
        match obj {
            Object::String(bytes, _) => {
                // Check UTF-16BE BOM
                if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                    let u16_chars: Vec<u16> = bytes[2..]
                        .chunks_exact(2)
                        .map(|c| ((c[0] as u16) << 8) | (c[1] as u16))
                        .collect();
                    String::from_utf16(&u16_chars).ok()
                } else {
                    String::from_utf8(bytes.clone())
                        .ok()
                        .or_else(|| Some(String::from_utf8_lossy(bytes).to_string()))
                }
            }
            Object::Name(bytes) => String::from_utf8(bytes.clone()).ok(),
            Object::Integer(i) => Some(i.to_string()),
            Object::Real(f) => Some(f.to_string()),
            Object::Boolean(b) => Some(b.to_string()),
            _ => None,
        }
    }
}
