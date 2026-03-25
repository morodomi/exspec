//! Integration tests for PHP observe against a Laravel repository.
//!
//! All tests in this file require `/tmp/exspec-dogfood/laravel` to exist and are
//! marked `#[ignore]` so they are skipped in CI by default.  Run them with:
//!
//!   cargo test -p exspec-lang-php --test php_observe_laravel_test -- --ignored
//!
//! The tests invoke `cargo run --bin exspec -- observe --lang php --format json`
//! as a subprocess and parse the JSON output.
//!
//! Ground Truth: docs/observe-ground-truth-php-laravel.md (to be created)

use std::path::Path;
use std::process::Command;

const LARAVEL_REPO: &str = "/tmp/exspec-dogfood/laravel";

/// Run `exspec observe --lang php --format json <root>` and return the parsed
/// `serde_json::Value`.  The workspace manifest path is resolved relative to
/// this file at compile-time so the test does not depend on the working
/// directory.
fn run_observe_json(root: &str) -> serde_json::Value {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            manifest,
            "--bin",
            "exspec",
            "--",
            "observe",
            "--lang",
            "php",
            "--format",
            "json",
            root,
        ])
        .output()
        .expect("failed to execute cargo run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "failed to parse JSON output: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// Count the number of test files that appear in at least one mapping.
fn count_mapped_test_files(report: &serde_json::Value) -> usize {
    let mut mapped = std::collections::HashSet::new();
    if let Some(mappings) = report["file_mappings"].as_array() {
        for m in mappings {
            if let Some(test_files) = m["test_files"].as_array() {
                for tf in test_files {
                    if let Some(s) = tf.as_str() {
                        mapped.insert(s.to_string());
                    }
                }
            }
        }
    }
    mapped.len()
}

/// Count total test files discovered (mapped + unmapped).
fn count_total_test_files(report: &serde_json::Value) -> usize {
    report["summary"]["total_test_files"].as_u64().unwrap_or(0) as usize
}

// ---------------------------------------------------------------------------
// TC-04: Recall >= 85% after fixes
// ---------------------------------------------------------------------------
/// Given Laravel observe JSON output after is_non_sut_helper fixes,
/// when recall is computed over all discovered test files,
/// then recall >= 85%.
///
/// Baseline: 81.6% (before Fixtures/Stubs fix).
/// Target: >= 85% (intermediate milestone toward ship criteria of >= 90%).
#[test]
#[ignore]
fn tc04_recall_gte_85_percent() {
    // Given: Laravel repository exists
    assert!(
        Path::new(LARAVEL_REPO).exists(),
        "Laravel repo not found at {LARAVEL_REPO}. \
         Clone a Laravel app there before running this test."
    );

    // When: observe runs
    let report = run_observe_json(LARAVEL_REPO);

    // Then: recall >= 85%
    let total = count_total_test_files(&report);
    let mapped = count_mapped_test_files(&report);

    assert!(
        total > 0,
        "no test files discovered; check the Laravel repo path"
    );

    let recall = mapped as f64 / total as f64;
    assert!(
        recall >= 0.85,
        "recall {:.1}% ({}/{}) is below target 85%",
        recall * 100.0,
        mapped,
        total
    );
}

// ---------------------------------------------------------------------------
// TC-05: Regression — previously mapped test files remain mapped
// ---------------------------------------------------------------------------
/// Given Laravel observe JSON output after fixes,
/// when compared to the pre-fix baseline (>= 744 mapped),
/// then the number of mapped test files has not regressed.
///
/// Baseline: 744 files mapped on Laravel (81.6% of ~912 test files).
#[test]
#[ignore]
fn tc05_no_regression_vs_baseline() {
    // Given: Laravel repository exists
    assert!(
        Path::new(LARAVEL_REPO).exists(),
        "Laravel repo not found at {LARAVEL_REPO}"
    );

    // When: observe runs
    let report = run_observe_json(LARAVEL_REPO);

    // Then: mapped count >= baseline
    let mapped = count_mapped_test_files(&report);
    let baseline: usize = 744;

    assert!(
        mapped >= baseline,
        "mapped test files {mapped} regressed below baseline {baseline}"
    );
}
