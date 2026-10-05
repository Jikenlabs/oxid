use oxid::connectors::traits::{DocumentReference, SecurityContext};
use oxid::connectors::{ConnectorRegistry, DocumentStorage};
use oxid::engine::annotations::AnnotationEngine;
use oxid::engine::builder::DocumentBuilderEngine;
use oxid::engine::comparison::ComparisonEngine;
use oxid::engine::pdf::PdfEngine;
use oxid::engine::redaction::RedactionEngine;
use oxid::models::{
    Annotation, AnnotationType, DocumentBuildOrder, PageAction, RedactionItem, RedactionOrder,
    WatermarkOptions,
};
use std::collections::HashMap;
use std::path::PathBuf;

fn ensure_sample_pdf() -> PathBuf {
    static INIT: std::sync::Once = std::sync::Once::new();
    let p = PathBuf::from("../sample_test.pdf");
    INIT.call_once(|| {
        if !p.exists() || std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) < 500 {
            use lopdf::{dictionary, Document, Object, Stream};
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

            let mut page_ids = Vec::new();
            for page_num in 1..=2 {
                let content = format!("BT\n/F1 14 Tf\n1 0 0 1 50 750 Tm\n(Sample Test Document - Page {}) Tj\nET\n", page_num);
                let content_id = doc.add_object(Stream::new(dictionary!(), content.into_bytes()));

                let page_dict = dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
                    "Resources" => resources_id,
                    "Contents" => content_id,
                };
                page_ids.push(doc.add_object(page_dict));
            }

            let pages_dict = dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
                "Count" => 2,
            };
            doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

            let catalog_id = doc.add_object(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            });
            doc.trailer.set("Root", Object::Reference(catalog_id));

            let tmp = PathBuf::from("../sample_test_tmp.pdf");
            doc.save(&tmp).unwrap();
            let _ = std::fs::rename(&tmp, &p);
        }
    });
    p
}

#[test]
fn test_metadata_and_rendering() {
    let sample_path = ensure_sample_pdf();

    let meta = PdfEngine::get_metadata("test-doc", "sample_test.pdf", &sample_path)
        .expect("Metadata extraction failed");

    assert_eq!(meta.page_count, 2);
    assert_eq!(meta.pages.len(), 2);
    assert!(meta.pages[0].width > 500.0);
    assert!(meta.pages[0].height > 800.0);

    let png_bytes = PdfEngine::render_page(&sample_path, 1, 150)
        .expect("Rendering page 1 failed");
    assert!(!png_bytes.is_empty());
    assert_eq!(&png_bytes[1..4], b"PNG");
}

#[test]
fn test_xfdf_roundtrip() {
    let annots = vec![
        Annotation {
            id: "ann-1".to_string(),
            page_number: 1,
            annotation_type: AnnotationType::Highlight,
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 20.0,
            color: "#FFE500".to_string(),
            opacity: 0.5,
            author: "Inspecteur".to_string(),
            created_at: "2026-09-11T20:00:00Z".to_string(),
            content: Some("Surlignage test".to_string()),
            points: vec![],
            reason: None,
        },
        Annotation {
            id: "ann-2".to_string(),
            page_number: 1,
            annotation_type: AnnotationType::Note,
            x: 80.0,
            y: 150.0,
            width: 24.0,
            height: 24.0,
            color: "#FFAA00".to_string(),
            opacity: 1.0,
            author: "Admin".to_string(),
            created_at: "2026-09-11T20:01:00Z".to_string(),
            content: Some("Validation requise".to_string()),
            points: vec![],
            reason: None,
        },
        Annotation {
            id: "ann-3".to_string(),
            page_number: 1,
            annotation_type: AnnotationType::Redact,
            x: 100.0,
            y: 200.0,
            width: 150.0,
            height: 25.0,
            color: "#000000".to_string(),
            opacity: 1.0,
            author: "DPO".to_string(),
            created_at: "2026-09-11T20:02:00Z".to_string(),
            content: Some("Texte secret".to_string()),
            points: vec![],
            reason: Some("RGPD".to_string()),
        },
    ];

    let xfdf_xml = AnnotationEngine::to_xfdf(&annots);
    assert!(xfdf_xml.contains("<highlight "));
    assert!(xfdf_xml.contains("<text "));
    assert!(xfdf_xml.contains("<redact "));

    let parsed = AnnotationEngine::from_xfdf(&xfdf_xml);
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0].annotation_type, AnnotationType::Highlight);
    assert_eq!(parsed[0].content, Some("Surlignage test".to_string()));
    assert_eq!(parsed[1].annotation_type, AnnotationType::Note);
    assert_eq!(parsed[1].content, Some("Validation requise".to_string()));
    assert_eq!(parsed[2].annotation_type, AnnotationType::Redact);
    assert_eq!(parsed[2].reason, Some("RGPD".to_string()));
}


