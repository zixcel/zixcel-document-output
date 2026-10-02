use zixcel_document_output::{build_plan, capabilities, parse_config};

const CONFIG: &str = r#"
schema = "zixcel://document/output/config/v1"
config_id = "management.report"
input_ref = "reports/current.md"
input_digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
output_ref = "output/current.pdf"
maximum_classification = "internal"
renderer = "pandoc_typst"
pandoc_digest = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
typst_digest = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
main_font = "Noto Sans CJK JP"
timeout_seconds = 90
maximum_input_bytes = 8388608
maximum_output_bytes = 67108864
"#;

#[test]
fn plan_is_deterministic_and_effects_are_explicit() {
    let config = parse_config(CONFIG).expect("config");
    let first = build_plan(&config).expect("plan");
    let second = build_plan(&config).expect("plan");
    assert_eq!(first, second);
    assert_eq!(first.steps[0].effect, "read");
    assert_eq!(first.steps[2].effect, "atomic_replace");
    assert!(first.steps.iter().all(|step| !step.network_required));
}

#[test]
fn contract_rejects_escape_unknowns_embedded_keys_and_bad_digests() {
    assert!(parse_config(&CONFIG.replace("reports/current.md", "../../outside")).is_err());
    assert!(parse_config(&format!("{CONFIG}\nfuture = true\n")).is_err());
    assert!(parse_config(&format!("{CONFIG}\nprivate_key = \"no\"\n")).is_err());
    assert!(parse_config(&CONFIG.replace(&"a".repeat(64), "not-a-digest")).is_err());
    assert!(parse_config(&" ".repeat(1_048_577)).is_err());
}

#[test]
fn capability_is_exact_and_provider_neutral() {
    let report = capabilities();
    assert_eq!(
        report.capabilities,
        [
            "document/render/pdf",
            "document/pdf/extract-text",
            "document/pdf/metadata",
            "document/pdf/page-analysis",
            "document/pdf/report-create"
        ]
    );
    assert_eq!(report.provider, "document-output");
}

#[test]
fn template_requires_paired_digest_and_changes_plan() {
    assert!(
        parse_config(&format!(
            "{CONFIG}\ntemplate_ref = \"templates/report.typ\"\n"
        ))
        .is_err()
    );
    let source = format!(
        "{CONFIG}\ntemplate_ref = \"templates/report.typ\"\ntemplate_digest = \"{}\"\n",
        "d".repeat(64)
    );
    let config = parse_config(&source).expect("template config");
    let plan = build_plan(&config).expect("template plan");
    assert_eq!(plan.steps[1].action, "verify_template");
    assert_eq!(plan.steps[3].sequence, 4);
    assert_ne!(
        plan.plan_id,
        build_plan(&parse_config(CONFIG).unwrap()).unwrap().plan_id
    );
    assert!(parse_config(&source.replace("templates/report.typ", "../report.typ")).is_err());
}
