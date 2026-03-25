//! Integration tests for Rust observe against the clap repository.
//!
//! All tests in this file require `/tmp/exspec-dogfood/clap/` to exist and are
//! marked `#[ignore]` so they are skipped in CI by default.  Run them with:
//!
//!   cargo test -p exspec-lang-rust --test rust_observe_clap_test -- --ignored
//!
//! The tests invoke `cargo run --bin exspec -- observe --lang rust --format json`
//! as a subprocess and parse the JSON output.
//!
//! Ground Truth: docs/observe-ground-truth-rust-clap.md
//! Commit: 70f3bb3

use std::path::Path;
use std::process::Command;

const CLAP_REPO: &str = "/tmp/exspec-dogfood/clap";

/// Run `exspec observe --lang rust --format json <root>` and return the parsed
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
            "rust",
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

/// Return all mappings whose `production_file` contains `needle`.
fn find_mappings_for_prod<'a>(
    report: &'a serde_json::Value,
    needle: &str,
) -> Vec<&'a serde_json::Value> {
    report["file_mappings"]
        .as_array()
        .expect("file_mappings must be array")
        .iter()
        .filter(|m| {
            m["production_file"]
                .as_str()
                .map(|p| p.contains(needle))
                .unwrap_or(false)
        })
        .collect()
}

