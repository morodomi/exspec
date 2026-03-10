use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Block,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Block => "BLOCK",
            Severity::Warn => "WARN",
            Severity::Info => "INFO",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Severity::Block => 1,
            Severity::Warn => 0,
            Severity::Info => 0,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BLOCK" => Ok(Severity::Block),
            "WARN" => Ok(Severity::Warn),
            "INFO" => Ok(Severity::Info),
            _ => Err(format!("unknown severity: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuleId(pub String);

impl RuleId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule: RuleId,
    pub severity: Severity,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
    pub details: Option<String>,
}

pub struct Config {
    pub mock_max: usize,
    pub mock_class_max: usize,
    pub test_max_lines: usize,
    pub parameterized_min_ratio: f64,
    pub fixture_max: usize,
    pub min_assertions_for_t105: usize,
    pub min_duplicate_count: usize,
    pub disabled_rules: Vec<RuleId>,
    pub custom_assertion_patterns: Vec<String>,
    pub ignore_patterns: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mock_max: 5,
            mock_class_max: 3,
            test_max_lines: 50,
            parameterized_min_ratio: 0.1,
            fixture_max: 5,
            min_assertions_for_t105: 5,
            min_duplicate_count: 3,
            disabled_rules: Vec::new(),
            custom_assertion_patterns: Vec::new(),
            ignore_patterns: Vec::new(),
        }
    }
}

use crate::extractor::TestFunction;

pub fn evaluate_rules(functions: &[TestFunction], config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for func in functions {
        let analysis = &func.analysis;

        // T001: assertion-free
        if !is_disabled(config, "T001")
            && !is_suppressed(analysis, "T001")
            && analysis.assertion_count == 0
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T001"),
                severity: Severity::Block,
                file: func.file.clone(),
                line: Some(func.line),
                message: "assertion-free: test has no assertions".to_string(),
                details: None,
            });
        }

        // T002: mock-overuse
        if !is_disabled(config, "T002")
            && !is_suppressed(analysis, "T002")
            && (analysis.mock_count > config.mock_max
                || analysis.mock_classes.len() > config.mock_class_max)
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T002"),
                severity: Severity::Warn,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "mock-overuse: {} mocks ({} classes), threshold: {} mocks / {} classes",
                    analysis.mock_count,
                    analysis.mock_classes.len(),
                    config.mock_max,
                    config.mock_class_max,
                ),
                details: None,
            });
        }

        // T003: giant-test
        if !is_disabled(config, "T003")
            && !is_suppressed(analysis, "T003")
            && analysis.line_count > config.test_max_lines
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T003"),
                severity: Severity::Warn,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "giant-test: {} lines, threshold: {}",
                    analysis.line_count, config.test_max_lines,
                ),
                details: None,
            });
        }

        // T102: fixture-sprawl
        if !is_disabled(config, "T102")
            && !is_suppressed(analysis, "T102")
            && analysis.fixture_count > config.fixture_max
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T102"),
                severity: Severity::Warn,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "fixture-sprawl: {} fixtures, threshold: {}",
                    analysis.fixture_count, config.fixture_max,
                ),
                details: None,
            });
        }

        // T108: wait-and-see
        if !is_disabled(config, "T108") && !is_suppressed(analysis, "T108") && analysis.has_wait {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T108"),
                severity: Severity::Warn,
                file: func.file.clone(),
                line: Some(func.line),
                message: "wait-and-see: test uses sleep/delay (causes flaky tests, consider async/mock alternatives)".to_string(),
                details: None,
            });
        }

        // T106: duplicate-literal-assertion
        if !is_disabled(config, "T106")
            && !is_suppressed(analysis, "T106")
            && analysis.duplicate_literal_count >= config.min_duplicate_count
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T106"),
                severity: Severity::Info,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "duplicate-literal-assertion: literal appears {} times in assertions (consider extracting to constant or parameter)",
                    analysis.duplicate_literal_count,
                ),
                details: None,
            });
        }

        // T107: assertion-roulette
        if !is_disabled(config, "T107")
            && !is_suppressed(analysis, "T107")
            && analysis.assertion_count >= 2
            && analysis.assertion_message_count == 0
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T107"),
                severity: Severity::Info,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "assertion-roulette: {} assertions without messages (add failure messages for readability)",
                    analysis.assertion_count,
                ),
                details: None,
            });
        }

        // T109: undescriptive-test-name
        if !is_disabled(config, "T109")
            && !is_suppressed(analysis, "T109")
            && is_undescriptive_test_name(&func.name)
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T109"),
                severity: Severity::Info,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "undescriptive-test-name: \"{}\" does not describe behavior (use descriptive names like \"test_user_creation_returns_valid_id\")",
                    func.name,
                ),
                details: None,
            });
        }

        // T101: how-not-what
        if !is_disabled(config, "T101")
            && !is_suppressed(analysis, "T101")
            && analysis.how_not_what_count > 0
        {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T101"),
                severity: Severity::Warn,
                file: func.file.clone(),
                line: Some(func.line),
                message: format!(
                    "how-not-what: {} implementation-testing pattern(s) detected",
                    analysis.how_not_what_count,
                ),
                details: None,
            });
        }
    }

    diagnostics
}

