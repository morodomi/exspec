use std::collections::HashSet;

use crate::metrics::ProjectMetrics;
use crate::rules::{Diagnostic, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Terminal,
    Json,
    Sarif,
}

/// Count unique violated functions by (file, line) pairs.
/// Only per-function diagnostics (line=Some) are counted.
fn count_violated_functions(diagnostics: &[Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter_map(|d| d.line.map(|l| (d.file.as_str(), l)))
        .collect::<HashSet<_>>()
        .len()
}

pub fn format_terminal(
    diagnostics: &[Diagnostic],
    file_count: usize,
    function_count: usize,
    metrics: &ProjectMetrics,
) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "exspec v{} -- {} test files, {} test functions",
        env!("CARGO_PKG_VERSION"),
        file_count,
        function_count,
    ));

    if file_count == 0 {
        lines.push("No test files found. Check --lang filter or run from a directory containing test files.".to_string());
    }

    for d in diagnostics {
        let line_str = d.line.map(|l| format!(":{l}")).unwrap_or_default();
        lines.push(format!(
            "{} {}{} {} {}",
            d.severity, d.file, line_str, d.rule, d.message,
        ));
    }

    // Metrics section
    lines.push("Metrics:".to_string());
    lines.push(format!(
        "  Mock density:      {:.1}/test (avg), {} distinct classes/test (max)",
        metrics.mock_density_avg, metrics.mock_class_max,
    ));

    let total_functions_for_param = if function_count > 0 {
        let count = (metrics.parameterized_ratio * function_count as f64).round() as usize;
        format!("{count}/{function_count}")
    } else {
        "0/0".to_string()
    };
    lines.push(format!(
        "  Parameterized:     {:.0}% ({})",
        metrics.parameterized_ratio * 100.0,
        total_functions_for_param,
    ));

    let pbt_files = (metrics.pbt_ratio * file_count as f64).round() as usize;
    lines.push(format!(
        "  PBT usage:         {:.0}% ({}/{} files)",
        metrics.pbt_ratio * 100.0,
        pbt_files,
        file_count,
    ));

    lines.push(format!(
        "  Assertion density: {:.1}/test (avg)",
        metrics.assertion_density_avg,
    ));

    let contract_files = (metrics.contract_coverage * file_count as f64).round() as usize;
    lines.push(format!(
        "  Contract coverage: {:.0}% ({}/{} files)",
        metrics.contract_coverage * 100.0,
        contract_files,
        file_count,
    ));

    // Score section
    let block_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Block)
        .count();
    let warn_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warn)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count();
    let violated = count_violated_functions(diagnostics);
    let pass_count = function_count.saturating_sub(violated);
    lines.push(format!(
        "Score: BLOCK {block_count} | WARN {warn_count} | INFO {info_count} | PASS {pass_count}",
    ));

    lines.join("\n")
}

pub fn format_json(
    diagnostics: &[Diagnostic],
    file_count: usize,
    function_count: usize,
    metrics: &ProjectMetrics,
) -> String {
    let block_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Block)
        .count();
    let warn_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warn)
        .count();
    let info_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Info)
        .count();
    let violated = count_violated_functions(diagnostics);
    let pass_count = function_count.saturating_sub(violated);

    let mut output = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "summary": {
            "files": file_count,
            "functions": function_count,
            "block": block_count,
            "warn": warn_count,
            "info": info_count,
            "pass": pass_count,
        },
        "diagnostics": diagnostics,
        "metrics": serde_json::to_value(metrics).unwrap_or_default(),
    });

    if file_count == 0 {
        output["guidance"] = serde_json::json!("No test files found. Check --lang filter or run from a directory containing test files.");
    }
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

struct RuleMeta {
    id: &'static str,
    name: &'static str,
    short_description: &'static str,
}