/// Return all mappings that have a `test_files` entry containing `needle`.
fn find_mappings_for_test<'a>(
    report: &'a serde_json::Value,
    needle: &str,
) -> Vec<&'a serde_json::Value> {
    report["file_mappings"]
        .as_array()
        .expect("file_mappings must be array")
        .iter()
        .filter(|m| {
            m["test_files"]
                .as_array()
                .map(|tfs| {
                    tfs.iter()
                        .any(|tf| tf.as_str().map(|s| s.contains(needle)).unwrap_or(false))
                })
                .unwrap_or(false)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// TC-01: Precision >= 98%
// ---------------------------------------------------------------------------
/// Given clap observe JSON output, when compared to GT primary_targets,
/// then precision >= 98%.
///
/// GT (docs/observe-ground-truth-rust-clap.md, commit 70f3bb3):
/// - 13 unique external test files mapped by observe
/// - All 13 are TP (verified by import trace + filename audit)
/// - FP = 0, Precision = 100%
#[test]
#[ignore]
fn tc01_precision_gte_98_percent() {
    // Given: clap repository exists
    assert!(
        Path::new(CLAP_REPO).exists(),
        "clap repo not found at {CLAP_REPO}"
    );

    // When: observe runs
    let report = run_observe_json(CLAP_REPO);

    // Then: count TP and FP from GT-verified external mappings.
    // GT says all 13 mapped external test files are TP (no FP found).
    // All GT-verified (test_file_needle, prod_file_needle) pairs in one place.
    let gt_tp_pairs: &[(&str, &str)] = &[
        (
            "tests/builder/action.rs",
            "clap_builder/src/builder/action.rs",
        ),
        ("tests/builder/action.rs", "arg_predicate.rs"),
        ("tests/builder/action.rs", "error/kind.rs"),
        (
            "tests/builder/app_settings.rs",
            "clap_builder/src/builder/app_settings.rs",
        ),
        (
            "tests/builder/command.rs",
            "clap_builder/src/builder/command.rs",
        ),
        ("tests/builder/require.rs", "arg_predicate.rs"),
        ("tests/builder/default_vals.rs", "arg_predicate.rs"),
        ("tests/builder/default_vals.rs", "error/kind.rs"),
        (
            "tests/builder/tests.rs",
            "clap_builder/src/builder/tests.rs",
        ),
        (
            "tests/derive/flags.rs",
            "clap_builder/src/builder/value_parser.rs",
        ),
        ("tests/derive/utf8.rs", "clap_builder/src/error/kind.rs"),
        ("tests/derive/non_literal_attributes.rs", "error/kind.rs"),
        ("tests/derive/custom_string_parsers.rs", "parser/parser.rs"),
        (
            "tests/derive/doc_comments_help.rs",
            "clap_derive/src/utils/doc_comments.rs",
        ),
        (
            "tests/derive_ui/value_parser_unsupported.rs",
            "parser/parser.rs",
        ),
        (
            "clap_complete/tests/testsuite/engine.rs",
            "clap_complete/src/engine/candidate.rs",
        ),
        (
            "clap_complete/tests/testsuite/engine.rs",
            "engine/custom.rs",
        ),
    ];

    let file_mappings = report["file_mappings"]
        .as_array()
        .expect("file_mappings must be array");

    let mut tp = 0usize;
    let mut fp = 0usize;

    for mapping in file_mappings {
        let prod = mapping["production_file"].as_str().unwrap_or("");
        let test_files = mapping["test_files"].as_array();
        let Some(tfs) = test_files else { continue };

        for tf in tfs {
            let tf_str = tf.as_str().unwrap_or("");
            // Skip inline self-matches (src files with #[cfg(test)])
            if tf_str == prod
                || tf_str.contains("clap_builder/src/")
                || tf_str.contains("clap_complete/src/")
                || tf_str.contains("clap_mangen/src/")
            {
                continue;
            }
            let is_tp = gt_tp_pairs
                .iter()
                .any(|(t, p)| tf_str.contains(t) && prod.contains(p));
            if is_tp {
                tp += 1;
            } else {
                fp += 1;
                eprintln!("Potential FP: test={tf_str} -> prod={prod}");
            }
        }
    }

    let total = tp + fp;
    assert!(
        total > 0,
        "No external test mappings found -- observe produced no output"
    );

    let precision = tp as f64 / total as f64;
    assert!(
        precision >= 0.98,
        "Precision {:.1}% < 98%: TP={tp}, FP={fp}, total={total}",
        precision * 100.0
    );
}

// ---------------------------------------------------------------------------
// TC-02: Recall measurement (current baseline recorded)
// ---------------------------------------------------------------------------
/// Given clap GT test files, when observe maps them, then recall is measured
/// and the current baseline (~14.3%) is documented.
///
/// Note: R >= 90% is NOT asserted here because clap uses crate-root barrel
/// re-exports (`use clap::Arg`) which observe cannot resolve.  This is the
/// same hard-case pattern as tokio.  The assertion records the current recall
/// so regressions are detectable.
///
/// GT scope: 91 test files (see docs/observe-ground-truth-rust-clap.md)
/// TP: 13, FN: 78, Recall = 14.3%
#[test]
#[ignore]
fn tc02_recall_baseline_recorded() {
    // Given: clap repository exists
    assert!(
        Path::new(CLAP_REPO).exists(),
        "clap repo not found at {CLAP_REPO}"
    );

    // When: observe runs
    let report = run_observe_json(CLAP_REPO);

    // Then: measure recall against GT test files.
    // GT TP set: external test files that observe correctly maps.
    let gt_tp_test_files: &[&str] = &[
        "tests/builder/action.rs",
        "tests/builder/app_settings.rs",
        "tests/builder/require.rs",
        "tests/builder/default_vals.rs",
        "tests/builder/command.rs",
        "tests/builder/tests.rs",
        "tests/derive/flags.rs",
        "tests/derive/utf8.rs",
        "tests/derive/non_literal_attributes.rs",
        "tests/derive/custom_string_parsers.rs",
        "tests/derive/doc_comments_help.rs",
        "tests/derive_ui/value_parser_unsupported.rs",
        "clap_complete/tests/testsuite/engine.rs",
    ];

    // GT total scope: 91 files (45 builder + 32 derive + 3 top-level + 3 lex + 7 complete + 1 mangen)
    let gt_total: usize = 91;

    let file_mappings = report["file_mappings"]
        .as_array()
        .expect("file_mappings must be array");

    // Count which GT TP files are actually mapped
    let mut tp_found = 0usize;
    for &gt_file in gt_tp_test_files {
        let found = file_mappings.iter().any(|m| {
            m["test_files"]
                .as_array()
                .map(|tfs| {
                    tfs.iter()
                        .any(|tf| tf.as_str().map(|s| s.contains(gt_file)).unwrap_or(false))
                })
                .unwrap_or(false)
        });
        if found {
            tp_found += 1;
        }
    }

    let recall = tp_found as f64 / gt_total as f64;

    // Regression guard: recall must not drop below the established baseline.
    // Current baseline: 14.3% (13/91). Threshold = 9% (baseline minus ~5pp tolerance
    // for measurement variance from observe improvements or clap repo changes).
    assert!(
        recall >= 0.09,
        "Recall {:.1}% dropped below 9% regression threshold. \
         TP found={tp_found}, GT total={gt_total}. \
         See docs/observe-ground-truth-rust-clap.md for FN root causes.",
        recall * 100.0
    );

    // Document current measurement (informational)
    eprintln!(
        "clap Recall: {:.1}% (TP={tp_found}/{gt_total}). \
         Known FN root causes: crate root barrel re-export (~65), \
         derive macro barrel (~25), automod::dir! (3). \
         R >= 90% ship criterion NOT met -- clap is a hard case.",
        recall * 100.0
    );
}

// ---------------------------------------------------------------------------
// TC-03: tests/builder/action.rs -> clap_builder/src/builder/action.rs
// ---------------------------------------------------------------------------
/// Given tests/builder/action.rs, when observe runs, then it maps to
/// clap_builder/src/builder/action.rs.
///
/// Note: the strategy is "import" (L2), not "filename" (L1). The L2 resolver
/// picks up `use clap::builder::ArgPredicate` and traces it to action.rs via
/// the explicit builder:: path. This is correct behaviour.
#[test]
#[ignore]
fn tc03_builder_action_mapped_via_import() {
    // Given: clap repository exists
    assert!(
        Path::new(CLAP_REPO).exists(),
        "clap repo not found at {CLAP_REPO}"
    );

    // When: observe runs
    let report = run_observe_json(CLAP_REPO);

    // Then: clap_builder/src/builder/action.rs appears in file_mappings and
    //       tests/builder/action.rs is in its test_files list
    let mappings = find_mappings_for_prod(&report, "clap_builder/src/builder/action.rs");
    assert!(
        !mappings.is_empty(),
        "clap_builder/src/builder/action.rs not found in file_mappings. \
         Full mappings: {:#?}",
        report["file_mappings"]
    );

    let mapping = mappings[0];
    let test_files = mapping["test_files"]
        .as_array()
        .expect("test_files must be array");
    let matched = test_files.iter().any(|tf| {
        tf.as_str()
            .map(|s| s.contains("tests/builder/action"))
            .unwrap_or(false)
    });
    assert!(
        matched,
        "tests/builder/action.rs not in test_files for clap_builder/src/builder/action.rs. \
         Got: {:?}",
        test_files
    );

    // Strategy is "import" (L2): observe traces `use clap::builder::ArgAction`
    // through the explicit builder:: namespace path to action.rs.
    let strategy = mapping["strategy"].as_str().unwrap_or("");
    assert_eq!(
        strategy, "import",
        "Expected import strategy for action.rs, got: {strategy:?}"
    );
}

// ---------------------------------------------------------------------------
// TC-04: tests/derive/basic.rs is a known FN (derive macro barrel)
// ---------------------------------------------------------------------------
/// Given tests/derive/basic.rs, when observe runs, then observe does NOT map
/// it to clap_derive/src/ -- this is a known FN.
///
/// Root cause: `use clap::Parser` routes through the crate root barrel and
/// into the clap_derive proc-macro crate. observe cannot resolve cross-crate
/// re-exports through derive macros.
///
/// When observe is improved to handle this, update this test to assert the
/// correct mapping.
#[test]
#[ignore]
fn tc04_derive_basic_is_known_fn_derive_macro_barrel() {
    // Given: clap repository exists and tests/derive/basic.rs is present
    assert!(
        Path::new(CLAP_REPO).exists(),
        "clap repo not found at {CLAP_REPO}"
    );
    assert!(
        Path::new(CLAP_REPO).join("tests/derive/basic.rs").exists(),
        "tests/derive/basic.rs not found in clap repo"
    );

    // When: observe runs
    let report = run_observe_json(CLAP_REPO);

    // Then: tests/derive/basic.rs is NOT mapped to clap_derive/src/ (known FN)
    let mappings = find_mappings_for_test(&report, "tests/derive/basic.rs");
    let derive_mappings: Vec<_> = mappings
        .iter()
        .filter(|m| {
            m["production_file"]
                .as_str()
                .map(|p| p.contains("clap_derive/src"))
                .unwrap_or(false)
        })
        .collect();

    // Currently a known FN: derive macro barrel import not resolved.
    // If this assertion fails, observe has been improved -- update test to
    // assert the correct mapping (clap_derive/src/derives/parser.rs).
    assert!(
        derive_mappings.is_empty(),
        "GOOD NEWS: tests/derive/basic.rs is now mapped to clap_derive/src/! \
         Update this test to assert the correct mapping \
         (expected: clap_derive/src/derives/parser.rs). \
         Got: {derive_mappings:#?}"
    );
}

// ---------------------------------------------------------------------------
// TC-05: clap_lex/tests/testsuite/lexer.rs is a known FN (automod::dir!)
// ---------------------------------------------------------------------------
/// Given clap_lex/tests/testsuite/lexer.rs, when observe runs, then observe
/// does NOT map it to clap_lex/src/lib.rs -- this is a known FN.
///
/// Root cause: clap_lex uses `automod::dir!("tests/testsuite")` for dynamic
/// module discovery. Static AST analysis cannot resolve this macro; the test
/// files under tests/testsuite/ are never discovered as test modules.
///
/// When observe is improved to handle automod, update this test.
#[test]
#[ignore]
fn tc05_clap_lex_lexer_is_known_fn_automod() {
    // Given: clap repository exists and the testsuite/lexer.rs is present
    assert!(
        Path::new(CLAP_REPO).exists(),
        "clap repo not found at {CLAP_REPO}"
    );
    assert!(
        Path::new(CLAP_REPO)
            .join("clap_lex/tests/testsuite/lexer.rs")
            .exists(),
        "clap_lex/tests/testsuite/lexer.rs not found in clap repo"
    );

    // When: observe runs
    let report = run_observe_json(CLAP_REPO);

    // Then: clap_lex/src/lib.rs is NOT mapped with lexer.rs in test_files (known FN)
    let mappings = find_mappings_for_prod(&report, "clap_lex/src/lib.rs");

    // Either no mapping exists for clap_lex/src/lib.rs, or it exists but without lexer.rs
    let lexer_mapped = mappings.iter().any(|m| {
        m["test_files"]
            .as_array()
            .map(|tfs| {
                tfs.iter().any(|tf| {
                    tf.as_str()
                        .map(|s| s.contains("testsuite/lexer"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    // Currently a known FN: automod::dir!() prevents static discovery.
    // If this assertion fails, observe has been improved -- update test to
    // assert the correct mapping (clap_lex/src/lib.rs).
    assert!(
        !lexer_mapped,
        "GOOD NEWS: clap_lex/tests/testsuite/lexer.rs is now mapped to clap_lex/src/lib.rs! \
         Update this test to assert the correct mapping. \
         Got mappings: {mappings:#?}"
    );
}