use crate::extractor::FileAnalysis;

pub fn evaluate_file_rules(analyses: &[FileAnalysis], config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for analysis in analyses {
        if analysis.functions.is_empty() {
            continue;
        }

        // T006: low-assertion-density
        // Total assertions / total functions < 1.0 → WARN
        // Skip if ALL functions are assertion-free (T001 handles those entirely)
        if !is_disabled(config, "T006") {
            let has_any_asserting = analysis
                .functions
                .iter()
                .any(|f| f.analysis.assertion_count > 0);

            if has_any_asserting {
                let total_assertions: usize = analysis
                    .functions
                    .iter()
                    .map(|f| f.analysis.assertion_count)
                    .sum();
                let density = total_assertions as f64 / analysis.functions.len() as f64;

                if density < 1.0 {
                    diagnostics.push(Diagnostic {
                        rule: RuleId::new("T006"),
                        severity: Severity::Warn,
                        file: analysis.file.clone(),
                        line: None,
                        message: format!(
                            "low-assertion-density: {density:.2} assertions/test (threshold: 1.0)",
                        ),
                        details: None,
                    });
                }
            }
        }

        // T004: no-parameterized
        if !is_disabled(config, "T004") {
            let total = analysis.functions.len();
            let ratio = analysis.parameterized_count as f64 / total as f64;
            if ratio < config.parameterized_min_ratio {
                diagnostics.push(Diagnostic {
                    rule: RuleId::new("T004"),
                    severity: Severity::Info,
                    file: analysis.file.clone(),
                    line: None,
                    message: format!(
                        "no-parameterized: {}/{} ({:.0}%) parameterized, threshold: {:.0}%",
                        analysis.parameterized_count,
                        total,
                        ratio * 100.0,
                        config.parameterized_min_ratio * 100.0,
                    ),
                    details: None,
                });
            }
        }

        // T005: pbt-missing
        if !is_disabled(config, "T005") && !analysis.has_pbt_import {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T005"),
                severity: Severity::Info,
                file: analysis.file.clone(),
                line: None,
                message: "pbt-missing: no property-based testing library imported".to_string(),
                details: None,
            });
        }

        // T008: no-contract
        if !is_disabled(config, "T008") && !analysis.has_contract_import {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T008"),
                severity: Severity::Info,
                file: analysis.file.clone(),
                line: None,
                message: "no-contract: no contract/schema library imported".to_string(),
                details: None,
            });
        }

        // T105: deterministic-no-metamorphic
        if !is_disabled(config, "T105") {
            let total_assertions: usize = analysis
                .functions
                .iter()
                .map(|f| f.analysis.assertion_count)
                .sum();
            if total_assertions >= config.min_assertions_for_t105
                && !analysis.has_relational_assertion
            {
                diagnostics.push(Diagnostic {
                    rule: RuleId::new("T105"),
                    severity: Severity::Info,
                    file: analysis.file.clone(),
                    line: None,
                    message: format!(
                        "deterministic-no-metamorphic: {total_assertions} assertions, all exact equality",
                    ),
                    details: None,
                });
            }
        }

        // T103: missing-error-test
        if !is_disabled(config, "T103") && !analysis.has_error_test {
            diagnostics.push(Diagnostic {
                rule: RuleId::new("T103"),
                severity: Severity::Info,
                file: analysis.file.clone(),
                line: None,
                message: "missing-error-test: no error/exception test found in file".to_string(),
                details: None,
            });
        }
    }

    diagnostics
}

