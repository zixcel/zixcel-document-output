use serde::{Deserialize, Serialize};

use crate::boundary::{identifier, reject_secrets};
use crate::path_policy::portable_relative;
use crate::{CONFIG_SCHEMA, ConnectorError};

const DEFAULT_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_MAXIMUM_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const DEFAULT_MAXIMUM_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaximumClassification {
    Public,
    Internal,
    Confidential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Renderer {
    PandocTypst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorConfig {
    pub schema: String,
    pub config_id: String,
    pub input_ref: String,
    pub input_digest: String,
    pub output_ref: String,
    pub maximum_classification: MaximumClassification,
    pub renderer: Renderer,
    pub pandoc_digest: String,
    pub typst_digest: String,
    pub main_font: String,
    #[serde(default)]
    pub template_ref: Option<String>,
    #[serde(default)]
    pub template_digest: Option<String>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_maximum_input_bytes")]
    pub maximum_input_bytes: u64,
    #[serde(default = "default_maximum_output_bytes")]
    pub maximum_output_bytes: u64,
    #[serde(default)]
    pub expected_output_digest: Option<String>,
}

impl ConnectorConfig {
    pub fn validate(&self) -> Result<(), ConnectorError> {
        if self.schema != CONFIG_SCHEMA {
            return Err(ConnectorError::new(
                "schema",
                "expected zixcel://document/output/config/v1",
            ));
        }
        identifier("config_id", &self.config_id)?;
        portable_relative("input_ref", &self.input_ref)?;
        portable_relative("output_ref", &self.output_ref)?;
        if self.input_ref == self.output_ref {
            return Err(ConnectorError::new(
                "output_ref",
                "must differ from input_ref",
            ));
        }
        match (&self.template_ref, &self.template_digest) {
            (Some(path), Some(hash)) => {
                portable_relative("template_ref", path)?;
                digest("template_digest", hash)?;
                if path == &self.output_ref {
                    return Err(ConnectorError::new(
                        "template_ref",
                        "must differ from output_ref",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(ConnectorError::new(
                    "template_ref",
                    "template reference and digest must be supplied together",
                ));
            }
        }
        digest("input_digest", &self.input_digest)?;
        digest("pandoc_digest", &self.pandoc_digest)?;
        digest("typst_digest", &self.typst_digest)?;
        if let Some(value) = &self.expected_output_digest {
            digest("expected_output_digest", value)?;
        }
        if self.main_font.trim().is_empty()
            || self.main_font.len() > 128
            || self.main_font.chars().any(char::is_control)
        {
            return Err(ConnectorError::new(
                "main_font",
                "must be a bounded printable font family",
            ));
        }
        if !(1..=600).contains(&self.timeout_seconds) {
            return Err(ConnectorError::new(
                "timeout_seconds",
                "must be between 1 and 600",
            ));
        }
        if !(1..=64 * 1024 * 1024).contains(&self.maximum_input_bytes) {
            return Err(ConnectorError::new(
                "maximum_input_bytes",
                "must be between 1 byte and 64 MiB",
            ));
        }
        if !(1..=512 * 1024 * 1024).contains(&self.maximum_output_bytes) {
            return Err(ConnectorError::new(
                "maximum_output_bytes",
                "must be between 1 byte and 512 MiB",
            ));
        }
        Ok(())
    }
}

fn digest(field: &'static str, value: &str) -> Result<(), ConnectorError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ConnectorError::new(
            field,
            "must be a lowercase SHA-256 digest",
        ));
    }
    Ok(())
}

const fn default_timeout_seconds() -> u64 {
    DEFAULT_TIMEOUT_SECONDS
}

const fn default_maximum_input_bytes() -> u64 {
    DEFAULT_MAXIMUM_INPUT_BYTES
}

const fn default_maximum_output_bytes() -> u64 {
    DEFAULT_MAXIMUM_OUTPUT_BYTES
}

pub fn parse_config(source: &str) -> Result<ConnectorConfig, ConnectorError> {
    reject_secrets(source)?;
    let config: ConnectorConfig = toml::from_str(source).map_err(|_| {
        ConnectorError::new(
            "config",
            "invalid TOML or fields do not match the document output schema",
        )
    })?;
    config.validate()?;
    Ok(config)
}
