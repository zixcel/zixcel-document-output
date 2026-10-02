//! PDF inspection has a read capability; publishing its JSON report requires a separate write capability.
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use lopdf::content::Content;
use lopdf::{Dictionary, Document, LoadOptions, Object, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ConnectorError;
use crate::path_policy::{existing_file, output_file, portable_relative};

const INPUT_LIMIT: usize = 8 * 1024 * 1024;
const REPORT_LIMIT: usize = 64 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Authority to inspect PDF files beneath one existing directory. It confers no write methods.
#[derive(Debug)]
pub struct PdfReadCapability {
    root: PathBuf,
}
/// Authority to create reports beneath a separately selected directory.
#[derive(Debug)]
pub struct PdfWriteCapability {
    root: PathBuf,
}

fn root_directory(root: &Path) -> Result<PathBuf, ConnectorError> {
    let root = root
        .canonicalize()
        .map_err(|_| ConnectorError::new("root", "directory does not exist"))?;
    if !root.is_dir() {
        return Err(ConnectorError::new("root", "must be a directory"));
    }
    Ok(root)
}
impl PdfReadCapability {
    pub fn authorize(root: &Path) -> Result<Self, ConnectorError> {
        Ok(Self {
            root: root_directory(root)?,
        })
    }
}
impl PdfWriteCapability {
    pub fn authorize(root: &Path) -> Result<Self, ConnectorError> {
        Ok(Self {
            root: root_directory(root)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PdfAnalysisRequest {
    pub input_ref: String,
    pub input_digest: String,
    /// Zero-based page selection; None extracts all pages. Analysis always covers the entire document.
    #[serde(default)]
    pub page_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PdfPageAnalysis {
    pub page: u32,
    pub has_text: bool,
    pub has_image: bool,
    pub is_image_only: bool,
    pub is_blank: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PdfSummary {
    pub total_pages: usize,
    pub text_pages: usize,
    pub image_include_pages: usize,
    pub image_only_pages: usize,
    pub blank_pages: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PdfAnalysisReport {
    pub schema: String,
    pub input_digest: String,
    pub text: String,
    pub metadata: BTreeMap<String, String>,
    pub pages: Vec<PdfPageAnalysis>,
    pub summary: PdfSummary,
}

pub fn analyze_pdf(
    read: &PdfReadCapability,
    request: &PdfAnalysisRequest,
) -> Result<PdfAnalysisReport, ConnectorError> {
    portable_relative("input_ref", &request.input_ref)?;
    if request.input_digest.len() != 64
        || !request
            .input_digest
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(ConnectorError::new(
            "input_digest",
            "must be a lowercase SHA-256 digest",
        ));
    }
    let input = existing_file(&read.root, "input_ref", &request.input_ref)?;
    let mut bytes = Vec::new();
    fs::File::open(input)
        .map_err(|_| ConnectorError::new("input_ref", "cannot open input"))?
        .take((INPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ConnectorError::new("input_ref", "cannot read input"))?;
    if bytes.len() > INPUT_LIMIT {
        return Err(ConnectorError::new("input_ref", "input exceeds 8 MiB"));
    }
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if digest != request.input_digest {
        return Err(ConnectorError::new(
            "input_digest",
            "does not match the input snapshot",
        ));
    }
    let doc = Document::load_mem_with_options(
        &bytes,
        LoadOptions::with_max_decompressed_size(INPUT_LIMIT),
    )
    .map_err(|_| ConnectorError::new("pdf", "invalid, encrypted, or oversized PDF stream"))?;
    if doc.was_encrypted() {
        return Err(ConnectorError::new(
            "pdf",
            "encrypted PDFs require a separate authorized decryption workflow",
        ));
    }
    let pages = doc.get_pages();
    if pages.is_empty() || pages.len() > 1000 {
        return Err(ConnectorError::new("pdf", "must contain 1 to 1000 pages"));
    }
    if request.page_index.is_some_and(|index| index >= pages.len()) {
        return Err(ConnectorError::new("page_index", "outside the document"));
    }
    let mut report = PdfAnalysisReport {
        schema: "zixcel://document/pdf/analysis/v1".into(),
        input_digest: digest,
        text: String::new(),
        metadata: metadata(&doc),
        pages: Vec::new(),
        summary: PdfSummary::default(),
    };
    for (index, (&number, &id)) in pages.iter().enumerate() {
        let text = doc
            .extract_text_with_limit(&[number], INPUT_LIMIT)
            .map_err(|_| {
                ConnectorError::new(
                    "pdf",
                    "page text could not be decoded within its stream limit",
                )
            })?;
        let has_text = !text.trim().is_empty();
        let (direct, inherited) = doc
            .get_page_resources(id)
            .map_err(|_| ConnectorError::new("pdf", "invalid page resources"))?;
        let mut resources = Vec::new();
        if let Some(dictionary) = direct {
            resources.push(dictionary);
        }
        for resource in inherited {
            resources.push(
                doc.get_dictionary(resource)
                    .map_err(|_| ConnectorError::new("pdf", "invalid inherited resources"))?,
            );
        }
        let content = doc
            .get_page_content_with_limit(id, INPUT_LIMIT)
            .map_err(|_| ConnectorError::new("pdf", "page stream exceeds its limit"))?;
        let has_image = images_in_content(&doc, &content, &resources, &mut HashSet::new(), 0)?;
        let image_only = !has_text && has_image;
        let blank = !has_text && !has_image;
        report.summary.total_pages += 1;
        report.summary.text_pages += usize::from(has_text);
        report.summary.image_include_pages += usize::from(has_image);
        report.summary.image_only_pages += usize::from(image_only);
        report.summary.blank_pages += usize::from(blank);
        report.pages.push(PdfPageAnalysis {
            page: number,
            has_text,
            has_image,
            is_image_only: image_only,
            is_blank: blank,
        });
        if request.page_index.is_none_or(|selected| selected == index) {
            report.text.push_str(&text);
        }
        if report.text.len() > REPORT_LIMIT {
            return Err(ConnectorError::new(
                "report",
                "extracted text exceeds 64 MiB",
            ));
        }
    }
    if serde_json::to_vec(&report)
        .map_err(|_| ConnectorError::new("report", "serialization failed"))?
        .len()
        > REPORT_LIMIT
    {
        return Err(ConnectorError::new("report", "report exceeds 64 MiB"));
    }
    Ok(report)
}

fn images_in_content(
    doc: &Document,
    bytes: &[u8],
    resources: &[&Dictionary],
    visited: &mut HashSet<ObjectId>,
    depth: usize,
) -> Result<bool, ConnectorError> {
    if depth > 32 {
        return Err(ConnectorError::new("pdf", "form nesting exceeds its limit"));
    }
    let content =
        Content::decode(bytes).map_err(|_| ConnectorError::new("pdf", "invalid page content"))?;
    for operation in content.operations {
        if operation.operator == "BI" {
            return Ok(true);
        }
        if operation.operator != "Do" {
            continue;
        }
        let name = operation
            .operands
            .first()
            .and_then(|value| value.as_name().ok())
            .ok_or_else(|| ConnectorError::new("pdf", "invalid image or form invocation"))?;
        let object = resources.iter().find_map(|dictionary| {
            doc.get_dict_in_dict(dictionary, b"XObject")
                .ok()?
                .get(name)
                .ok()
        });
        let Some(object) = object else {
            return Err(ConnectorError::new(
                "pdf",
                "unresolved image or form resource",
            ));
        };
        let (id, value) = doc
            .dereference(object)
            .map_err(|_| ConnectorError::new("pdf", "invalid image reference"))?;
        let stream = value
            .as_stream()
            .map_err(|_| ConnectorError::new("pdf", "image or form must be a stream"))?;
        let subtype = stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .map_err(|_| ConnectorError::new("pdf", "missing object subtype"))?;
        if subtype == b"Image" {
            return Ok(true);
        }
        if subtype == b"Form" {
            if id.is_some_and(|id| !visited.insert(id)) {
                return Err(ConnectorError::new("pdf", "cyclic form reference"));
            }
            let owned = doc.get_dict_in_dict(&stream.dict, b"Resources").ok();
            let next = owned.map_or_else(|| resources.to_vec(), |dictionary| vec![dictionary]);
            let decoded = stream
                .decompressed_content_with_limit(INPUT_LIMIT)
                .map_err(|_| ConnectorError::new("pdf", "form exceeds its stream limit"))?;
            let found = images_in_content(doc, &decoded, &next, visited, depth + 1)?;
            if let Some(id) = id {
                visited.remove(&id);
            }
            if found {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn metadata(doc: &Document) -> BTreeMap<String, String> {
    let mut output = BTreeMap::new();
    if let Ok(info) = doc
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .and_then(|id| doc.get_dictionary(id))
    {
        for (key, value) in info.iter() {
            let value = doc
                .dereference(value)
                .map(|(_, value)| value)
                .unwrap_or(value);
            let text = match value {
                Object::String(bytes, _) => decode_string(bytes),
                Object::Name(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                Object::Integer(value) => value.to_string(),
                Object::Real(value) => value.to_string(),
                Object::Boolean(value) => value.to_string(),
                _ => continue,
            };
            let key = String::from_utf8_lossy(key).into_owned();
            let text = if key.contains("Date") {
                pdf_date(&text).unwrap_or(text)
            } else {
                text
            };
            output.insert(key, text);
        }
    }
    output
}
fn decode_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xfe, 0xff]) || bytes.starts_with(&[0xff, 0xfe]) {
        let big_endian = bytes[0] == 0xfe;
        let units: Vec<_> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| {
                if big_endian {
                    u16::from_be_bytes([pair[0], pair[1]])
                } else {
                    u16::from_le_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8(bytes.to_vec())
        .unwrap_or_else(|_| bytes.iter().map(|byte| char::from(*byte)).collect())
}
fn pdf_date(value: &str) -> Option<String> {
    let raw = value.strip_prefix("D:")?;
    if raw.len() < 14 || !raw.as_bytes()[..14].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let year: u32 = raw[0..4].parse().ok()?;
    let month: usize = raw[4..6].parse().ok()?;
    let day: u32 = raw[6..8].parse().ok()?;
    let hour: u32 = raw[8..10].parse().ok()?;
    let minute: u32 = raw[10..12].parse().ok()?;
    let second: u32 = raw[12..14].parse().ok()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days[month - 1]
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let suffix = &raw[14..];
    let timezone = if suffix.starts_with('Z') {
        "+00:00".into()
    } else if suffix.starts_with(['+', '-']) {
        let zone = suffix.replace('\'', "");
        if zone.len() != 5
            || !zone.as_bytes()[1..].iter().all(u8::is_ascii_digit)
            || zone[1..3].parse::<u32>().ok()? > 23
            || zone[3..5].parse::<u32>().ok()? > 59
        {
            return None;
        }
        format!("{}{}:{}", &zone[..1], &zone[1..3], &zone[3..5])
    } else if suffix.is_empty() {
        String::new()
    } else {
        return None;
    };
    Some(format!(
        "{}-{}-{}T{}:{}:{}{}",
        &raw[..4],
        &raw[4..6],
        &raw[6..8],
        &raw[8..10],
        &raw[10..12],
        &raw[12..14],
        timezone
    ))
}

/// Create a JSON report without replacing an existing file or modifying the source PDF.
pub fn write_pdf_report(
    write: &PdfWriteCapability,
    output_ref: &str,
    report: &PdfAnalysisReport,
) -> Result<String, ConnectorError> {
    portable_relative("output_ref", output_ref)?;
    let output = output_file(&write.root, "output_ref", output_ref)?;
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|_| ConnectorError::new("report", "serialization failed"))?;
    if bytes.len() > REPORT_LIMIT {
        return Err(ConnectorError::new("report", "report exceeds 64 MiB"));
    }
    let temporary = output.with_file_name(format!(
        ".pdf-report-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut created = false;
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| ConnectorError::new("report", "cannot create temporary output"))?;
        created = true;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| ConnectorError::new("report", "cannot persist report"))?;
        fs::hard_link(&temporary, &output).map_err(|_| {
            ConnectorError::new(
                "output_ref",
                "destination already exists or cannot be published atomically",
            )
        })?;
        Ok(format!("{:x}", Sha256::digest(&bytes)))
    })();
    if created {
        let _ = fs::remove_file(temporary);
    }
    result
}