pub fn evaluate_project_rules(
    test_file_count: usize,
    source_file_count: usize,
    config: &Config,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // T007: test-source-ratio
    if !is_disabled(config, "T007") && source_file_count > 0 {
        let ratio = test_file_count as f64 / source_file_count as f64;
        diagnostics.push(Diagnostic {
            rule: RuleId::new("T007"),
            severity: Severity::Info,
            file: "<project>".to_string(),
            line: None,
            message: format!(
                "test-source-ratio: {test_file_count}/{source_file_count} ({ratio:.2})",
            ),
            details: None,
        });
    }

    diagnostics
}

/// Blacklist of generic test name suffixes (after stripping test_ prefix).
const GENERIC_TEST_NAMES: &[&str] = &[
    "case", "example", "sample", "basic", "data", "check", "func", "method",
];

/// Check if a test name is undescriptive.
///
/// Violation patterns:
/// - `test_` + digits only: `test_1`, `test_123`
/// - `test` + digits only (camelCase): `test1`, `testCase1`
/// - `test_` + single short word (4 chars or less): `test_it`, `test_foo`
/// - Generic blacklist: `test_case`, `test_example`, etc.
/// - Short string names (TypeScript): `"test 1"`, `"works"`, `"it"`
pub fn is_undescriptive_test_name(name: &str) -> bool {
    // Handle TypeScript string-style test names (may contain spaces)
    let trimmed = name.trim_matches(|c: char| c == '"' || c == '\'');
    let normalized = if trimmed != name {
        // String-style name: normalize spaces to underscores for uniform checks
        trimmed.to_lowercase().replace(' ', "_")
    } else {
        name.to_lowercase()
    };

    // Strip test_ or test prefix
    let suffix = if let Some(s) = normalized.strip_prefix("test_") {
        s
    } else if let Some(s) = normalized.strip_prefix("test") {
        if s.is_empty() {
            return true; // just "test"
        }
        s
    } else {
        // No test prefix - check if the whole name is very short/generic
        // e.g. TypeScript `it('works', ...)` -> name="works"
        // Single word or very short: undescriptive
        let is_single_word = !normalized.contains('_') && !normalized.contains(' ');
        return is_single_word || GENERIC_TEST_NAMES.contains(&normalized.as_str());
    };

    // Digits only after prefix
    if suffix.chars().all(|c| c.is_ascii_digit() || c == '_')
        && suffix.chars().any(|c| c.is_ascii_digit())
    {
        return true;
    }

    // Single short word (4 chars or less, no underscores = single word)
    if !suffix.contains('_') && suffix.len() <= 4 {
        return true;
    }

    // Generic blacklist
    GENERIC_TEST_NAMES.contains(&suffix)
}

fn is_disabled(config: &Config, rule_id: &str) -> bool {
    config.disabled_rules.iter().any(|r| r.0 == rule_id)
}

