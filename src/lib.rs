#![forbid(unsafe_code)]
#![doc = "Bounded local document output with exact-input and atomic-publication checks."]

mod boundary;
mod config;
mod path_policy;
mod plan;
mod render;
mod reports;

pub use boundary::ConnectorError;
pub use config::{ConnectorConfig, MaximumClassification, Renderer, parse_config};
pub use plan::{ConnectorPlan, PlanStep, build_plan};
pub use render::{RenderReceipt, render};
pub use reports::{
    CapabilityReport, DoctorReport, ValidationReport, capabilities, doctor, validation_report,
};

/// Stable connector identity.
pub const CONNECTOR: &str = "zixcel-document-output";
/// Provider family represented by this connector.
pub const PROVIDER: &str = "document-output";
/// Accepted configuration schema.
pub const CONFIG_SCHEMA: &str = "zixcel://document/output/config/v1";
/// Emitted planning contract schema.
pub const PLAN_SCHEMA: &str = "zixcel://document/output/plan/v1";
