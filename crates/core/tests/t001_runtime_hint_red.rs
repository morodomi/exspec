use exspec_core::hints::{compute_hints, Hint};
use exspec_core::metrics::ProjectMetrics;
use exspec_core::output::{format_json, format_terminal};
use exspec_core::rules::{Diagnostic, RuleId, Severity};

fn make_diag(rule: &str, severity: Severity, line: usize) -> Diagnostic {
    Diagnostic {
        rule: RuleId::new(rule),
        severity,
        file: "tests/sample_test.rs".to_string(),
        line: Some(line),
        message: format!("{rule} diagnostic"),
        details: None,
    }
}

fn t001_blocks(count: usize) -> Vec<Diagnostic> {
    (0..count)
        .map(|idx| make_diag("T001", Severity::Block, idx + 1))
        .collect()
}

fn sample_hint() -> Hint {
    Hint {
        rule: RuleId::new("T001"),
        title: "Assertion helper patterns may be missing".to_string(),
        message: "Add [assertions] custom_patterns to .exspec.toml.".to_string(),
    }
}

#[test]
fn compute_hints_triggers_at_threshold() {
    let hints = compute_hints(&t001_blocks(10), true);
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].rule, RuleId::new("T001"));
    assert!(
        hints[0].message.contains("custom_patterns"),
        "hint message should reference custom_patterns config key"
    );
}

#[test]
fn compute_hints_below_threshold_no_hint() {
    let hints = compute_hints(&t001_blocks(9), true);
    assert!(hints.is_empty());
}

#[test]
fn compute_hints_with_custom_patterns_no_hint() {
    let hints = compute_hints(&t001_blocks(10), false);
    assert!(hints.is_empty());
}

#[test]
fn compute_hints_mixed_rules_only_counts_t001() {
    let mut diags = t001_blocks(5);
    diags.extend((0..5).map(|idx| make_diag("T003", Severity::Warn, idx + 20)));
    let hints = compute_hints(&diags, true);
    assert!(hints.is_empty());
}

#[test]
fn compute_hints_t001_warn_not_counted() {
    let diags: Vec<_> = (0..10)
        .map(|idx| make_diag("T001", Severity::Warn, idx + 1))
        .collect();
    let hints = compute_hints(&diags, true);
    assert!(hints.is_empty());
}

// TC-DISCOVERED: 9 T001 BLOCKs + 1 T001 WARN = only 9 BLOCKs counted → no hint
#[test]
fn compute_hints_block_warn_mix_below_threshold() {
    let mut diags = t001_blocks(9);
    diags.push(make_diag("T001", Severity::Warn, 100));
    let hints = compute_hints(&diags, true);
    assert!(
        hints.is_empty(),
        "T001 WARNs should not contribute to BLOCK threshold"
    );
}

#[test]
fn terminal_hint_shown_after_score() {
    let output = format_terminal(
        &t001_blocks(1),
        1,
        1,
        &ProjectMetrics::default(),
        &[sample_hint()],
    );
    let score_pos = output.find("Score:").expect("score line missing");
    let hint_pos = output.find("Hint [T001]").expect("hint line missing");
    assert!(score_pos < hint_pos, "hint should be rendered after score");
}

#[test]
fn terminal_no_hint_when_empty() {
    let output = format_terminal(&[], 1, 1, &ProjectMetrics::default(), &[]);
    assert!(!output.contains("Hint ["));
}

#[test]
fn json_hints_array_present() {
    let output = format_json(
        &[],
        1,
        1,
        &ProjectMetrics::default(),
        None,
        &[sample_hint()],
    );
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed["hints"].is_array());
    assert_eq!(parsed["hints"][0]["rule"], "T001");
}

#[test]
fn json_no_hints_key_when_empty() {
    let output = format_json(&[], 1, 1, &ProjectMetrics::default(), None, &[]);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.get("hints").is_none());
}
