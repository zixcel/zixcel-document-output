#[path = "support/pdf_fixture.rs"]
mod fixture;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zixcel_document_output::{
    PdfAnalysisRequest, PdfReadCapability, PdfWriteCapability, analyze_pdf, write_pdf_report,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "zixcel-pdf-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn request(&self) -> PdfAnalysisRequest {
        let bytes = fixture::document_bytes();
        fs::write(self.0.join("input.pdf"), &bytes).unwrap();
        PdfAnalysisRequest {
            input_ref: "input.pdf".into(),
            input_digest: format!("{:x}", Sha256::digest(&bytes)),
            page_index: None,
        }
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn extracts_text_metadata_and_exclusive_page_classes_without_writes() {
    let w = Workspace::new();
    let request = w.request();
    let before = fs::read(w.0.join("input.pdf")).unwrap();
    let report = analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &request).unwrap();
    assert!(report.text.contains("First page") && report.text.contains("Third page"));
    assert_eq!(report.metadata["Title"], "例示資料");
    assert_eq!(report.metadata["CreationDate"], "2025-03-14T05:12:47+09:00");
    assert_eq!(
        (
            report.summary.total_pages,
            report.summary.text_pages,
            report.summary.image_include_pages,
            report.summary.image_only_pages,
            report.summary.blank_pages
        ),
        (4, 2, 2, 1, 1)
    );
    assert!(report.pages[1].is_image_only);
    assert!(report.pages[2].has_image);
    assert!(report.pages[3].is_blank);
    assert_eq!(before, fs::read(w.0.join("input.pdf")).unwrap());
    assert_eq!(fs::read_dir(&w.0).unwrap().count(), 1);
}
#[test]
fn selects_zero_based_pages_and_rejects_out_of_range() {
    let w = Workspace::new();
    let mut r = w.request();
    let read = PdfReadCapability::authorize(&w.0).unwrap();
    r.page_index = Some(2);
    let report = analyze_pdf(&read, &r).unwrap();
    assert!(report.text.contains("Third page"));
    assert!(!report.text.contains("First page"));
    r.page_index = Some(4);
    assert!(analyze_pdf(&read, &r).is_err());
}
#[test]
fn requires_the_exact_input_digest() {
    let w = Workspace::new();
    let mut r = w.request();
    r.input_digest = "0".repeat(64);
    assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &r).is_err());
}
#[test]
fn rejects_traversal_and_absolute_inputs() {
    let w = Workspace::new();
    let mut r = w.request();
    for path in ["../input.pdf", "/input.pdf", "C:\\input.pdf"] {
        r.input_ref = path.into();
        assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &r).is_err());
    }
}
#[test]
fn rejects_invalid_and_oversized_documents() {
    let w = Workspace::new();
    let mut r = w.request();
    for bytes in [b"not a PDF".to_vec(), vec![b'x'; 8 * 1024 * 1024 + 1]] {
        fs::write(w.0.join("input.pdf"), &bytes).unwrap();
        r.input_digest = format!("{:x}", Sha256::digest(&bytes));
        assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &r).is_err());
    }
}
#[test]
fn report_creation_requires_a_separate_root_and_preserves_existing_files() {
    let read_root = Workspace::new();
    let write_root = Workspace::new();
    let request = read_root.request();
    let report = analyze_pdf(
        &PdfReadCapability::authorize(&read_root.0).unwrap(),
        &request,
    )
    .unwrap();
    let write = PdfWriteCapability::authorize(&write_root.0).unwrap();
    write_pdf_report(&write, "report.json", &report).unwrap();
    let first = fs::read(write_root.0.join("report.json")).unwrap();
    assert!(write_pdf_report(&write, "report.json", &report).is_err());
    assert_eq!(first, fs::read(write_root.0.join("report.json")).unwrap());
    assert!(write_pdf_report(&write, "../escape.json", &report).is_err());
    assert_eq!(fs::read_dir(&write_root.0).unwrap().count(), 1);
    assert_eq!(
        serde_json::from_slice::<zixcel_document_output::PdfAnalysisReport>(&first).unwrap(),
        report
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(write_root.0.join("report.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
#[cfg(unix)]
fn rejects_input_and_output_symlinks() {
    use std::os::unix::fs::symlink;
    let w = Workspace::new();
    let mut r = w.request();
    symlink(w.0.join("input.pdf"), w.0.join("alias.pdf")).unwrap();
    r.input_ref = "alias.pdf".into();
    assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &r).is_err());
    r.input_ref = "input.pdf".into();
    let report = analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &r).unwrap();
    symlink(w.0.join("input.pdf"), w.0.join("alias.json")).unwrap();
    assert!(
        write_pdf_report(
            &PdfWriteCapability::authorize(&w.0).unwrap(),
            "alias.json",
            &report
        )
        .is_err()
    );
}

#[test]
fn rejects_a_compressed_stream_that_exceeds_the_decompression_limit() {
    use lopdf::{Document, Object};
    let w = Workspace::new();
    let mut doc = Document::load_mem(&fixture::document_bytes()).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let id = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    let Object::Stream(stream) = doc.objects.get_mut(&id).unwrap() else {
        panic!("fixture stream")
    };
    stream.set_plain_content(vec![b' '; 8 * 1024 * 1024 + 1]);
    stream.compress().unwrap();
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    assert!(bytes.len() < 8 * 1024 * 1024);
    fs::write(w.0.join("input.pdf"), &bytes).unwrap();
    let request = PdfAnalysisRequest {
        input_ref: "input.pdf".into(),
        input_digest: format!("{:x}", Sha256::digest(&bytes)),
        page_index: None,
    };
    assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &request).is_err());
}

