use std::env;
use std::path::Path;

use serde::Serialize;

use crate::{CONNECTOR, ConnectorConfig, ConnectorError, PROVIDER};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub schema: &'static str,
    pub connector: &'static str,
    pub status: &'static str,
    pub network_client_linked: bool,
    pub secret_resolution_enabled: bool,
    pub execution_enabled: bool,
    pub file_writes_enabled: bool,
    pub pandoc_available: bool,
    pub typst_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityReport {
    pub schema: &'static str,
    pub connector: &'static str,
    pub provider: &'static str,
    pub capabilities: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub schema: &'static str,
    pub connector: &'static str,
    pub config_id: String,
    pub valid: bool,
}

#[must_use]
pub fn doctor() -> DoctorReport {
    let pandoc_available = command_available("pandoc");
    let typst_available = command_available("typst");
    DoctorReport {
        schema: "zixcel://document/output/doctor/v1",
        connector: CONNECTOR,
        status: if pandoc_available && typst_available {
            "healthy"
        } else {
            "unavailable"
        },
        network_client_linked: false,
        secret_resolution_enabled: false,
        execution_enabled: pandoc_available && typst_available,
        file_writes_enabled: true,
        pandoc_available,
        typst_available,
    }
}

#[must_use]
pub fn capabilities() -> CapabilityReport {
    CapabilityReport {
        schema: "zixcel://document/output/capabilities/v1",
        connector: CONNECTOR,
        provider: PROVIDER,
        capabilities: vec!["document/render/pdf"],
    }
}

pub fn validation_report(config: &ConnectorConfig) -> Result<ValidationReport, ConnectorError> {
    config.validate()?;
    Ok(ValidationReport {
        schema: "zixcel://document/output/validation/v1",
        connector: CONNECTOR,
        config_id: config.config_id.clone(),
        valid: true,
    })
}

fn command_available(command: &str) -> bool {
    env::var_os("PATH").is_some_and(|value| {
        env::split_paths(&value).any(|path| Path::new(&path).join(command).is_file())
    })
}
