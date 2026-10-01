use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{CONNECTOR, ConnectorConfig, ConnectorError, PLAN_SCHEMA, PROVIDER};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanStep {
    pub sequence: u32,
    pub action: &'static str,
    pub target: String,
    pub effect: &'static str,
    pub network_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectorPlan {
    pub schema: &'static str,
    pub plan_id: String,
    pub request_id: String,
    pub provider: &'static str,
    pub connector: &'static str,
    pub mode: &'static str,
    pub steps: Vec<PlanStep>,
}

pub fn build_plan(config: &ConnectorConfig) -> Result<ConnectorPlan, ConnectorError> {
    config.validate()?;
    let mut steps = vec![
        PlanStep {
            sequence: 1,
            action: "verify_input",
            target: config.input_ref.clone(),
            effect: "read",
            network_required: false,
        },
        PlanStep {
            sequence: 2,
            action: "render_pdf",
            target: config.output_ref.clone(),
            effect: "temporary_write",
            network_required: false,
        },
        PlanStep {
            sequence: 3,
            action: "publish_output",
            target: config.output_ref.clone(),
            effect: "atomic_replace",
            network_required: false,
        },
    ];
    if let Some(reference) = &config.template_ref {
        steps.insert(
            1,
            PlanStep {
                sequence: 2,
                action: "verify_template",
                target: reference.clone(),
                effect: "read",
                network_required: false,
            },
        );
        for (index, step) in steps.iter_mut().enumerate() {
            step.sequence = (index + 1) as u32;
        }
    }
    let bytes = serde_json::to_vec(&(config, &steps))
        .map_err(|_| ConnectorError::new("plan", "could not serialize canonical plan"))?;
    let digest = Sha256::digest(bytes);
    Ok(ConnectorPlan {
        schema: PLAN_SCHEMA,
        plan_id: format!("document-output-{}", &hex(&digest)[..16]),
        request_id: format!("request-{}", config.config_id),
        provider: PROVIDER,
        connector: CONNECTOR,
        mode: "local_execute",
        steps,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
