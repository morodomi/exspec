use std::str::FromStr;

use serde::Deserialize;

use crate::rules::{Config, RuleId, Severity};

#[derive(Debug, Deserialize, Default)]
pub struct ExspecConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub thresholds: ThresholdsConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub assertions: AssertionsConfig,
    #[serde(default)]
    pub output: OutputConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct OutputConfig {
    pub min_severity: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AssertionsConfig {
    #[serde(default)]
    pub custom_patterns: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub lang: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RulesConfig {
    #[serde(default)]
    pub disable: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ThresholdsConfig {
    pub mock_max: Option<usize>,
    pub mock_class_max: Option<usize>,
    pub test_max_lines: Option<usize>,
    pub parameterized_min_ratio: Option<f64>,
    pub fixture_max: Option<usize>,
    pub min_assertions_for_t105: Option<usize>,
    pub min_duplicate_count: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PathsConfig {
    #[serde(default)]
    pub test_patterns: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl ExspecConfig {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }
}

impl From<ExspecConfig> for Config {
    fn from(ec: ExspecConfig) -> Self {
        let defaults = Config::default();
        Config {
            mock_max: ec.thresholds.mock_max.unwrap_or(defaults.mock_max),
            mock_class_max: ec
                .thresholds
                .mock_class_max
                .unwrap_or(defaults.mock_class_max),
            test_max_lines: ec
                .thresholds
                .test_max_lines
                .unwrap_or(defaults.test_max_lines),
            parameterized_min_ratio: ec
                .thresholds
                .parameterized_min_ratio
                .filter(|v| v.is_finite())
                .unwrap_or(defaults.parameterized_min_ratio)
                .clamp(0.0, 1.0),
            fixture_max: ec.thresholds.fixture_max.unwrap_or(defaults.fixture_max),
            min_assertions_for_t105: ec
                .thresholds
                .min_assertions_for_t105
                .unwrap_or(defaults.min_assertions_for_t105),
            min_duplicate_count: ec
                .thresholds
                .min_duplicate_count
                .unwrap_or(defaults.min_duplicate_count),
            disabled_rules: ec.rules.disable.iter().map(|s| RuleId::new(s)).collect(),
            custom_assertion_patterns: ec.assertions.custom_patterns,
            ignore_patterns: ec.paths.ignore,
            min_severity: ec
                .output
                .min_severity
                .as_deref()
                .map(|s| {
                    Severity::from_str(s).unwrap_or_else(|_| {
                        eprintln!("warning: invalid min_severity '{s}', using default");
                        defaults.min_severity
                    })
                })
                .unwrap_or(defaults.min_severity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/config/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/core", ""),
            name,
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    #[test]
    fn parse_valid_config() {
        let content = fixture("valid.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert_eq!(ec.general.lang, vec!["python", "typescript"]);
        assert_eq!(ec.rules.disable, vec!["T004", "T005"]);
        assert_eq!(ec.thresholds.mock_max, Some(10));
        assert_eq!(ec.thresholds.mock_class_max, Some(5));
        assert_eq!(ec.thresholds.test_max_lines, Some(100));
        assert_eq!(ec.thresholds.parameterized_min_ratio, Some(0.2));
        assert_eq!(ec.thresholds.fixture_max, Some(10));
        assert_eq!(ec.thresholds.min_assertions_for_t105, Some(8));
        assert_eq!(ec.thresholds.min_duplicate_count, Some(4));
        assert_eq!(ec.paths.test_patterns, vec!["tests/**", "**/*_test.*"]);
        assert_eq!(ec.paths.ignore, vec!["node_modules", ".venv"]);
    }

    #[test]
    fn parse_partial_config() {
        let content = fixture("partial.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert_eq!(ec.thresholds.mock_max, Some(8));
        assert_eq!(ec.thresholds.mock_class_max, None);
        assert!(ec.rules.disable.is_empty());
    }

    #[test]
    fn parse_empty_config() {
        let content = fixture("empty.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert!(ec.general.lang.is_empty());
        assert!(ec.rules.disable.is_empty());
        assert_eq!(ec.thresholds.mock_max, None);
    }

    #[test]
    fn parse_invalid_config_returns_error() {
        let content = fixture("invalid.toml");
        let result = ExspecConfig::from_toml(&content);
        assert!(result.is_err());
    }

    #[test]
    fn convert_full_config_to_rules_config() {
        let content = fixture("valid.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        assert_eq!(config.mock_max, 10);
        assert_eq!(config.mock_class_max, 5);
        assert_eq!(config.test_max_lines, 100);
        assert_eq!(config.parameterized_min_ratio, 0.2);
        assert_eq!(config.fixture_max, 10);
        assert_eq!(config.min_assertions_for_t105, 8);
        assert_eq!(config.min_duplicate_count, 4);
        assert_eq!(config.disabled_rules.len(), 2);
        assert_eq!(config.disabled_rules[0].0, "T004");
        assert_eq!(config.disabled_rules[1].0, "T005");
    }

    #[test]
    fn convert_partial_config_uses_defaults() {
        let content = fixture("partial.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        let defaults = Config::default();
        assert_eq!(config.mock_max, 8);
        assert_eq!(config.mock_class_max, defaults.mock_class_max);
        assert_eq!(config.test_max_lines, defaults.test_max_lines);
        assert_eq!(
            config.parameterized_min_ratio,
            defaults.parameterized_min_ratio
        );
        assert!(config.disabled_rules.is_empty());
    }

    #[test]
    fn convert_negative_ratio_clamped_to_zero() {
        let ec = ExspecConfig {
            thresholds: ThresholdsConfig {
                parameterized_min_ratio: Some(-0.5),
                ..Default::default()
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.parameterized_min_ratio, 0.0);
    }

    #[test]
    fn convert_zero_ratio_stays_zero() {
        let ec = ExspecConfig {
            thresholds: ThresholdsConfig {
                parameterized_min_ratio: Some(0.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.parameterized_min_ratio, 0.0);
    }

    #[test]
    fn convert_positive_ratio_unchanged() {
        let ec = ExspecConfig {
            thresholds: ThresholdsConfig {
                parameterized_min_ratio: Some(0.3),
                ..Default::default()
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.parameterized_min_ratio, 0.3);
    }

    #[test]
    fn convert_ratio_above_one_clamped_to_one() {
        let ec = ExspecConfig {
            thresholds: ThresholdsConfig {
                parameterized_min_ratio: Some(1.5),
                ..Default::default()
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.parameterized_min_ratio, 1.0);
    }

    #[test]
    fn convert_nan_ratio_falls_back_to_default() {
        let content = fixture("nan_ratio.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        let defaults = Config::default();
        assert_eq!(
            config.parameterized_min_ratio, defaults.parameterized_min_ratio,
            "NaN should fall back to default"
        );
    }

    #[test]
    fn convert_inf_ratio_falls_back_to_default() {
        let content = fixture("inf_ratio.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        let defaults = Config::default();
        assert_eq!(
            config.parameterized_min_ratio, defaults.parameterized_min_ratio,
            "Inf should fall back to default"
        );
    }

    #[test]
    fn convert_neg_inf_ratio_falls_back_to_default() {
        let content = fixture("neg_inf_ratio.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        let defaults = Config::default();
        assert_eq!(
            config.parameterized_min_ratio, defaults.parameterized_min_ratio,
            "-Inf should fall back to default"
        );
    }

    // --- TC-01: custom_patterns populated from toml ---
    #[test]
    fn parse_custom_assertions_config() {
        let content = fixture("custom_assertions.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert_eq!(
            ec.assertions.custom_patterns,
            vec!["util.assertEqual(", "myAssert(", "customCheck("]
        );
    }

    // --- TC-02: missing [assertions] section -> empty vec ---
    #[test]
    fn parse_config_without_assertions_section() {
        let content = fixture("valid.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert!(ec.assertions.custom_patterns.is_empty());
    }

    // --- TC-03: ExspecConfig -> Config preserves custom_assertion_patterns ---
    #[test]
    fn convert_config_preserves_custom_assertion_patterns() {
        let ec = ExspecConfig {
            assertions: AssertionsConfig {
                custom_patterns: vec!["myAssert(".to_string()],
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.custom_assertion_patterns, vec!["myAssert("]);
    }

    #[test]
    fn convert_config_empty_assertions_gives_empty_patterns() {
        let ec = ExspecConfig::default();
        let config: Config = ec.into();
        assert!(config.custom_assertion_patterns.is_empty());
    }

    // --- TC: ignore_patterns propagated from ExspecConfig ---
    #[test]
    fn convert_config_propagates_ignore_patterns() {
        let content = fixture("valid.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        assert_eq!(config.ignore_patterns, vec!["node_modules", ".venv"]);
    }

    #[test]
    fn convert_config_empty_ignore_gives_empty_patterns() {
        let ec = ExspecConfig::default();
        let config: Config = ec.into();
        assert!(config.ignore_patterns.is_empty());
    }

    // --- #59: OutputConfig parsing ---

    #[test]
    fn parse_output_min_severity() {
        let content = fixture("min_severity.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert_eq!(ec.output.min_severity, Some("warn".to_string()));
    }

    #[test]
    fn parse_config_without_output_section() {
        let content = fixture("empty.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        assert_eq!(ec.output.min_severity, None);
    }

    #[test]
    fn convert_output_min_severity_block() {
        let ec = ExspecConfig {
            output: OutputConfig {
                min_severity: Some("BLOCK".to_string()),
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.min_severity, Severity::Block);
    }

    #[test]
    fn convert_no_min_severity_defaults_to_info() {
        let ec = ExspecConfig::default();
        let config: Config = ec.into();
        assert_eq!(config.min_severity, Severity::Info);
    }

    #[test]
    fn convert_invalid_min_severity_string_falls_back_to_info() {
        let ec = ExspecConfig {
            output: OutputConfig {
                min_severity: Some("BLOKC".to_string()),
            },
            ..Default::default()
        };
        let config: Config = ec.into();
        assert_eq!(config.min_severity, Severity::Info);
    }

    #[test]
    fn convert_empty_config_all_defaults() {
        let content = fixture("empty.toml");
        let ec = ExspecConfig::from_toml(&content).unwrap();
        let config: Config = ec.into();
        let defaults = Config::default();
        assert_eq!(config.mock_max, defaults.mock_max);
        assert_eq!(config.mock_class_max, defaults.mock_class_max);
        assert_eq!(config.test_max_lines, defaults.test_max_lines);
        assert_eq!(
            config.parameterized_min_ratio,
            defaults.parameterized_min_ratio
        );
        assert!(config.disabled_rules.is_empty());
    }
}
