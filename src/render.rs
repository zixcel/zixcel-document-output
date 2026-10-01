use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, ffi::OsString};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::path_policy::{existing_file, output_file};
use crate::{CONNECTOR, ConnectorConfig, ConnectorError, Renderer};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderReceipt {
    pub schema: &'static str,
    pub connector: &'static str,
    pub config_id: String,
    pub input_digest: String,
    pub output_digest: String,
    pub renderer: &'static str,
    pub output_bytes: u64,
    pub changed: bool,
}

pub fn render(config: &ConnectorConfig, root: &Path) -> Result<RenderReceipt, ConnectorError> {
    config.validate()?;
    let root = root
        .canonicalize()
        .map_err(|_| ConnectorError::new("root", "root directory does not exist"))?;
    if !root.is_dir() {
        return Err(ConnectorError::new("root", "must be a directory"));
    }
    let input = existing_file(&root, "input_ref", &config.input_ref)?;
    let input_metadata = input
        .metadata()
        .map_err(|_| ConnectorError::new("input_ref", "metadata could not be read"))?;
    if input_metadata.len() > config.maximum_input_bytes {
        return Err(ConnectorError::new(
            "input_ref",
            "input exceeds maximum_input_bytes",
        ));
    }
    let input_digest = file_digest(&input, config.maximum_input_bytes, "input_ref")?;
    if input_digest != config.input_digest {
        return Err(ConnectorError::new(
            "input_digest",
            "does not match the input file",
        ));
    }
    let template = match (&config.template_ref, &config.template_digest) {
        (Some(reference), Some(expected)) => {
            let path = existing_file(&root, "template_ref", reference)?;
            if file_digest(&path, 1024 * 1024, "template_ref")? != *expected {
                return Err(ConnectorError::new(
                    "template_digest",
                    "does not match the template file",
                ));
            }
            Some(path)
        }
        _ => None,
    };
    let output = output_file(&root, "output_ref", &config.output_ref)?;
    let existing_digest = if output.exists() {
        Some(file_digest(
            &output,
            config.maximum_output_bytes,
            "output_ref",
        )?)
    } else {
        None
    };
    match (&existing_digest, &config.expected_output_digest) {
        (Some(actual), Some(expected)) if actual != expected => {
            return Err(ConnectorError::new(
                "expected_output_digest",
                "does not match the existing output",
            ));
        }
        (None, Some(_)) => {
            return Err(ConnectorError::new(
                "expected_output_digest",
                "was supplied but the output does not exist",
            ));
        }
        _ => {}
    }
    let temporary = temporary_path(&output)?;
    if let Err(error) = render_temporary(config, &input, &temporary, template.as_deref()) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let result = publish_temporary(
        config,
        &temporary,
        &output,
        existing_digest.as_deref(),
        input_digest,
    );
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publish_temporary(
    config: &ConnectorConfig,
    temporary: &Path,
    output: &Path,
    existing_digest: Option<&str>,
    input_digest: String,
) -> Result<RenderReceipt, ConnectorError> {
    let metadata = temporary
        .metadata()
        .map_err(|_| ConnectorError::new("renderer", "did not produce an output file"))?;
    if !metadata.is_file() || metadata.len() > config.maximum_output_bytes {
        return Err(ConnectorError::new(
            "renderer",
            "output is not a bounded regular file",
        ));
    }
    let mut header = [0_u8; 5];
    File::open(temporary)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|_| ConnectorError::new("renderer", "output header could not be read"))?;
    if &header != b"%PDF-" {
        return Err(ConnectorError::new(
            "renderer",
            "output is not a PDF document",
        ));
    }
    let output_digest = file_digest(temporary, config.maximum_output_bytes, "renderer")?;
    if existing_digest == Some(&output_digest) {
        fs::remove_file(temporary).map_err(|_| {
            ConnectorError::new("output_ref", "temporary output could not be removed")
        })?;
        return Ok(receipt(
            config,
            input_digest,
            output_digest,
            metadata.len(),
            false,
        ));
    }
    if existing_digest.is_some() && config.expected_output_digest.is_none() {
        return Err(ConnectorError::new(
            "expected_output_digest",
            "is required to replace a different existing output",
        ));
    }
    File::open(temporary)
        .and_then(|file| file.sync_all())
        .map_err(|_| {
            ConnectorError::new("output_ref", "temporary output could not be synchronized")
        })?;
    fs::rename(temporary, output)
        .map_err(|_| ConnectorError::new("output_ref", "atomic output publication failed"))?;
    File::open(output)
        .and_then(|file| file.sync_all())
        .map_err(|_| {
            ConnectorError::new("output_ref", "published output could not be synchronized")
        })?;
    Ok(receipt(
        config,
        input_digest,
        output_digest,
        metadata.len(),
        true,
    ))
}

