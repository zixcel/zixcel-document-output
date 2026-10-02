use std::fs;
use std::process::ExitCode;

use serde::Serialize;
use serde_json::json;
use zixcel_document_output::{
    PdfAnalysisRequest, PdfReadCapability, PdfWriteCapability, analyze_pdf, build_plan,
    capabilities, doctor, parse_config, render, validation_report, write_pdf_report,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let error = json!({
                "schema": "zixcel://document/output/error/v1",
                "connector": zixcel_document_output::CONNECTOR,
                "status": "error",
                "message": message
            });
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&error)
                    .unwrap_or_else(|_| "{\"status\":\"error\"}".to_owned())
            );
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().ok_or_else(usage)?;
    match command.as_str() {
        "pdf-analyze" => {
            let config_path = arguments.next().ok_or_else(usage)?;
            if arguments.next().as_deref() != Some("--read-root") {
                return Err(usage());
            }
            let read_root = arguments.next().ok_or_else(usage)?;
            let output = match arguments.next() {
                None => None,
                Some(flag) if flag == "--output" => {
                    let reference = arguments.next().ok_or_else(usage)?;
                    if arguments.next().as_deref() != Some("--write-root") {
                        return Err(usage());
                    }
                    let root = arguments.next().ok_or_else(usage)?;
                    expect_no_more(&mut arguments)?;
                    Some((reference, root))
                }
                _ => return Err(usage()),
            };
            let request: PdfAnalysisRequest = serde_json::from_str(&read_config(&config_path)?)
                .map_err(|_| "invalid PDF analysis configuration".to_owned())?;
            let read = PdfReadCapability::authorize(std::path::Path::new(&read_root))
                .map_err(|error| error.to_string())?;
            let report = analyze_pdf(&read, &request).map_err(|error| error.to_string())?;
            if let Some((reference, root)) = output {
                let write = PdfWriteCapability::authorize(std::path::Path::new(&root))
                    .map_err(|error| error.to_string())?;
                let digest = write_pdf_report(&write, &reference, &report)
                    .map_err(|error| error.to_string())?;
                print_json(
                    &json!({"schema":"zixcel://document/pdf/report-receipt/v1", "input_digest":report.input_digest,"output_ref":reference,"output_digest":digest,"summary":report.summary}),
                )
            } else {
                print_json(&report)
            }
        }
        "doctor" => {
            expect_no_more(&mut arguments)?;
            print_json(&doctor())
        }
        "capabilities" => {
            expect_no_more(&mut arguments)?;
            print_json(&capabilities())
        }
        "validate" => {
            let path = one_path(&mut arguments)?;
            let source = read_config(&path)?;
            let config = parse_config(&source).map_err(|error| error.to_string())?;
            print_json(&validation_report(&config).map_err(|error| error.to_string())?)
        }
        "plan" => {
            let path = one_path(&mut arguments)?;
            let source = read_config(&path)?;
            let config = parse_config(&source).map_err(|error| error.to_string())?;
            print_json(&build_plan(&config).map_err(|error| error.to_string())?)
        }
        "render" => {
            let path = arguments.next().ok_or_else(usage)?;
            if arguments.next().as_deref() != Some("--root") {
                return Err(usage());
            }
            let root = arguments.next().ok_or_else(usage)?;
            expect_no_more(&mut arguments)?;
            let source = read_config(&path)?;
            let config = parse_config(&source).map_err(|error| error.to_string())?;
            print_json(
                &render(&config, std::path::Path::new(&root)).map_err(|error| error.to_string())?,
            )
        }
        _ => Err(usage()),
    }
}

fn one_path(arguments: &mut impl Iterator<Item = String>) -> Result<String, String> {
    let path = arguments.next().ok_or_else(usage)?;
    expect_no_more(arguments)?;
    Ok(path)
}

fn expect_no_more(arguments: &mut impl Iterator<Item = String>) -> Result<(), String> {
    if arguments.next().is_some() {
        Err(usage())
    } else {
        Ok(())
    }
}

fn read_config(path: &str) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|_| "configuration file could not be read as UTF-8 text".to_owned())
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|_| "JSON serialization failed".to_owned())?
    );
    Ok(())
}

fn usage() -> String {
    "usage: zixcel-document-output <doctor|capabilities|validate <config>|plan <config>|render <config> --root <directory>|pdf-analyze <config> --read-root <directory> [--output <relative.json> --write-root <directory>]>".to_owned()
}
