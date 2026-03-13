use serde::Serialize;

use crate::rules::{Diagnostic, RuleId, Severity};

const T001_HINT_THRESHOLD: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub rule: RuleId,
    pub title: String,
    pub message: String,
}

pub fn compute_hints(diagnostics: &[Diagnostic], custom_patterns_empty: bool) -> Vec<Hint> {
    if !custom_patterns_empty {
        return Vec::new();
    }

    let t001_block_count = diagnostics
        .iter()
        .filter(|d| d.rule.0 == "T001" && d.severity == Severity::Block)
        .count();

    if t001_block_count < T001_HINT_THRESHOLD {
        return Vec::new();
    }

    vec![Hint {
        rule: RuleId::new("T001"),
        title: "Assertion helper patterns may be missing".to_string(),
        message:
            "Add `[assertions] custom_patterns = [\"my_helper\"]` to `.exspec.toml` when your tests assert through helper functions."
                .to_string(),
    }]
}