const RULE_REGISTRY: &[RuleMeta] = &[
    RuleMeta {
        id: "T001",
        name: "assertion-free",
        short_description: "Test function has no assertions",
    },
    RuleMeta {
        id: "T002",
        name: "mock-overuse",
        short_description: "Test function uses too many mocks",
    },
    RuleMeta {
        id: "T003",
        name: "giant-test",
        short_description: "Test function exceeds line count threshold",
    },
    RuleMeta {
        id: "T004",
        name: "no-parameterized",
        short_description: "Low ratio of parameterized tests",
    },
    RuleMeta {
        id: "T005",
        name: "pbt-missing",
        short_description: "No property-based testing library imported",
    },
    RuleMeta {
        id: "T006",
        name: "low-assertion-density",
        short_description: "Low assertion count per test function",
    },
    RuleMeta {
        id: "T007",
        name: "test-source-ratio",
        short_description: "Test file to source file ratio",
    },
    RuleMeta {
        id: "T008",
        name: "no-contract",
        short_description: "No contract testing library used in tests",
    },
    RuleMeta {
        id: "T101",
        name: "how-not-what",
        short_description: "Test verifies implementation rather than behavior",
    },
    RuleMeta {
        id: "T102",
        name: "fixture-sprawl",
        short_description: "Test depends on too many fixtures",
    },
    RuleMeta {
        id: "T103",
        name: "missing-error-test",
        short_description: "No error/exception test found in file",
    },
    RuleMeta {
        id: "T105",
        name: "deterministic-no-metamorphic",
        short_description: "All assertions use exact equality, no relational checks",
    },
    RuleMeta {
        id: "T106",
        name: "duplicate-literal-assertion",
        short_description: "Same literal appears multiple times in assertions",
    },
    RuleMeta {
        id: "T107",
        name: "assertion-roulette",
        short_description: "Multiple assertions without failure messages",
    },
    RuleMeta {
        id: "T108",
        name: "wait-and-see",
        short_description: "Test uses sleep/delay causing flaky tests",
    },
    RuleMeta {
        id: "T109",
        name: "undescriptive-test-name",
        short_description: "Test name does not describe behavior",
    },
];

pub fn format_sarif(diagnostics: &[Diagnostic]) -> String {
    use serde_sarif::sarif;

    let rules: Vec<sarif::ReportingDescriptor> = RULE_REGISTRY
        .iter()
        .map(|r| {
            sarif::ReportingDescriptor::builder()
                .id(r.id)
                .name(r.name)
                .short_description(&String::from(r.short_description))
                .build()
        })
        .collect();

    let results: Vec<sarif::Result> = diagnostics
        .iter()
        .map(|d| {
            let level = match d.severity {
                Severity::Block => sarif::ResultLevel::Error,
                Severity::Warn => sarif::ResultLevel::Warning,
                Severity::Info => sarif::ResultLevel::Note,
            };
            let start_line = d.line.unwrap_or(1) as i64;
            let location = sarif::Location::builder()
                .physical_location(
                    sarif::PhysicalLocation::builder()
                        .artifact_location(sarif::ArtifactLocation::builder().uri(&d.file).build())
                        .region(sarif::Region::builder().start_line(start_line).build())
                        .build(),
                )
                .build();

            sarif::Result::builder()
                .rule_id(&d.rule.0)
                .message(sarif::Message::builder().text(&d.message).build())
                .level(level)
                .locations(vec![location])
                .build()
        })
        .collect();

    let tool_component = sarif::ToolComponent::builder()
        .name("exspec")
        .version(env!("CARGO_PKG_VERSION"))
        .rules(rules)
        .build();

    let invocation = sarif::Invocation::builder()
        .execution_successful(true)
        .build();

    let run = sarif::Run::builder()
        .tool(tool_component)
        .results(results)
        .invocations(vec![invocation])
        .build();

    let sarif_doc = sarif::Sarif::builder()
        .version(sarif::Version::V2_1_0.to_string())
        .schema(sarif::SCHEMA_URL)
        .runs(vec![run])
        .build();

    serde_json::to_string_pretty(&sarif_doc).unwrap_or_else(|_| "{}".to_string())
}