#[test]
fn test_redaction_burn_in() {
    let sample_path = ensure_sample_pdf();
    let temp_dir = tempfile::tempdir().unwrap();
    let redacted_output = temp_dir.path().join("redacted.pdf");

    let order = RedactionOrder {
        document_id: "test-doc".to_string(),
        items: vec![RedactionItem {
            page_number: 1,
            x: 50.0,
            y: 700.0,
            width: 300.0,
            height: 25.0,
            reason: Some("RGPD".to_string()),
            overlay_text: Some("CONFIDENTIEL - RGPD".to_string()),
        }],
    };

    RedactionEngine::apply_redaction(&sample_path, &order, &redacted_output)
        .expect("Redaction failed");

    assert!(redacted_output.exists());
    let meta = PdfEngine::get_metadata("redacted", "redacted.pdf", &redacted_output).unwrap();
    assert_eq!(meta.page_count, 2);

    let png_bytes = PdfEngine::render_page(&redacted_output, 1, 100).unwrap();
    assert!(!png_bytes.is_empty());
}

#[test]
fn test_document_builder_and_watermark() {
    let sample_path = ensure_sample_pdf();
    let temp_dir = tempfile::tempdir().unwrap();
    let built_output = temp_dir.path().join("built.pdf");

    let order = DocumentBuildOrder {
        source_document_ids: vec!["test-doc".to_string()],
        page_actions: vec![
            PageAction::Rotate {
                page_number: 1,
                degrees: 90,
            },
            PageAction::Delete { page_number: 2 },
        ],
        watermark: Some(WatermarkOptions {
            text: "COPIE STRICTEMENT PRIVEE".to_string(),
            opacity: 0.4,
            font_size: 32.0,
            rotation: 45.0,
            color: "#888888".to_string(),
        }),
    };

    DocumentBuilderEngine::execute_build(&[sample_path], &order, &built_output)
        .expect("Build order failed");

    assert!(built_output.exists());
    let meta = PdfEngine::get_metadata("built", "built.pdf", &built_output).unwrap();
    assert_eq!(meta.page_count, 1);
    assert_eq!(meta.pages[0].rotation, 90);
    assert!((meta.pages[0].width - 842.0).abs() < 1.0);
    assert!((meta.pages[0].height - 595.0).abs() < 1.0);

    let rendered = PdfEngine::render_page(&built_output, 1, 100).expect("Rendering built page failed");
    assert!(!rendered.is_empty());
}

#[test]
fn test_builder_multi_page_deletion_rotation_and_watermark() {
    let sample_path = ensure_sample_pdf();
    let temp_dir = tempfile::tempdir().unwrap();
    let four_page_pdf = temp_dir.path().join("four_pages.pdf");
    let built_output = temp_dir.path().join("built_multi.pdf");

    // Fusionne 2x sample_test.pdf (2 pages chacun -> 4 pages)
    let status = std::process::Command::new("pdfunite")
        .arg(&sample_path)
        .arg(&sample_path)
        .arg(&four_page_pdf)
        .status()
        .expect("pdfunite failed");
    assert!(status.success());

    let four_meta = PdfEngine::get_metadata("4p", "4p.pdf", &four_page_pdf).unwrap();
    assert_eq!(four_meta.page_count, 4);

    // Supprime les pages 2 et 4, pivote la page 1 de 180° et la page 3 de 90°
    let order = DocumentBuildOrder {
        source_document_ids: vec!["multi-doc".to_string()],
        page_actions: vec![
            PageAction::Rotate {
                page_number: 1,
                degrees: 180,
            },
            PageAction::Delete { page_number: 2 },
            PageAction::Rotate {
                page_number: 3,
                degrees: 90,
            },
            PageAction::Delete { page_number: 4 },
        ],
        watermark: Some(WatermarkOptions {
            text: "PROPRIETE INTELLECTUELLE".to_string(),
            opacity: 0.35,
            font_size: 28.0,
            rotation: 45.0,
            color: "#FF0000".to_string(),
        }),
    };

    DocumentBuilderEngine::execute_build(&[four_page_pdf], &order, &built_output)
        .expect("Build order failed");

    assert!(built_output.exists());
    let meta = PdfEngine::get_metadata("built", "built_multi.pdf", &built_output).unwrap();
    assert_eq!(meta.page_count, 2, "Expected 2 pages after deleting 2 and 4");
    assert_eq!(meta.pages[0].rotation, 180);
    assert_eq!(meta.pages[1].rotation, 90);

    let p1_render = PdfEngine::render_page(&built_output, 1, 100).expect("Rendering page 1 failed");
    let p2_render = PdfEngine::render_page(&built_output, 2, 100).expect("Rendering page 2 failed");
    assert!(!p1_render.is_empty());
    assert!(!p2_render.is_empty());
}


#[test]
fn test_comparison_engine() {
    let sample_path = ensure_sample_pdf();

    // Compare la page 1 identique
    let result_same = ComparisonEngine::compare_pages(&sample_path, &sample_path, 1, 72).unwrap();
    assert!(result_same.diff_ratio < 0.01);
    assert!(!result_same.has_differences);
    assert!(!result_same.diff_image_png.is_empty());

    // Compare la page 2 identique
    let result_p2 = ComparisonEngine::compare_pages(&sample_path, &sample_path, 2, 72).unwrap();
    assert!(result_p2.diff_ratio < 0.01);
    assert!(!result_p2.has_differences);

    // Vérifie que les pages hors limites échouent proprement
    let result_out = ComparisonEngine::compare_pages(&sample_path, &sample_path, 99, 72);
    assert!(result_out.is_err());
}