fn render_temporary(
    config: &ConnectorConfig,
    input: &Path,
    output: &Path,
    template: Option<&Path>,
) -> Result<(), ConnectorError> {
    match config.renderer {
        Renderer::PandocTypst => {
            let pandoc = verified_executable("pandoc", &config.pandoc_digest)?;
            let typst = verified_executable("typst", &config.typst_digest)?;
            let path = executable_path(&typst)?;
            let mut command = Command::new(pandoc);
            if let Some(template) = template {
                command.arg("--template").arg(template);
            }
            let mut child = command
                .arg(input)
                .args(["--from", "gfm", "--pdf-engine", "typst", "-V"])
                .arg(format!("mainfont={}", config.main_font))
                .arg("-o")
                .arg(output)
                // Remove wall-clock variation from otherwise identical retries.
                .env("SOURCE_DATE_EPOCH", "946684800")
                // Make the already verified Typst executable the first lookup result.
                .env("PATH", path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| ConnectorError::new("renderer", "pandoc could not be started"))?;
            let deadline = Instant::now() + Duration::from_secs(config.timeout_seconds);
            loop {
                match child.try_wait().map_err(|_| {
                    ConnectorError::new("renderer", "renderer state could not be read")
                })? {
                    Some(status) if status.success() => return Ok(()),
                    Some(_) => {
                        return Err(ConnectorError::new(
                            "renderer",
                            "pandoc or typst rejected the document",
                        ));
                    }
                    None if Instant::now() >= deadline => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(ConnectorError::new(
                            "renderer",
                            "rendering exceeded timeout_seconds",
                        ));
                    }
                    None => thread::sleep(Duration::from_millis(20)),
                }
            }
        }
    }
}

fn verified_executable(
    command: &'static str,
    expected_digest: &str,
) -> Result<PathBuf, ConnectorError> {
    let candidate = env::var_os("PATH")
        .and_then(|value| {
            env::split_paths(&value)
                .map(|path| path.join(command))
                .find(|path| path.is_file())
        })
        .ok_or_else(|| ConnectorError::new("renderer", format!("{command} is not available")))?;
    let canonical = candidate
        .canonicalize()
        .map_err(|_| ConnectorError::new("renderer", format!("{command} could not be resolved")))?;
    let actual = file_digest(&canonical, 512 * 1024 * 1024, "renderer")?;
    if actual != expected_digest {
        return Err(ConnectorError::new(
            "renderer",
            format!("{command} digest does not match the admitted executable"),
        ));
    }
    Ok(canonical)
}

fn executable_path(typst: &Path) -> Result<OsString, ConnectorError> {
    let directory = typst
        .parent()
        .ok_or_else(|| ConnectorError::new("renderer", "typst has no parent directory"))?;
    let mut paths = vec![directory.to_path_buf()];
    if let Some(value) = env::var_os("PATH") {
        paths.extend(env::split_paths(&value));
    }
    env::join_paths(paths)
        .map_err(|_| ConnectorError::new("renderer", "renderer PATH could not be constructed"))
}

fn temporary_path(output: &Path) -> Result<PathBuf, ConnectorError> {
    let parent = output
        .parent()
        .ok_or_else(|| ConnectorError::new("output_ref", "must have a parent directory"))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ConnectorError::new("output_ref", "file name must be UTF-8"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ConnectorError::new("output_ref", "system clock is invalid"))?
        .as_nanos();
    // Keep the PDF suffix because Pandoc selects the final writer from the output extension.
    let path = parent.join(format!(".{name}.{}.{}.tmp.pdf", std::process::id(), nonce));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|file| file.sync_all())
        .map_err(|_| ConnectorError::new("output_ref", "temporary output could not be reserved"))?;
    fs::remove_file(&path).map_err(|_| {
        ConnectorError::new(
            "output_ref",
            "temporary output reservation could not be released",
        )
    })?;
    Ok(path)
}

fn file_digest(path: &Path, maximum: u64, field: &'static str) -> Result<String, ConnectorError> {
    let file =
        File::open(path).map_err(|_| ConnectorError::new(field, "file could not be opened"))?;
    let mut reader = BufReader::new(file.take(maximum.saturating_add(1)));
    let mut digest = Sha256::new();
    let copied = std::io::copy(&mut reader, &mut digest)
        .map_err(|_| ConnectorError::new(field, "file could not be hashed"))?;
    if copied > maximum {
        return Err(ConnectorError::new(
            field,
            "file exceeds its configured byte limit",
        ));
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn receipt(
    config: &ConnectorConfig,
    input_digest: String,
    output_digest: String,
    output_bytes: u64,
    changed: bool,
) -> RenderReceipt {
    RenderReceipt {
        schema: "zixcel://document/output/receipt/v1",
        connector: CONNECTOR,
        config_id: config.config_id.clone(),
        input_digest,
        output_digest,
        renderer: "pandoc_typst",
        output_bytes,
        changed,
    }
}