fn is_suppressed(analysis: &crate::extractor::TestAnalysis, rule_id: &str) -> bool {
    analysis.suppressed_rules.iter().any(|r| r.0 == rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::{TestAnalysis, TestFunction};

    fn make_func(name: &str, analysis: TestAnalysis) -> TestFunction {
        TestFunction {
            name: name.to_string(),
            file: "test.py".to_string(),
            line: 1,
            end_line: 10,
            analysis,
        }
    }

    // --- Severity tests (from Phase 1) ---

    #[test]
    fn severity_ordering() {
        assert!(Severity::Block > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }

    #[test]
    fn severity_as_str_roundtrip() {
        for severity in [Severity::Block, Severity::Warn, Severity::Info] {
            let s = severity.as_str();
            let parsed = Severity::from_str(s).unwrap();
            assert_eq!(parsed, severity);
        }
    }

    #[test]
    fn severity_to_exit_code() {
        assert_eq!(Severity::Block.exit_code(), 1);
        assert_eq!(Severity::Warn.exit_code(), 0);
        assert_eq!(Severity::Info.exit_code(), 0);
    }

    #[test]
    fn severity_from_str_invalid() {
        assert!(Severity::from_str("UNKNOWN").is_err());
    }

    #[test]
    fn rule_id_display() {
        let id = RuleId::new("T001");
        assert_eq!(id.to_string(), "T001");
    }

    // --- T001: assertion-free ---

    #[test]
    fn t001_assertion_count_zero_produces_block() {
        let funcs = vec![make_func(
            "test_no_assert",
            TestAnalysis {
                assertion_count: 0,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T001"));
        assert_eq!(diags[0].severity, Severity::Block);
    }

    #[test]
    fn t001_assertion_count_positive_no_diagnostic() {
        let funcs = vec![make_func(
            "test_with_assert",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    // --- T002: mock-overuse ---

    #[test]
    fn t002_mock_count_exceeds_threshold_produces_warn() {
        let funcs = vec![make_func(
            "test_many_mocks",
            TestAnalysis {
                assertion_count: 1,
                mock_count: 6,
                mock_classes: vec![
                    "a".into(),
                    "b".into(),
                    "c".into(),
                    "d".into(),
                    "e".into(),
                    "f".into(),
                ],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T002"));
        assert_eq!(diags[0].severity, Severity::Warn);
    }

    #[test]
    fn t002_mock_count_within_threshold_no_diagnostic() {
        let funcs = vec![make_func(
            "test_few_mocks",
            TestAnalysis {
                assertion_count: 1,
                mock_count: 2,
                mock_classes: vec!["db".into()],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t002_mock_class_count_exceeds_threshold_alone_produces_warn() {
        let funcs = vec![make_func(
            "test_many_classes",
            TestAnalysis {
                assertion_count: 1,
                mock_count: 4, // within mock_max=5
                mock_classes: vec!["a".into(), "b".into(), "c".into(), "d".into()], // > mock_class_max=3
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T002"));
    }

    // --- T003: giant-test ---

    #[test]
    fn t003_line_count_exceeds_threshold_produces_warn() {
        let funcs = vec![make_func(
            "test_giant",
            TestAnalysis {
                assertion_count: 1,
                line_count: 73,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T003"));
        assert_eq!(diags[0].severity, Severity::Warn);
    }

    #[test]
    fn t003_line_count_at_threshold_no_diagnostic() {
        let funcs = vec![make_func(
            "test_boundary",
            TestAnalysis {
                assertion_count: 1,
                line_count: 50, // exactly at threshold, strict >
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    // --- Config disabled ---

    #[test]
    fn disabled_rule_not_reported() {
        let funcs = vec![make_func(
            "test_no_assert",
            TestAnalysis {
                assertion_count: 0,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T001")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        assert!(diags.is_empty());
    }

    // --- Suppression ---

    #[test]
    fn suppressed_rule_not_reported() {
        let funcs = vec![make_func(
            "test_many_mocks",
            TestAnalysis {
                assertion_count: 1,
                mock_count: 6,
                mock_classes: vec![
                    "a".into(),
                    "b".into(),
                    "c".into(),
                    "d".into(),
                    "e".into(),
                    "f".into(),
                ],
                suppressed_rules: vec![RuleId::new("T002")],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    // --- T101: how-not-what ---

    #[test]
    fn t101_how_not_what_produces_warn() {
        let funcs = vec![make_func(
            "test_calls_repo",
            TestAnalysis {
                assertion_count: 1,
                how_not_what_count: 2,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T101"));
        assert_eq!(diags[0].severity, Severity::Warn);
        assert!(diags[0]
            .message
            .contains("2 implementation-testing pattern(s)"));
    }

    #[test]
    fn t101_zero_how_not_what_no_diagnostic() {
        let funcs = vec![make_func(
            "test_behavior",
            TestAnalysis {
                assertion_count: 1,
                how_not_what_count: 0,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t101_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_calls_repo",
            TestAnalysis {
                assertion_count: 1,
                how_not_what_count: 2,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T101")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        assert!(diags.is_empty());
    }

    #[test]
    fn t101_suppressed_no_diagnostic() {
        let funcs = vec![make_func(
            "test_calls_repo",
            TestAnalysis {
                assertion_count: 1,
                how_not_what_count: 2,
                suppressed_rules: vec![RuleId::new("T101")],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    // --- T102: fixture-sprawl ---

    #[test]
    fn t102_fixture_count_exceeds_threshold_produces_warn() {
        let funcs = vec![make_func(
            "test_sprawl",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 7,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T102"));
        assert_eq!(diags[0].severity, Severity::Warn);
        assert!(diags[0].message.contains("7 fixtures"));
    }

    #[test]
    fn t102_fixture_count_at_threshold_no_diagnostic() {
        let funcs = vec![make_func(
            "test_fixtures_at_threshold",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 5, // exactly at threshold, strict >
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t102_zero_fixtures_no_diagnostic() {
        let funcs = vec![make_func(
            "test_no_fixtures",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 0,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t102_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_sprawl",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 7,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T102")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        assert!(diags.is_empty());
    }

    #[test]
    fn t102_suppressed_no_diagnostic() {
        let funcs = vec![make_func(
            "test_sprawl",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 7,
                suppressed_rules: vec![RuleId::new("T102")],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t102_custom_threshold() {
        let funcs = vec![make_func(
            "test_sprawl",
            TestAnalysis {
                assertion_count: 1,
                fixture_count: 4,
                ..Default::default()
            },
        )];
        let config = Config {
            fixture_max: 3,
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T102"));
    }

    // --- Multiple violations ---

    #[test]
    fn multiple_violations_reported() {
        let funcs = vec![make_func(
            "test_assertion_free_and_giant",
            TestAnalysis {
                assertion_count: 0,
                line_count: 73,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        assert_eq!(diags.len(), 2);
        let rule_ids: Vec<&str> = diags.iter().map(|d| d.rule.0.as_str()).collect();
        assert!(rule_ids.contains(&"T001"));
        assert!(rule_ids.contains(&"T003"));
    }

    // === File-level rules ===

    fn make_file_analysis(
        file: &str,
        functions: Vec<TestFunction>,
        has_pbt_import: bool,
        has_contract_import: bool,
        parameterized_count: usize,
    ) -> FileAnalysis {
        make_file_analysis_full(
            file,
            functions,
            has_pbt_import,
            has_contract_import,
            false,
            parameterized_count,
        )
    }

    fn make_file_analysis_full(
        file: &str,
        functions: Vec<TestFunction>,
        has_pbt_import: bool,
        has_contract_import: bool,
        has_error_test: bool,
        parameterized_count: usize,
    ) -> FileAnalysis {
        FileAnalysis {
            file: file.to_string(),
            functions,
            has_pbt_import,
            has_contract_import,
            has_error_test,
            has_relational_assertion: false,
            parameterized_count,
        }
    }

    // --- T006: low-assertion-density ---

    #[test]
    fn t006_low_density_produces_warn() {
        // density = total_assertions / total_functions (all functions, including assertion-free).
        // Fires when density < 1.0 and at least one function has assertions.
        // When ALL functions are assertion-free, T006 does not fire (T001 handles those).
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 0,
                    ..Default::default()
                },
            ),
            make_func(
                "test_c",
                TestAnalysis {
                    assertion_count: 0,
                    ..Default::default()
                },
            ),
        ];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T006"));
    }

    #[test]
    fn t006_high_density_no_diagnostic() {
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 2,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
        ];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T006"));
    }

    #[test]
    fn t006_all_assertion_free_no_diagnostic() {
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 0,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 0,
                    ..Default::default()
                },
            ),
        ];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T006"),
            "T006 should not fire when all functions are assertion-free (T001 handles)"
        );
    }

    #[test]
    fn t006_empty_file_no_diagnostic() {
        let analyses = vec![make_file_analysis("test.py", vec![], false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T006"));
    }

    #[test]
    fn t006_disabled_no_diagnostic() {
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 0,
                    ..Default::default()
                },
            ),
        ];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let config = Config {
            disabled_rules: vec![RuleId::new("T006")],
            ..Config::default()
        };
        let diags = evaluate_file_rules(&analyses, &config);
        assert!(!diags.iter().any(|d| d.rule.0 == "T006"));
    }

    // --- T004: no-parameterized ---

    #[test]
    fn t004_no_parameterized_produces_info() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T004"));
        let t004 = diags.iter().find(|d| d.rule.0 == "T004").unwrap();
        assert_eq!(t004.severity, Severity::Info);
    }

    #[test]
    fn t004_sufficient_parameterized_no_diagnostic() {
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
        ];
        // parameterized_count=1 out of 2 → ratio 0.5 >= 0.1
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 1)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T004"));
    }

    #[test]
    fn t004_custom_threshold() {
        let funcs = vec![
            make_func(
                "test_a",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
            make_func(
                "test_b",
                TestAnalysis {
                    assertion_count: 1,
                    ..Default::default()
                },
            ),
        ];
        // 1/2 = 0.5, threshold 0.6 → should fire
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 1)];
        let config = Config {
            parameterized_min_ratio: 0.6,
            ..Config::default()
        };
        let diags = evaluate_file_rules(&analyses, &config);
        assert!(diags.iter().any(|d| d.rule.0 == "T004"));
    }

    // --- T005: pbt-missing ---

    #[test]
    fn t005_no_pbt_import_produces_info() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T005"));
    }

    #[test]
    fn t005_has_pbt_import_no_diagnostic() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, true, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T005"));
    }

    #[test]
    fn t005_empty_file_no_diagnostic() {
        let analyses = vec![make_file_analysis("test.py", vec![], false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T005"));
    }

    // --- T008: no-contract ---

    #[test]
    fn t008_no_contract_import_produces_info() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T008"));
    }

    #[test]
    fn t008_has_contract_import_no_diagnostic() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, true, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T008"));
    }

    #[test]
    fn t008_empty_file_no_diagnostic() {
        let analyses = vec![make_file_analysis("test.py", vec![], false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T008"));
    }

    // --- T103: missing-error-test ---

    #[test]
    fn t103_no_error_test_produces_info() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T103"));
        let t103 = diags.iter().find(|d| d.rule.0 == "T103").unwrap();
        assert_eq!(t103.severity, Severity::Info);
    }

    #[test]
    fn t103_has_error_test_no_diagnostic() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis_full(
            "test.py", funcs, false, false, true, 0,
        )];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T103"));
    }

    #[test]
    fn t103_empty_file_no_diagnostic() {
        let analyses = vec![make_file_analysis("test.py", vec![], false, false, 0)];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T103"));
    }

    // --- T105: deterministic-no-metamorphic ---

    #[test]
    fn t105_all_equality_above_threshold_produces_info() {
        let funcs: Vec<TestFunction> = (0..5)
            .map(|i| {
                make_func(
                    &format!("test_{i}"),
                    TestAnalysis {
                        assertion_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let analyses = vec![make_file_analysis_full(
            "test.py", funcs, false, false, false, 0,
        )];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T105"));
        let t105 = diags.iter().find(|d| d.rule.0 == "T105").unwrap();
        assert_eq!(t105.severity, Severity::Info);
        assert!(t105.message.contains("5 assertions"));
    }

    #[test]
    fn t105_has_relational_no_diagnostic() {
        let funcs: Vec<TestFunction> = (0..5)
            .map(|i| {
                make_func(
                    &format!("test_{i}"),
                    TestAnalysis {
                        assertion_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let mut analysis = make_file_analysis_full("test.py", funcs, false, false, false, 0);
        analysis.has_relational_assertion = true;
        let diags = evaluate_file_rules(&[analysis], &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T105"));
    }

    #[test]
    fn t105_below_threshold_no_diagnostic() {
        let funcs: Vec<TestFunction> = (0..2)
            .map(|i| {
                make_func(
                    &format!("test_{i}"),
                    TestAnalysis {
                        assertion_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let analyses = vec![make_file_analysis_full(
            "test.py", funcs, false, false, false, 0,
        )];
        let diags = evaluate_file_rules(&analyses, &Config::default());
        assert!(!diags.iter().any(|d| d.rule.0 == "T105"));
    }

    #[test]
    fn t105_disabled_no_diagnostic() {
        let funcs: Vec<TestFunction> = (0..5)
            .map(|i| {
                make_func(
                    &format!("test_{i}"),
                    TestAnalysis {
                        assertion_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let analyses = vec![make_file_analysis_full(
            "test.py", funcs, false, false, false, 0,
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T105")],
            ..Config::default()
        };
        let diags = evaluate_file_rules(&analyses, &config);
        assert!(!diags.iter().any(|d| d.rule.0 == "T105"));
    }

    #[test]
    fn t105_custom_threshold() {
        let funcs: Vec<TestFunction> = (0..3)
            .map(|i| {
                make_func(
                    &format!("test_{i}"),
                    TestAnalysis {
                        assertion_count: 1,
                        ..Default::default()
                    },
                )
            })
            .collect();
        let analyses = vec![make_file_analysis_full(
            "test.py", funcs, false, false, false, 0,
        )];
        let config = Config {
            min_assertions_for_t105: 3,
            ..Config::default()
        };
        let diags = evaluate_file_rules(&analyses, &config);
        assert!(diags.iter().any(|d| d.rule.0 == "T105"));
    }

    #[test]
    fn t103_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_a",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let analyses = vec![make_file_analysis("test.py", funcs, false, false, 0)];
        let config = Config {
            disabled_rules: vec![RuleId::new("T103")],
            ..Config::default()
        };
        let diags = evaluate_file_rules(&analyses, &config);
        assert!(!diags.iter().any(|d| d.rule.0 == "T103"));
    }

    // === Project-level rules ===

    // --- T007: test-source-ratio ---

    #[test]
    fn t007_produces_info_with_ratio() {
        let diags = evaluate_project_rules(5, 10, &Config::default());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, RuleId::new("T007"));
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("5/10"));
    }

    #[test]
    fn t007_zero_source_files_no_diagnostic() {
        let diags = evaluate_project_rules(5, 0, &Config::default());
        assert!(diags.is_empty());
    }

    #[test]
    fn t007_disabled_no_diagnostic() {
        let config = Config {
            disabled_rules: vec![RuleId::new("T007")],
            ..Config::default()
        };
        let diags = evaluate_project_rules(5, 10, &config);
        assert!(diags.is_empty());
    }

    // --- T108: wait-and-see ---

    #[test]
    fn t108_has_wait_produces_warn() {
        let funcs = vec![make_func(
            "test_sleepy",
            TestAnalysis {
                assertion_count: 1,
                has_wait: true,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t108: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T108"))
            .collect();
        assert_eq!(t108.len(), 1);
        assert_eq!(t108[0].severity, Severity::Warn);
        assert!(t108[0].message.contains("wait-and-see"));
    }

    #[test]
    fn t108_no_wait_no_diagnostic() {
        let funcs = vec![make_func(
            "test_fast",
            TestAnalysis {
                assertion_count: 1,
                has_wait: false,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t108: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T108"))
            .collect();
        assert!(t108.is_empty());
    }

    #[test]
    fn t108_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_sleepy",
            TestAnalysis {
                assertion_count: 1,
                has_wait: true,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T108")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        let t108: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T108"))
            .collect();
        assert!(t108.is_empty());
    }

    #[test]
    fn t108_suppressed_no_diagnostic() {
        let funcs = vec![make_func(
            "test_sleepy",
            TestAnalysis {
                assertion_count: 1,
                has_wait: true,
                suppressed_rules: vec![RuleId::new("T108")],
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t108: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T108"))
            .collect();
        assert!(t108.is_empty());
    }

    // --- T109: undescriptive-test-name ---

    #[test]
    fn t109_is_undescriptive_digits_only() {
        assert!(is_undescriptive_test_name("test_1"));
        assert!(is_undescriptive_test_name("test_123"));
        assert!(is_undescriptive_test_name("test1"));
    }

    #[test]
    fn t109_is_undescriptive_short_word() {
        assert!(is_undescriptive_test_name("test_it"));
        assert!(is_undescriptive_test_name("test_foo"));
        assert!(is_undescriptive_test_name("test_run"));
        assert!(is_undescriptive_test_name("test_main"));
    }

    #[test]
    fn t109_is_undescriptive_blacklist() {
        assert!(is_undescriptive_test_name("test_case"));
        assert!(is_undescriptive_test_name("test_example"));
        assert!(is_undescriptive_test_name("test_sample"));
        assert!(is_undescriptive_test_name("test_basic"));
        assert!(is_undescriptive_test_name("test_data"));
        assert!(is_undescriptive_test_name("test_check"));
        assert!(is_undescriptive_test_name("test_func"));
        assert!(is_undescriptive_test_name("test_method"));
    }

    #[test]
    fn t109_is_undescriptive_just_test() {
        assert!(is_undescriptive_test_name("test"));
    }

    #[test]
    fn t109_is_descriptive_pass() {
        assert!(!is_undescriptive_test_name(
            "test_user_creation_returns_valid_id"
        ));
        assert!(!is_undescriptive_test_name("test_empty_input_raises_error"));
        assert!(!is_undescriptive_test_name("test_calculate_total_price"));
        assert!(!is_undescriptive_test_name("test_login"));
    }

    #[test]
    fn t109_typescript_string_names() {
        // Undescriptive
        assert!(is_undescriptive_test_name("works"));
        assert!(is_undescriptive_test_name("test"));
        assert!(is_undescriptive_test_name("it"));
        // Descriptive
        assert!(!is_undescriptive_test_name(
            "should calculate total price correctly"
        ));
        assert!(!is_undescriptive_test_name(
            "returns valid user when given valid credentials"
        ));
    }

    #[test]
    fn t109_produces_info_diagnostic() {
        let funcs = vec![make_func(
            "test_1",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t109: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T109"))
            .collect();
        assert_eq!(t109.len(), 1);
        assert_eq!(t109[0].severity, Severity::Info);
        assert!(t109[0].message.contains("undescriptive-test-name"));
    }

    #[test]
    fn t109_descriptive_name_no_diagnostic() {
        let funcs = vec![make_func(
            "test_user_creation_returns_valid_id",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t109: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T109"))
            .collect();
        assert!(t109.is_empty());
    }

    #[test]
    fn t109_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_1",
            TestAnalysis {
                assertion_count: 1,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T109")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        let t109: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T109"))
            .collect();
        assert!(t109.is_empty());
    }

    // --- T107: assertion-roulette ---

    #[test]
    fn t107_multiple_assertions_no_messages_produces_info() {
        let funcs = vec![make_func(
            "test_multiple_asserts_no_messages",
            TestAnalysis {
                assertion_count: 3,
                assertion_message_count: 0,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t107: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T107"))
            .collect();
        assert_eq!(t107.len(), 1);
        assert_eq!(t107[0].severity, Severity::Info);
        assert!(t107[0].message.contains("assertion-roulette"));
    }

    #[test]
    fn t107_single_assertion_no_diagnostic() {
        let funcs = vec![make_func(
            "test_single_assert_passes",
            TestAnalysis {
                assertion_count: 1,
                assertion_message_count: 0,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t107: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T107"))
            .collect();
        assert!(t107.is_empty());
    }

    #[test]
    fn t107_assertions_with_messages_no_diagnostic() {
        let funcs = vec![make_func(
            "test_asserts_with_messages_pass",
            TestAnalysis {
                assertion_count: 3,
                assertion_message_count: 3,
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t107: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T107"))
            .collect();
        assert!(t107.is_empty());
    }

    #[test]
    fn t107_partial_messages_no_diagnostic() {
        let funcs = vec![make_func(
            "test_partial_messages_still_pass",
            TestAnalysis {
                assertion_count: 3,
                assertion_message_count: 1, // at least some have messages
                ..Default::default()
            },
        )];
        let diags = evaluate_rules(&funcs, &Config::default());
        let t107: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T107"))
            .collect();
        assert!(t107.is_empty(), "partial messages should not trigger T107");
    }

    #[test]
    fn t107_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_multiple_asserts_disabled",
            TestAnalysis {
                assertion_count: 3,
                assertion_message_count: 0,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T107")],
            ..Config::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        let t107: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T107"))
            .collect();
        assert!(t107.is_empty());
    }

    // --- T106: duplicate-literal-assertion ---

    #[test]
    fn t106_duplicate_literal_produces_info() {
        let funcs = vec![make_func(
            "test_duplicate_literals",
            TestAnalysis {
                assertion_count: 4,
                duplicate_literal_count: 4,
                ..Default::default()
            },
        )];
        let config = Config::default(); // min_duplicate_count = 3
        let diags = evaluate_rules(&funcs, &config);
        let t106: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T106"))
            .collect();
        assert_eq!(t106.len(), 1);
        assert_eq!(t106[0].severity, Severity::Info);
        assert!(t106[0].message.contains("duplicate-literal-assertion"));
    }

    #[test]
    fn t106_below_threshold_no_diagnostic() {
        let funcs = vec![make_func(
            "test_few_duplicates",
            TestAnalysis {
                assertion_count: 3,
                duplicate_literal_count: 2,
                ..Default::default()
            },
        )];
        let config = Config::default();
        let diags = evaluate_rules(&funcs, &config);
        let t106: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T106"))
            .collect();
        assert!(t106.is_empty());
    }

    #[test]
    fn t106_at_threshold_produces_diagnostic() {
        let funcs = vec![make_func(
            "test_at_threshold",
            TestAnalysis {
                assertion_count: 3,
                duplicate_literal_count: 3,
                ..Default::default()
            },
        )];
        let config = Config::default(); // min_duplicate_count = 3
        let diags = evaluate_rules(&funcs, &config);
        let t106: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T106"))
            .collect();
        assert_eq!(t106.len(), 1);
    }

    #[test]
    fn t106_disabled_no_diagnostic() {
        let funcs = vec![make_func(
            "test_duplicate_literals",
            TestAnalysis {
                assertion_count: 4,
                duplicate_literal_count: 4,
                ..Default::default()
            },
        )];
        let config = Config {
            disabled_rules: vec![RuleId::new("T106")],
            ..Default::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        let t106: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T106"))
            .collect();
        assert!(t106.is_empty());
    }

    #[test]
    fn t106_custom_threshold() {
        let funcs = vec![make_func(
            "test_duplicate_literals",
            TestAnalysis {
                assertion_count: 4,
                duplicate_literal_count: 4,
                ..Default::default()
            },
        )];
        let config = Config {
            min_duplicate_count: 5,
            ..Default::default()
        };
        let diags = evaluate_rules(&funcs, &config);
        let t106: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == RuleId::new("T106"))
            .collect();
        assert!(t106.is_empty(), "count=4 with threshold=5 should not fire");
    }
}