#[tokio::test]
async fn test_connectors_filesystem_and_registry() {
    let sample_path = ensure_sample_pdf();
    let temp_dir = tempfile::tempdir().unwrap();
    let storage_dir = temp_dir.path().join("storage");
    let docs_dir = temp_dir.path().join("docs");
    std::fs::create_dir_all(&docs_dir).unwrap();

    // Copie sample_test.pdf dans docs_dir
    let sample_in_docs = docs_dir.join("sample.pdf");
    std::fs::copy(&sample_path, &sample_in_docs).unwrap();

    let registry = ConnectorRegistry::new(docs_dir.clone());
    let storage = DocumentStorage::new(storage_dir.clone());
    let ctx = SecurityContext::default();

    let doc_ref = DocumentReference {
        connector: "filesystem".to_string(),
        resource_id: "sample.pdf".to_string(),
        extra_params: HashMap::new(),
    };

    // Ouvre via le registre de connecteurs
    let doc_id = registry
        .open_document(&doc_ref, &ctx, &storage)
        .await
        .expect("Registry failed to open filesystem document");

    assert!(!doc_id.is_empty());

    // Vérifie que le chemin du document existe dans le stockage
    let path = storage.get_document_path(&doc_id);
    assert!(path.is_some());

    // Teste la sauvegarde d'annotations via le connecteur
    let xfdf_content = "<xfdf><annots><highlight page=\"0\"/></annots></xfdf>";
    registry
        .sync_annotations(&doc_id, xfdf_content, &ctx, &storage)
        .await
        .expect("Failed to sync annotations to connector");

    // Vérifie que le fichier .xfdf a été enregistré à la source
    let source_xfdf = docs_dir.join("sample.xfdf");
    assert!(source_xfdf.exists(), "Source XFDF file should be saved");
}

#[test]
fn test_text_converter_to_pdf() {
    let temp_dir = tempfile::tempdir().unwrap();
    let text_file = temp_dir.path().join("logfile.txt");
    let pdf_output = temp_dir.path().join("logfile.pdf");

    let sample_text = "2026-09-11 20:00:00 [INFO] System initialized\n".repeat(60);
    std::fs::write(&text_file, sample_text).unwrap();

    oxid::engine::converter::text::TextConverter::convert_text_to_pdf(&text_file, &pdf_output)
        .expect("Failed to convert text to PDF");

    assert!(pdf_output.exists());
    let meta = PdfEngine::get_metadata("txt-1", "logfile.txt", &pdf_output).unwrap();
    assert_eq!(meta.page_count, 2); // 60 lignes divisées en 48 lignes/page -> 2 pages
}

#[test]
fn test_email_converter_to_pdf() {
    let temp_dir = tempfile::tempdir().unwrap();
    let eml_file = temp_dir.path().join("test_email.eml");
    let pdf_output = temp_dir.path().join("test_email.pdf");

    let raw_eml = concat!(
        "From: Alice Dupont <alice@example.com>\r\n",
        "To: Bob Martin <bob@example.com>\r\n",
        "Subject: Rapport Financier Q3\r\n",
        "Date: Fri, 11 Sep 2026 14:30:00 +0200\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "\r\n",
        "Bonjour Bob,\r\nVeuillez trouver ci-joint le compte-rendu d activite.\r\nCordialement,\r\nAlice\r\n"
    );
    std::fs::write(&eml_file, raw_eml).unwrap();

    let res = oxid::engine::converter::email::EmailConverter::convert_eml_to_pdf(&eml_file, &pdf_output)
        .expect("Failed to convert EML to PDF");

    assert!(pdf_output.exists());
    let meta = PdfEngine::get_metadata("eml-1", "test_email.eml", &pdf_output).unwrap();
    assert_eq!(meta.page_count, 1);
    assert_eq!(res.attachments.len(), 0);

    let png = PdfEngine::render_page(&pdf_output, 1, 100).unwrap();
    assert!(!png.is_empty());
}

