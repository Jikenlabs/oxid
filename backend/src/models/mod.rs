use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailAttachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: u64,
    pub page_count: usize,
    pub pages: Vec<PageMetadata>,
    pub bookmarks: Vec<Bookmark>,
    #[serde(default)]
    pub attachments: Vec<EmailAttachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageMetadata {
    pub page_number: usize,
    pub width: f64,
    pub height: f64,
    pub rotation: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub page_number: usize,
    #[serde(default)]
    pub children: Vec<Bookmark>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub font_size: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageText {
    pub page_number: usize,
    pub spans: Vec<TextSpan>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationType {
    Highlight,
    Underline,
    Strikeout,
    Note,
    Freehand,
    Rectangle,
    Stamp,
    Redact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub page_number: usize,
    pub annotation_type: AnnotationType,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub color: String,
    pub opacity: f32,
    pub author: String,
    pub created_at: String,
    pub content: Option<String>,
    #[serde(default)]
    pub points: Vec<(f64, f64)>, // Pour tracé libre / polygone
    pub reason: Option<String>,   // Motif de caviardage
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedactionItem {
    pub page_number: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub reason: Option<String>,
    pub overlay_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedactionOrder {
    pub document_id: String,
    pub items: Vec<RedactionItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PageAction {
    Keep { page_number: usize, rotation: Option<i32> },
    Rotate { page_number: usize, degrees: i32 },
    Delete { page_number: usize },
    InsertBlank { width: f64, height: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentBuildOrder {
    pub source_document_ids: Vec<String>,
    pub page_actions: Vec<PageAction>,
    pub watermark: Option<WatermarkOptions>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatermarkOptions {
    pub text: String,
    pub opacity: f32,
    pub font_size: f64,
    pub rotation: f64,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareRequest {
    pub doc_a_id: String,
    pub doc_b_id: String,
    pub page_number: usize,
    #[serde(default = "default_compare_mode")]
    pub mode: String, // "visual" | "text"
}

fn default_compare_mode() -> String {
    "visual".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareResponse {
    pub page_number: usize,
    pub diff_ratio: f64, // 0.0 à 1.0
    pub has_differences: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiffOp {
    Equal,
    Insert,
    Delete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextDiffToken {
    pub op: DiffOp,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextDiffResult {
    pub page_number: usize,
    pub additions_count: usize,
    pub deletions_count: usize,
    pub unchanged_count: usize,
    pub diff_ratio: f64,
    pub tokens: Vec<TextDiffToken>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PiiCategory {
    CreditCard,
    Iban,
    SocialSecurity,
    Email,
    Phone,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PiiItem {
    pub id: String,
    pub category: PiiCategory,
    pub matched_text: String,
    pub masked_preview: String,
    pub page_number: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PiiScanResponse {
    pub total_found: usize,
    pub items: Vec<PiiItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchMatchItem {
    pub page_number: usize,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalSearchResponse {
    pub query: String,
    pub total_matches: usize,
    pub matches: Vec<SearchMatchItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignRequest {
    pub page_number: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub signer_name: String,
    pub reason: Option<String>,
    pub location: Option<String>,
    pub handwritten_png_base64: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignResponse {
    pub signed_doc_id: String,
    pub sha256_digest: String,
    pub timestamp_utc: String,
    pub page_number: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FormFieldType {
    Text,
    Checkbox,
    Radio,
    Choice,
    Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormField {
    pub id: String,
    pub name: String,
    pub page_number: usize,
    pub field_type: FormFieldType,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub value: String,
    pub default_value: Option<String>,
    pub read_only: bool,
    pub required: bool,
    pub multiline: bool,
    pub options: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormFieldsSummary {
    pub has_forms: bool,
    pub fields_count: usize,
    pub fields: Vec<FormField>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormFillRequest {
    pub values: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub save_as_new: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormFillResponse {
    pub document_id: String,
    pub updated_fields_count: usize,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CadLayerInfo {
    pub name: String,
    pub color_hex: String,
    pub is_visible: bool,
    pub entity_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CadMetadata {
    pub layers: Vec<CadLayerInfo>,
    pub width: f64,
    pub height: f64,
    pub units: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DicomPreset {
    pub name: String,
    pub label: String,
    pub window_center: f64,
    pub window_width: f64,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DicomMetadata {
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
    pub rescale_intercept: f64,
    pub rescale_slope: f64,
    pub default_window_center: f64,
    pub default_window_width: f64,
    #[serde(default)]
    pub window_center: Option<f64>,
    #[serde(default)]
    pub window_width: Option<f64>,
    pub pixel_spacing: Option<(f64, f64)>,
    pub presets: Vec<DicomPreset>,
}