pub fn compute_exit_code(diagnostics: &[Diagnostic], strict: bool) -> i32 {
    for d in diagnostics {
        if d.severity == Severity::Block {
            return 1;
        }
    }
    if strict {
        for d in diagnostics {
            if d.severity == Severity::Warn {
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleId;

    fn block_diag() -> Diagnostic {
        Diagnostic {
            rule: RuleId::new("T001"),
            severity: Severity::Block,
            file: "test.py".to_string(),
            line: Some(10),
            message: "assertion-free: test has no assertions".to_string(),
            details: None,
        }
    }

    fn warn_diag() -> Diagnostic {
        Diagnostic {
            rule: RuleId::new("T003"),
            severity: Severity::Warn,
            file: "test.py".to_string(),
            line: Some(5),
            message: "giant-test: 73 lines, threshold: 50".to_string(),
            details: None,
        }
    }

    // --- Terminal format ---

    #[test]
    fn terminal_format_has_summary_header() {
        let output = format_terminal(&[block_diag()], 1, 1, &ProjectMetrics::default());
        assert!(output.starts_with("exspec v"));
        assert!(output.contains("1 test files"));
        assert!(output.contains("1 test functions"));
    }

    #[test]
    fn terminal_format_has_score_footer() {
        let output = format_terminal(&[block_diag()], 1, 1, &ProjectMetrics::default());
        assert!(output.contains("Score: BLOCK 1 | WARN 0 | INFO 0 | PASS 0"));
    }

    #[test]
    fn terminal_format_block() {
        let output = format_terminal(&[block_diag()], 1, 1, &ProjectMetrics::default());
        assert!(output.contains("BLOCK test.py:10 T001 assertion-free: test has no assertions"));
    }

    #[test]
    fn terminal_format_warn() {
        let output = format_terminal(&[warn_diag()], 1, 1, &ProjectMetrics::default());
        assert!(output.contains("WARN test.py:5 T003 giant-test: 73 lines, threshold: 50"));
    }

    #[test]
    fn terminal_format_multiple() {
        let output = format_terminal(
            &[block_diag(), warn_diag()],
            2,
            2,
            &ProjectMetrics::default(),
        );
        assert!(output.contains("BLOCK"));
        assert!(output.contains("WARN"));
    }

    #[test]
    fn terminal_format_empty_has_header_and_footer() {
        let output = format_terminal(&[], 0, 0, &ProjectMetrics::default());
        assert!(output.contains("exspec v"));
        assert!(output.contains("Score:"));
    }

    // --- JSON format ---

    #[test]
    fn json_format_has_version_and_summary() {
        let output = format_json(&[block_diag()], 1, 1, &ProjectMetrics::default());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["version"].is_string());
        assert!(parsed["summary"].is_object());
        assert_eq!(parsed["summary"]["files"], 1);
        assert_eq!(parsed["summary"]["functions"], 1);
        assert_eq!(parsed["summary"]["block"], 1);
        assert_eq!(parsed["summary"]["warn"], 0);
        assert_eq!(parsed["summary"]["pass"], 0);
    }

    #[test]
    fn json_format_has_diagnostics_and_metrics() {
        let output = format_json(&[block_diag()], 1, 1, &ProjectMetrics::default());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["diagnostics"].is_array());
        assert!(parsed["metrics"].is_object());
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn json_format_empty() {
        let output = format_json(&[], 0, 0, &ProjectMetrics::default());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["summary"]["functions"], 0);
    }

    // --- Exit code ---

    // --- Empty result UX ---

    #[test]
    fn terminal_format_zero_files_shows_guidance() {
        let output = format_terminal(&[], 0, 0, &ProjectMetrics::default());
        assert!(
            output.contains("No test files found"),
            "expected guidance message, got: {output}"
        );
    }

    #[test]
    fn json_format_zero_files_has_guidance() {
        let output = format_json(&[], 0, 0, &ProjectMetrics::default());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["guidance"].is_string());
    }

    // --- pass_count multi-violation ---

    #[test]
    fn pass_count_with_multi_violation_function() {
        let d1 = Diagnostic {
            rule: RuleId::new("T001"),
            severity: Severity::Block,
            file: "test.py".to_string(),
            line: Some(10),
            message: "assertion-free".to_string(),
            details: None,
        };
        let d2 = Diagnostic {
            rule: RuleId::new("T003"),
            severity: Severity::Warn,
            file: "test.py".to_string(),
            line: Some(10),
            message: "giant-test".to_string(),
            details: None,
        };
        let output = format_terminal(&[d1, d2], 1, 2, &ProjectMetrics::default());
        assert!(output.contains("PASS 1"), "expected PASS 1, got: {output}");
    }

    #[test]
    fn pass_count_excludes_file_level_diagnostics() {
        let d1 = Diagnostic {
            rule: RuleId::new("T004"),
            severity: Severity::Info,
            file: "test.py".to_string(),
            line: None,
            message: "no-parameterized".to_string(),
            details: None,
        };
        let output = format_terminal(&[d1], 1, 1, &ProjectMetrics::default());
        assert!(output.contains("PASS 1"), "expected PASS 1, got: {output}");
    }

    #[test]
    fn terminal_format_nonzero_files_no_guidance() {
        let output = format_terminal(&[], 1, 0, &ProjectMetrics::default());
        assert!(!output.contains("No test files found"));
    }

    #[test]
    fn exit_code_block_returns_1() {
        assert_eq!(compute_exit_code(&[block_diag()], false), 1);
    }

    #[test]
    fn exit_code_warn_only_returns_0() {
        assert_eq!(compute_exit_code(&[warn_diag()], false), 0);
    }

    #[test]
    fn exit_code_strict_warn_returns_1() {
        assert_eq!(compute_exit_code(&[warn_diag()], true), 1);
    }

    #[test]
    fn exit_code_empty_returns_0() {
        assert_eq!(compute_exit_code(&[], false), 0);
    }

    // --- Metrics display ---

    #[test]
    fn terminal_metrics_section_between_diagnostics_and_score() {
        let metrics = ProjectMetrics {
            mock_density_avg: 2.3,
            mock_class_max: 4,
            parameterized_ratio: 0.15,
            pbt_ratio: 0.4,
            assertion_density_avg: 1.8,
            contract_coverage: 0.2,
            ..Default::default()
        };
        let output = format_terminal(&[block_diag()], 5, 187, &metrics);
        let metrics_pos = output.find("Metrics:").expect("Metrics section missing");
        let diag_pos = output.find("BLOCK test.py").expect("diagnostic missing");
        let score_pos = output.find("Score:").expect("Score missing");
        assert!(
            diag_pos < metrics_pos,
            "Metrics should come after diagnostics"
        );
        assert!(metrics_pos < score_pos, "Metrics should come before Score");
    }

    #[test]
    fn terminal_metrics_mock_density_line() {
        let metrics = ProjectMetrics {
            mock_density_avg: 2.3,
            mock_class_max: 4,
            ..Default::default()
        };
        let output = format_terminal(&[], 1, 1, &metrics);
        assert!(
            output.contains("2.3/test (avg)"),
            "mock density avg: {output}"
        );
        assert!(
            output.contains("4 distinct classes/test (max)"),
            "mock class max: {output}"
        );
    }

    #[test]
    fn terminal_metrics_parameterized_line() {
        let metrics = ProjectMetrics {
            parameterized_ratio: 0.15,
            ..Default::default()
        };
        let output = format_terminal(&[], 5, 20, &metrics);
        assert!(output.contains("15%"), "parameterized pct: {output}");
        assert!(output.contains("3/20"), "parameterized fraction: {output}");
    }

    #[test]
    fn terminal_metrics_pbt_and_contract_file_count() {
        let metrics = ProjectMetrics {
            pbt_ratio: 0.4,
            contract_coverage: 0.2,
            ..Default::default()
        };
        let output = format_terminal(&[], 5, 1, &metrics);
        assert!(output.contains("2/5 files"), "pbt files: {output}");
        assert!(output.contains("1/5 files"), "contract files: {output}");
    }

    #[test]
    fn json_metrics_has_all_fields() {
        let metrics = ProjectMetrics {
            mock_density_avg: 1.5,
            mock_class_max: 2,
            parameterized_ratio: 0.3,
            pbt_ratio: 0.5,
            assertion_density_avg: 2.0,
            contract_coverage: 0.1,
            test_source_ratio: 0.8,
        };
        let output = format_json(&[], 1, 1, &metrics);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let m = &parsed["metrics"];
        assert_eq!(m["mock_density_avg"], 1.5);
        assert_eq!(m["mock_class_max"], 2);
        assert_eq!(m["parameterized_ratio"], 0.3);
        assert_eq!(m["pbt_ratio"], 0.5);
        assert_eq!(m["assertion_density_avg"], 2.0);
        assert_eq!(m["contract_coverage"], 0.1);
        assert_eq!(m["test_source_ratio"], 0.8);
    }

    #[test]
    fn json_metrics_values_are_numbers() {
        let metrics = ProjectMetrics {
            mock_density_avg: 1.0,
            mock_class_max: 3,
            ..Default::default()
        };
        let output = format_json(&[], 1, 1, &metrics);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["metrics"]["mock_density_avg"].is_number());
        assert!(parsed["metrics"]["mock_class_max"].is_number());
    }

    // --- SARIF format ---

    fn info_diag() -> Diagnostic {
        Diagnostic {
            rule: RuleId::new("T005"),
            severity: Severity::Info,
            file: "test.py".to_string(),
            line: None,
            message: "pbt-missing".to_string(),
            details: None,
        }
    }

    fn parse_sarif(output: &str) -> serde_json::Value {
        serde_json::from_str(output).expect("SARIF should be valid JSON")
    }

    #[test]
    fn sarif_valid_json() {
        let output = format_sarif(&[block_diag()]);
        parse_sarif(&output);
    }

    #[test]
    fn sarif_has_schema_url() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        assert!(parsed["$schema"].is_string());
        assert!(parsed["$schema"].as_str().unwrap().contains("sarif"));
    }

    #[test]
    fn sarif_version_2_1_0() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        assert_eq!(parsed["version"], "2.1.0");
    }

    #[test]
    fn sarif_tool_driver_name() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        assert_eq!(parsed["runs"][0]["tool"]["driver"]["name"], "exspec");
    }

    #[test]
    fn sarif_tool_driver_version() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        assert_eq!(
            parsed["runs"][0]["tool"]["driver"]["version"],
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn sarif_rules_match_registry_count() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), RULE_REGISTRY.len());
    }

    #[test]
    fn sarif_rules_have_short_description() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        let rule0 = &parsed["runs"][0]["tool"]["driver"]["rules"][0];
        assert!(rule0["shortDescription"].is_object());
        assert!(rule0["shortDescription"]["text"].is_string());
    }

    #[test]
    fn sarif_block_maps_to_error() {
        let output = format_sarif(&[block_diag()]);
        let parsed = parse_sarif(&output);
        assert_eq!(parsed["runs"][0]["results"][0]["level"], "error");
    }

    #[test]
    fn sarif_warn_maps_to_warning() {
        let output = format_sarif(&[warn_diag()]);
        let parsed = parse_sarif(&output);
        assert_eq!(parsed["runs"][0]["results"][0]["level"], "warning");
    }

    #[test]
    fn sarif_info_maps_to_note() {
        let output = format_sarif(&[info_diag()]);
        let parsed = parse_sarif(&output);
        assert_eq!(parsed["runs"][0]["results"][0]["level"], "note");
    }

    #[test]
    fn sarif_file_level_diag_start_line_1() {
        let output = format_sarif(&[info_diag()]);
        let parsed = parse_sarif(&output);
        let region = &parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];
        assert_eq!(region["startLine"], 1);
    }

    #[test]
    fn sarif_result_has_location_and_uri() {
        let output = format_sarif(&[block_diag()]);
        let parsed = parse_sarif(&output);
        let loc = &parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "test.py");
        assert_eq!(loc["region"]["startLine"], 10);
    }

    #[test]
    fn sarif_empty_diagnostics_empty_results() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        let results = parsed["runs"][0]["results"].as_array().unwrap();
        assert!(results.is_empty());
        let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), 16);
    }

    #[test]
    fn sarif_invocations_execution_successful() {
        let output = format_sarif(&[]);
        let parsed = parse_sarif(&output);
        assert_eq!(
            parsed["runs"][0]["invocations"][0]["executionSuccessful"],
            true
        );
    }
}