#[test]
fn test_docx_native_converter() {
    let temp_dir = tempfile::tempdir().unwrap();
    let docx_file = temp_dir.path().join("document.docx");
    let pdf_output = temp_dir.path().join("document.pdf");

    // Construit un DOCX valide minimal en mémoire (ZIP avec word/document.xml)
    {
        let file = std::fs::File::create(&docx_file).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("word/document.xml", options).unwrap();
        use std::io::Write;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body>
                <w:p><w:r><w:t>CONTRAT DE PRESTATION</w:t></w:r></w:p>
                <w:p><w:r><w:t>Article 1 : Objet du contrat d entreprise.</w:t></w:r></w:p>
                <w:p><w:r><w:t>Article 2 : Conditions financieres et clauses de confidentialite.</w:t></w:r></w:p>
            </w:body>
        </w:document>"#;
        zip.write_all(xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    oxid::engine::converter::office::OfficeConverter::convert_docx_native(&docx_file, &pdf_output)
        .expect("Failed to convert DOCX natively to PDF");

    assert!(pdf_output.exists());
    let meta = PdfEngine::get_metadata("docx-1", "document.docx", &pdf_output).unwrap();
    assert_eq!(meta.page_count, 1);

    let png = PdfEngine::render_page(&pdf_output, 1, 100).unwrap();
    assert!(!png.is_empty());
}

#[test]
fn test_docx_native_converter_with_embedded_image() {
    let temp_dir = tempfile::tempdir().unwrap();
    let docx_file = temp_dir.path().join("doc_with_img.docx");
    let pdf_output = temp_dir.path().join("doc_with_img.pdf");

    // Construit un DOCX avec une image PNG intégrée et sa relation
    {
        let file = std::fs::File::create(&docx_file).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. Mappage des relations
        zip.start_file("word/_rels/document.xml.rels", options).unwrap();
        use std::io::Write;
        let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
        <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
            <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
        </Relationships>"#;
        zip.write_all(rels.as_bytes()).unwrap();

        // 2. Image PNG factice 2x2 dans word/media/image1.png
        zip.start_file("word/media/image1.png", options).unwrap();
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, image::ImageFormat::Png).unwrap();
        zip.write_all(png_buf.get_ref()).unwrap();

        // 3. Corps du document référençant rId1
        zip.start_file("word/document.xml", options).unwrap();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                    xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
                    xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:body>
                <w:p><w:r><w:t>Titre du document avec image</w:t></w:r></w:p>
                <w:p>
                    <w:drawing>
                        <wp:extent cx="1270000" cy="1270000"/>
                        <a:blip r:embed="rId1"/>
                    </w:drawing>
                </w:p>
                <w:p><w:r><w:t>Paragraphe apres image.</w:t></w:r></w:p>
            </w:body>
        </w:document>"#;
        zip.write_all(xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    oxid::engine::converter::office::OfficeConverter::convert_docx_native(&docx_file, &pdf_output)
        .expect("Failed to convert DOCX with image natively to PDF");

    assert!(pdf_output.exists());
    let meta = PdfEngine::get_metadata("docx-img", "doc_with_img.docx", &pdf_output).unwrap();
    assert_eq!(meta.page_count, 1);

    let png = PdfEngine::render_page(&pdf_output, 1, 100).unwrap();
    assert!(!png.is_empty());
}

#[test]
fn test_real_user_docx_conversion() {
    let docx_path = std::path::Path::new("../data/documents/e1bf1387-3c68-4e36-a1de-ca2f8c3418af.docx");
    if docx_path.exists() {
        let pdf_output = std::path::Path::new("../data/documents/e1bf1387-3c68-4e36-a1de-ca2f8c3418af.docx.rendition.pdf");
        oxid::engine::converter::office::OfficeConverter::convert_to_pdf(docx_path, pdf_output)
            .expect("Failed to convert real docx file via high-fidelity converter");
        assert!(pdf_output.exists());
        let meta = PdfEngine::get_metadata("real-docx", "test.docx", pdf_output).unwrap();
        println!("Real user DOCX successfully converted with high fidelity: {} pages", meta.page_count);
        assert!(meta.page_count >= 5);
    }
}
#[test]
fn test_pii_algorithms() {
    use oxid::engine::pii::PiiEngine;

    // 1. Validation carte bancaire (Luhn)
    assert!(PiiEngine::is_valid_luhn("4532015112830366")); // Visa de test valide
    assert!(!PiiEngine::is_valid_luhn("4532015112830367")); // Somme de contrôle invalide

    // 2. Validation IBAN (FR76...)
    assert!(PiiEngine::is_valid_iban("FR7630006000011234567890189"));
    assert!(!PiiEngine::is_valid_iban("FR7630006000011234567890199")); // Clé invalide

    // 3. Validation numéro de sécurité sociale français (NIR)
    // Clé = 97 - (1851275108042 % 97)
    let nir_base: u64 = 1851275108042;
    let expected_key = 97 - (nir_base % 97);
    let full_nir = format!("{}{:02}", nir_base, expected_key);
    assert!(PiiEngine::is_valid_french_nir(&full_nir));
    assert!(!PiiEngine::is_valid_french_nir("185127510804200")); // Mauvaise clé

    // 4. Validation adresse email
    assert!(PiiEngine::is_valid_email("contact@acme.com"));
    assert!(!PiiEngine::is_valid_email("not-an-email"));

    // 5. Validation numéro de téléphone
    assert!(PiiEngine::is_valid_phone("+33612345678"));
    assert!(PiiEngine::is_valid_phone("0612345678"));
}

#[test]
fn test_pii_and_search_spans() {
    use oxid::engine::pii::PiiEngine;
    use oxid::models::{PiiCategory, TextSpan};

    let spans = vec![
        TextSpan {
            text: "Client:".to_string(),
            x: 10.0,
            y: 50.0,
            width: 40.0,
            height: 12.0,
            font_size: 12.0,
        },
        TextSpan {
            text: "dupont@defense.gouv.fr".to_string(),
            x: 55.0,
            y: 50.0,
            width: 120.0,
            height: 12.0,
            font_size: 12.0,
        },
        // IBAN multi-segments : FR76 3000 6000 0112 3456 7890 189
        TextSpan {
            text: "FR7630006000011234567890189".to_string(),
            x: 10.0,
            y: 100.0,
            width: 200.0,
            height: 12.0,
            font_size: 12.0,
        },
    ];

    let pii_items = PiiEngine::scan_page_spans(1, &spans);
    assert_eq!(pii_items.len(), 2);
    assert_eq!(pii_items[0].category, PiiCategory::Email);
    assert_eq!(pii_items[1].category, PiiCategory::Iban);
}

#[test]
fn test_digital_signature_and_stamp() {
    use oxid::engine::signature::DigitalSignatureEngine;
    use oxid::models::SignRequest;

    let temp_dir = tempfile::tempdir().unwrap();
    let src_pdf = temp_dir.path().join("original.pdf");
    let signed_pdf = temp_dir.path().join("signed.pdf");

    let txt_file = temp_dir.path().join("original.txt");
    std::fs::write(&txt_file, "ACCORD DE PARTENARIAT ET CONFIDENTIALITE\nDate: 2026-09-11\nPartie A et Partie B").unwrap();
    oxid::engine::converter::text::TextConverter::convert_text_to_pdf(
        &txt_file,
        &src_pdf,
    ).unwrap();

    let sign_req = SignRequest {
        page_number: 1,
        x: 50.0,
        y: 600.0,
        width: 260.0,
        height: 70.0,
        signer_name: "Jean Dupont".to_string(),
        reason: Some("Approbation Direction".to_string()),
        location: Some("Paris, France".to_string()),
        handwritten_png_base64: None,
    };

    let resp = DigitalSignatureEngine::sign_pdf(&src_pdf, &signed_pdf, &sign_req)
        .expect("Signature PDF failed");

    assert!(signed_pdf.exists());
    assert_eq!(resp.page_number, 1);
    assert!(!resp.sha256_digest.is_empty());

    // Vérifie le rendu du PDF signé avec le cartouche visuel
    let png = PdfEngine::render_page(&signed_pdf, 1, 100).unwrap();
    assert!(!png.is_empty());

    // Vérifie que le catalogue PDF contient les métadonnées de signature
    let doc = lopdf::Document::load(&signed_pdf).unwrap();
    let root_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let root_dict = doc.get_object(root_id).unwrap().as_dict().unwrap();
    assert!(root_dict.has(b"OxidSignature"));
}

#[tokio::test]
async fn test_cluster_cache_and_metrics() {
    let temp_cache = tempfile::tempdir().unwrap();
    let cache = oxid::cache::CacheManager::with_redis(
        temp_cache.path().to_path_buf(),
        64,
        Some("redis://valkey-cluster:6379".to_string()),
    );

    let key = "doc123-p1-150";
    let fake_image_data = vec![1, 2, 3, 4, 5, 6, 7, 8];

    // Échec de cache initial (Miss)
    let res = cache.get(key).await;
    assert!(res.is_none());
    assert_eq!(cache.stats.misses.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Écriture (Set)
    cache.set(key, fake_image_data.clone()).await;
    assert_eq!(cache.stats.writes.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Succès Cache L1 RAM (Hit)
    let res2 = cache.get(key).await;
    assert_eq!(res2, Some(fake_image_data.clone()));
    assert_eq!(cache.stats.l1_hits.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Simule l'éviction du L1 pour tester la récupération L2
    cache.clear_for_doc("doc123").await;
    assert_eq!(cache.l1_entry_count().await, 0);

    // Succès Cache L2 Disque / Distribué (Hit)
    let res3 = cache.get(key).await;
    assert_eq!(res3, Some(fake_image_data));
    assert_eq!(cache.stats.l2_hits.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn test_extended_formats_support() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. Test du support des formats d'image (WebP & PNG)
    let img_path = temp_dir.path().join("test.png");
    let img = image::RgbImage::new(100, 100);
    img.save(&img_path).unwrap();

    let meta = PdfEngine::get_metadata("img-1", "test.png", &img_path).unwrap();
    assert_eq!(meta.page_count, 1);
    assert_eq!(meta.mime_type, "image/png");

    let rendered = PdfEngine::render_page(&img_path, 1, 100).unwrap();
    assert!(!rendered.is_empty());
    assert_eq!(&rendered[1..4], b"PNG");

    // 2. Test de la représentation native OpenDocument Draw (ODG) / Visio (VSDX)
    let odg_path = temp_dir.path().join("diagram.odg");
    // Crée une archive zip factice contenant content.xml
    {
        let file = std::fs::File::create(&odg_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("content.xml", options).unwrap();
        use std::io::Write;
        zip.write_all(b"<office:document-content><text:p>Schema d'architecture reseau</text:p><text:p>Serveur Principal - Routeur 10.0.0.1</text:p></office:document-content>").unwrap();
        zip.finish().unwrap();
    }

    let meta_odg = PdfEngine::get_metadata("odg-1", "diagram.odg", &odg_path).unwrap();
    assert!(meta_odg.page_count >= 1);

    let rendered_odg = PdfEngine::render_page(&odg_path, 1, 100).unwrap();
    assert!(!rendered_odg.is_empty());
}

#[test]
fn test_presentation_slide_text_accuracy() {
    let pdf_path = PathBuf::from("../data/documents/fdd273df-1cfe-4bdc-bc22-a5d140ad47c8.pdf");
    if !pdf_path.exists() {
        return;
    }

    // Page 1 : Vérifie que "Cadrage" et "bis" ne débordent pas
    let p1 = PdfEngine::get_page_text(&pdf_path, 1).unwrap();
    let cadrage = p1.spans.iter().find(|s| s.text == "Cadrage").unwrap();
    assert!(
        cadrage.x + cadrage.width < 580.0 && cadrage.x + cadrage.width > 565.0,
        "Cadrage end_x must be around 573, got {}",
        cadrage.x + cadrage.width
    );

    let bis = p1.spans.iter().find(|s| s.text == "bis").unwrap();
    assert!(
        bis.x + bis.width < 798.0 && bis.x + bis.width > 785.0,
        "bis end_x must be around 792, got {}",
        bis.x + bis.width
    );

    // Page 2 : Vérifie que "solution" dans "Périmètre de la solution" est intégralement couvert
    let p2 = PdfEngine::get_page_text(&pdf_path, 2).unwrap();
    let solution = p2.spans.iter().find(|s| s.text == "solution").unwrap();
    assert!(
        solution.x + solution.width > 435.0 && solution.x + solution.width < 450.0,
        "solution end_x must cover full word ending around 441-445, got {}",
        solution.x + solution.width
    );
}

#[test]
fn test_acroform_extraction_and_fill() {
    use lopdf::{Dictionary, Document, Object, StringFormat};
    use oxid::engine::forms::FormEngine;

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    // Crée un widget de champ texte (Text Field)
    let mut field1_dict = Dictionary::new();
    field1_dict.set("Type", Object::Name(b"Annot".to_vec()));
    field1_dict.set("Subtype", Object::Name(b"Widget".to_vec()));
    field1_dict.set("FT", Object::Name(b"Tx".to_vec()));
    field1_dict.set(
        "T",
        Object::String(b"FirstName".to_vec(), StringFormat::Literal),
    );
    field1_dict.set(
        "V",
        Object::String(b"Jean".to_vec(), StringFormat::Literal),
    );
    field1_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(50.0),
            Object::Real(700.0),
            Object::Real(250.0),
            Object::Real(720.0),
        ]),
    );
    field1_dict.set("P", Object::Reference(page_id));
    let field1_id = doc.add_object(Object::Dictionary(field1_dict));

    // Crée un widget de case à cocher (Checkbox)
    let mut field2_dict = Dictionary::new();
    field2_dict.set("Type", Object::Name(b"Annot".to_vec()));
    field2_dict.set("Subtype", Object::Name(b"Widget".to_vec()));
    field2_dict.set("FT", Object::Name(b"Btn".to_vec()));
    field2_dict.set(
        "T",
        Object::String(b"AgreeTerms".to_vec(), StringFormat::Literal),
    );
    field2_dict.set("V", Object::Name(b"Yes".to_vec()));
    field2_dict.set(
        "Rect",
        Object::Array(vec![
            Object::Real(50.0),
            Object::Real(650.0),
            Object::Real(70.0),
            Object::Real(670.0),
        ]),
    );
    field2_dict.set("P", Object::Reference(page_id));
    let field2_id = doc.add_object(Object::Dictionary(field2_dict));

    // Dictionnaire Page
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    page_dict.set(
        "Annots",
        Object::Array(vec![
            Object::Reference(field1_id),
            Object::Reference(field2_id),
        ]),
    );
    doc.objects.insert(page_id, Object::Dictionary(page_dict));

    // Dictionnaire Pages
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages_dict.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Dictionnaire AcroForm
    let mut acro_dict = Dictionary::new();
    acro_dict.set(
        "Fields",
        Object::Array(vec![
            Object::Reference(field1_id),
            Object::Reference(field2_id),
        ]),
    );
    let acro_id = doc.add_object(Object::Dictionary(acro_dict));

    // Dictionnaire Catalog
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog_dict.set("Pages", Object::Reference(pages_id));
    catalog_dict.set("AcroForm", Object::Reference(acro_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let temp_dir = std::env::temp_dir();
    let form_pdf_path = temp_dir.join("test_acroform.pdf");
    doc.save(&form_pdf_path).unwrap();

    // 1. Test d'extraction
    let summary = FormEngine::extract_form_fields(&form_pdf_path).unwrap();
    assert_eq!(summary.fields_count, 2);
    assert!(summary.has_forms);

    let f1 = summary.fields.iter().find(|f| f.name == "FirstName").unwrap();
    assert_eq!(f1.field_type, oxid::models::FormFieldType::Text);
    assert_eq!(f1.value, "Jean");
    assert_eq!(f1.page_number, 1);
    // Hauteur de page = 792. Rect y supérieur était à 720 -> y visionneuse = 792 - 720 = 72.
    assert!((f1.y - 72.0).abs() < 1.0);

    let f2 = summary
        .fields
        .iter()
        .find(|f| f.name == "AgreeTerms")
        .unwrap();
    assert_eq!(f2.field_type, oxid::models::FormFieldType::Checkbox);
    assert_eq!(f2.value, "Yes");

    // 2. Test de remplissage
    let mut fill_values = HashMap::new();
    fill_values.insert(
        "FirstName".to_string(),
        serde_json::Value::String("Alice Dupont".to_string()),
    );
    fill_values.insert(
        "AgreeTerms".to_string(),
        serde_json::Value::String("Off".to_string()),
    );

    let updated_count =
        FormEngine::fill_form_fields(&form_pdf_path, &form_pdf_path, &fill_values).unwrap();
    assert_eq!(updated_count, 2);

    // 3. Ré-extraction pour vérifier les valeurs mises à jour
    let updated_summary = FormEngine::extract_form_fields(&form_pdf_path).unwrap();
    let updated_f1 = updated_summary
        .fields
        .iter()
        .find(|f| f.name == "FirstName")
        .unwrap();
    assert_eq!(updated_f1.value, "Alice Dupont");

    let _ = std::fs::remove_file(form_pdf_path);
}

#[test]
fn test_generate_sample_form_file() {
    use lopdf::{Dictionary, Document, Object, StringFormat};

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    // 1. Champ texte : Nom_Complet
    let mut f1 = Dictionary::new();
    f1.set("Type", Object::Name(b"Annot".to_vec()));
    f1.set("Subtype", Object::Name(b"Widget".to_vec()));
    f1.set("FT", Object::Name(b"Tx".to_vec()));
    f1.set(
        "T",
        Object::String(b"Nom_Complet".to_vec(), StringFormat::Literal),
    );
    f1.set(
        "V",
        Object::String(b"Jean Dupont".to_vec(), StringFormat::Literal),
    );
    f1.set(
        "Rect",
        Object::Array(vec![
            Object::Real(100.0),
            Object::Real(680.0),
            Object::Real(350.0),
            Object::Real(705.0),
        ]),
    );
    f1.set("P", Object::Reference(page_id));
    let f1_id = doc.add_object(Object::Dictionary(f1));

    // 2. Champ texte multiligne : Commentaires
    let mut f2 = Dictionary::new();
    f2.set("Type", Object::Name(b"Annot".to_vec()));
    f2.set("Subtype", Object::Name(b"Widget".to_vec()));
    f2.set("FT", Object::Name(b"Tx".to_vec()));
    f2.set("Ff", Object::Integer(1 << 12)); // Indicateur multiligne (bit 13)
    f2.set(
        "T",
        Object::String(b"Commentaires".to_vec(), StringFormat::Literal),
    );
    f2.set(
        "V",
        Object::String(
            b"Formulaire valide et conforme.".to_vec(),
            StringFormat::Literal,
        ),
    );
    f2.set(
        "Rect",
        Object::Array(vec![
            Object::Real(100.0),
            Object::Real(580.0),
            Object::Real(500.0),
            Object::Real(650.0),
        ]),
    );
    f2.set("P", Object::Reference(page_id));
    let f2_id = doc.add_object(Object::Dictionary(f2));

    // 3. Case à cocher : Consentement_RGPD
    let mut f3 = Dictionary::new();
    f3.set("Type", Object::Name(b"Annot".to_vec()));
    f3.set("Subtype", Object::Name(b"Widget".to_vec()));
    f3.set("FT", Object::Name(b"Btn".to_vec()));
    f3.set(
        "T",
        Object::String(b"Consentement_RGPD".to_vec(), StringFormat::Literal),
    );
    f3.set("V", Object::Name(b"Yes".to_vec()));
    f3.set(
        "Rect",
        Object::Array(vec![
            Object::Real(100.0),
            Object::Real(530.0),
            Object::Real(124.0),
            Object::Real(554.0),
        ]),
    );
    f3.set("P", Object::Reference(page_id));
    let f3_id = doc.add_object(Object::Dictionary(f3));

    // 4. Liste de choix : Departement
    let mut f4 = Dictionary::new();
    f4.set("Type", Object::Name(b"Annot".to_vec()));
    f4.set("Subtype", Object::Name(b"Widget".to_vec()));
    f4.set("FT", Object::Name(b"Ch".to_vec()));
    f4.set(
        "T",
        Object::String(b"Departement".to_vec(), StringFormat::Literal),
    );
    f4.set(
        "V",
        Object::String(b"R&D".to_vec(), StringFormat::Literal),
    );
    f4.set(
        "Opt",
        Object::Array(vec![
            Object::String(b"Direction".to_vec(), StringFormat::Literal),
            Object::String(b"R&D".to_vec(), StringFormat::Literal),
            Object::String(b"Finance".to_vec(), StringFormat::Literal),
            Object::String(b"RH".to_vec(), StringFormat::Literal),
        ]),
    );
    f4.set(
        "Rect",
        Object::Array(vec![
            Object::Real(100.0),
            Object::Real(470.0),
            Object::Real(300.0),
            Object::Real(495.0),
        ]),
    );
    f4.set("P", Object::Reference(page_id));
    let f4_id = doc.add_object(Object::Dictionary(f4));

    // 5. Champ de signature : Signature_Client
    let mut f5 = Dictionary::new();
    f5.set("Type", Object::Name(b"Annot".to_vec()));
    f5.set("Subtype", Object::Name(b"Widget".to_vec()));
    f5.set("FT", Object::Name(b"Sig".to_vec()));
    f5.set(
        "T",
        Object::String(b"Signature_Client".to_vec(), StringFormat::Literal),
    );
    f5.set(
        "Rect",
        Object::Array(vec![
            Object::Real(100.0),
            Object::Real(380.0),
            Object::Real(350.0),
            Object::Real(440.0),
        ]),
    );
    f5.set("P", Object::Reference(page_id));
    let f5_id = doc.add_object(Object::Dictionary(f5));

    // Dictionnaire Page
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    page_dict.set(
        "Annots",
        Object::Array(vec![
            Object::Reference(f1_id),
            Object::Reference(f2_id),
            Object::Reference(f3_id),
            Object::Reference(f4_id),
            Object::Reference(f5_id),
        ]),
    );
    doc.objects.insert(page_id, Object::Dictionary(page_dict));

    // Dictionnaire Pages
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages_dict.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Dictionnaire AcroForm
    let mut acro_dict = Dictionary::new();
    acro_dict.set(
        "Fields",
        Object::Array(vec![
            Object::Reference(f1_id),
            Object::Reference(f2_id),
            Object::Reference(f3_id),
            Object::Reference(f4_id),
            Object::Reference(f5_id),
        ]),
    );
    let acro_id = doc.add_object(Object::Dictionary(acro_dict));

    // Dictionnaire Catalog
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog_dict.set("Pages", Object::Reference(pages_id));
    catalog_dict.set("AcroForm", Object::Reference(acro_id));
    let catalog_id = doc.add_object(Object::Dictionary(catalog_dict));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let sample_path = PathBuf::from("../sample_form.pdf");
    doc.save(&sample_path).unwrap();

    let docs_dir = PathBuf::from("../data/documents");
    if docs_dir.exists() {
        let _ = doc.save(docs_dir.join("sample_form.pdf"));
    }

    use oxid::engine::forms::FormEngine;
    use oxid::models::FormFieldType;

    let summary = FormEngine::extract_form_fields(&sample_path).unwrap();
    assert_eq!(summary.fields_count, 5);
    assert!(summary.has_forms);

    let nom = summary.fields.iter().find(|f| f.name == "Nom_Complet").unwrap();
    assert_eq!(nom.field_type, FormFieldType::Text);
    assert_eq!(nom.value, "Jean Dupont");

    let comm = summary.fields.iter().find(|f| f.name == "Commentaires").unwrap();
    assert_eq!(comm.field_type, FormFieldType::Text);
    assert!(comm.multiline);

    let rgpd = summary.fields.iter().find(|f| f.name == "Consentement_RGPD").unwrap();
    assert_eq!(rgpd.field_type, FormFieldType::Checkbox);
    assert_eq!(rgpd.value, "Yes");

    let dept = summary.fields.iter().find(|f| f.name == "Departement").unwrap();
    assert_eq!(dept.field_type, FormFieldType::Choice);
    assert_eq!(dept.value, "R&D");
    assert_eq!(dept.options.as_ref().unwrap().len(), 4);

    let sig = summary.fields.iter().find(|f| f.name == "Signature_Client").unwrap();
    assert_eq!(sig.field_type, FormFieldType::Signature);

    println!("All 5 fields extracted with 100% fidelity!");
}

#[tokio::test]
async fn test_cors_http_headers() {
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use oxid::config::AppConfig;
    use tower::ServiceExt;

    // 1. Test avec wildcard par défaut (*)
    let mut config_default = AppConfig::default();
    config_default.cors_allowed_origins = vec!["*".to_string()];
    config_default.cors_allow_credentials = false;

    let app_wildcard = Router::new()
        .route("/test", get(|| async { "ok" }))
        .layer(config_default.build_cors_layer());

    let req = Request::builder()
        .method("GET")
        .uri("/test")
        .header(header::ORIGIN, "https://example.com")
        .body(Body::empty())
        .unwrap();

    let res = app_wildcard.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );

    // 2. Test avec origines explicites et credentials
    let mut config_restricted = AppConfig::default();
    config_restricted.cors_allowed_origins = vec![
        "https://app.client.com".to_string(),
        "http://localhost:3000".to_string(),
    ];
    config_restricted.cors_allow_credentials = true;

    let app_restricted = Router::new()
        .route("/test", get(|| async { "ok" }))
        .layer(config_restricted.build_cors_layer());

    // Requête GET depuis une origine explicitement autorisée
    let req_allowed = Request::builder()
        .method("GET")
        .uri("/test")
        .header(header::ORIGIN, "https://app.client.com")
        .body(Body::empty())
        .unwrap();

    let res_allowed = app_restricted.clone().oneshot(req_allowed).await.unwrap();
    assert_eq!(res_allowed.status(), StatusCode::OK);
    assert_eq!(
        res_allowed
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://app.client.com")
    );
    assert_eq!(
        res_allowed
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .and_then(|v| v.to_str().ok()),
        Some("true")
    );

    // Requête preflight OPTIONS
    let req_preflight = Request::builder()
        .method("OPTIONS")
        .uri("/test")
        .header(header::ORIGIN, "https://app.client.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();

    let res_preflight = app_restricted.clone().oneshot(req_preflight).await.unwrap();
    assert_eq!(res_preflight.status(), StatusCode::OK);
    assert_eq!(
        res_preflight
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://app.client.com")
    );

    // Requête depuis une origine non autorisée (doit refuser l'en-tête Allow-Origin)
    let req_denied = Request::builder()
        .method("GET")
        .uri("/test")
        .header(header::ORIGIN, "https://malicious-site.com")
        .body(Body::empty())
        .unwrap();

    let res_denied = app_restricted.oneshot(req_denied).await.unwrap();
    assert!(res_denied
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());
}