#[test]
fn rejects_recursive_form_resources() {
    use lopdf::{Document, Object};
    let w = Workspace::new();
    let mut doc = Document::load_mem(&fixture::document_bytes()).unwrap();
    let id = *doc
        .objects
        .iter()
        .find(|(_, object)| {
            object.as_stream().ok().is_some_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|v| v.as_name().ok())
                    == Some(b"Form")
            })
        })
        .unwrap()
        .0;
    let Object::Stream(stream) = doc.objects.get_mut(&id).unwrap() else {
        panic!("fixture form")
    };
    stream.dict.set(
        "Resources",
        lopdf::dictionary! {"XObject" => lopdf::dictionary! {"Im1" => id}},
    );
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    fs::write(w.0.join("input.pdf"), &bytes).unwrap();
    let request = PdfAnalysisRequest {
        input_ref: "input.pdf".into(),
        input_digest: format!("{:x}", Sha256::digest(&bytes)),
        page_index: None,
    };
    assert!(analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &request).is_err());
}

#[test]
fn extracts_unicode_text_using_a_pdf_unicode_map() {
    use lopdf::{Document, Object, Stream, dictionary};
    let w = Workspace::new();
    let mut doc = Document::load_mem(&fixture::document_bytes()).unwrap();
    let map = doc.add_object(Stream::new(dictionary! {}, b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Example def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange 2 beginbfchar <0001> <65E5> <0002> <672C> endbfchar endcmap CMapName currentdict /CMap defineresource pop end end".to_vec()));
    let descendant = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>"Example", "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"), "Ordering"=>Object::string_literal("Identity"), "Supplement"=>0}, "DW"=>1000});
    let font = doc.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"Example", "Encoding"=>"Identity-H", "DescendantFonts"=>vec![Object::Reference(descendant)], "ToUnicode"=>map});
    let page = *doc.get_pages().values().next().unwrap();
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 20 50 Td <00010002> Tj ET".to_vec(),
    ));
    let page_dict = doc.get_dictionary_mut(page).unwrap();
    page_dict.set("Resources", dictionary! {"Font"=>dictionary!{"F1"=>font}});
    page_dict.set("Contents", content);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    fs::write(w.0.join("input.pdf"), &bytes).unwrap();
    let request = PdfAnalysisRequest {
        input_ref: "input.pdf".into(),
        input_digest: format!("{:x}", Sha256::digest(&bytes)),
        page_index: Some(0),
    };
    let report = analyze_pdf(&PdfReadCapability::authorize(&w.0).unwrap(), &request).unwrap();
    assert!(report.text.contains("日本"));
}
