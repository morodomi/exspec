use std::sync::OnceLock;

use exspec_core::extractor::{FileAnalysis, LanguageExtractor, TestAnalysis, TestFunction};
use exspec_core::query_utils::{
    collect_mock_class_names, count_captures, count_captures_within_context,
    count_duplicate_literals, extract_suppression_from_previous_line, has_any_match,
};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const TEST_FUNCTION_QUERY: &str = include_str!("../queries/test_function.scm");
const ASSERTION_QUERY: &str = include_str!("../queries/assertion.scm");
const MOCK_USAGE_QUERY: &str = include_str!("../queries/mock_usage.scm");
const MOCK_ASSIGNMENT_QUERY: &str = include_str!("../queries/mock_assignment.scm");
const PARAMETERIZED_QUERY: &str = include_str!("../queries/parameterized.scm");
const IMPORT_PBT_QUERY: &str = include_str!("../queries/import_pbt.scm");
const IMPORT_CONTRACT_QUERY: &str = include_str!("../queries/import_contract.scm");
const HOW_NOT_WHAT_QUERY: &str = include_str!("../queries/how_not_what.scm");
const PRIVATE_IN_ASSERTION_QUERY: &str = include_str!("../queries/private_in_assertion.scm");
const ERROR_TEST_QUERY: &str = include_str!("../queries/error_test.scm");
const RELATIONAL_ASSERTION_QUERY: &str = include_str!("../queries/relational_assertion.scm");
const WAIT_AND_SEE_QUERY: &str = include_str!("../queries/wait_and_see.scm");

fn rust_language() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&rust_language(), source).expect("invalid query"))
}

static TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static MOCK_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static MOCK_ASSIGN_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static PARAMETERIZED_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static IMPORT_PBT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static IMPORT_CONTRACT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static HOW_NOT_WHAT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static PRIVATE_IN_ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static ERROR_TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static RELATIONAL_ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static WAIT_AND_SEE_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

pub struct RustExtractor;

impl RustExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn parser() -> Parser {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .expect("failed to load Rust grammar");
        parser
    }
}

impl Default for RustExtractor {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_mock_class_name(var_name: &str) -> String {
    // Rust uses snake_case: mock_service -> "service"
    if let Some(stripped) = var_name.strip_prefix("mock_") {
        if !stripped.is_empty() {
            return stripped.to_string();
        }
    }
    // camelCase: mockService -> "Service" (less common in Rust but handle it)
    if let Some(stripped) = var_name.strip_prefix("mock") {
        if !stripped.is_empty() && stripped.starts_with(|c: char| c.is_uppercase()) {
            return stripped.to_string();
        }
    }
    var_name.to_string()
}

struct TestMatch {
    name: String,
    fn_start_byte: usize,
    fn_end_byte: usize,
    fn_start_row: usize,
    fn_end_row: usize,
    /// Row of attribute_item (for suppression lookup)
    attr_start_row: usize,
}

/// Find the root object of a field_expression chain (method call chain).
/// e.g. `Config::builder().timeout(30).build()`:
///   call_expression { function: field_expression { value: call_expression { function: field_expression { value: call_expression { function: scoped_identifier } } } } }
///   → root call_expression's function is scoped_identifier
/// Check if a call_expression is a "constructor" (setup) or "method on local" (action).
/// Returns true for fixture-like calls: Type::new(), free_func(), builder chains from constructors.
/// Returns false for method calls on local variables: service.create(), result.unwrap().
fn is_constructor_call(node: Node) -> bool {
    let func = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return true, // conservative
    };
    match func.kind() {
        // Type::new(), Config::default() — constructor
        "scoped_identifier" => true,
        // add(1, 2), create_user() — free function call
        "identifier" => true,
        // obj.method() or chain.method() — need to find the root
        "field_expression" => {
            let value = match func.child_by_field_name("value") {
                Some(v) => v,
                None => return true,
            };
            if value.kind() == "call_expression" {
                // Chain: inner_call().method() — recurse to check inner call
                is_constructor_call(value)
            } else {
                // Root is a local variable: service.create(), result.unwrap()
                false
            }
        }
        _ => true,
    }
}

/// Check if a let value expression represents fixture/setup (not action/prep).
/// In tree-sitter-rust, `obj.method()` is `call_expression { function: field_expression }`.
/// Fixture: Type::new(), struct literals, macros, free function calls, builder chains from constructors.
/// Non-fixture: method calls on local variables (e.g. service.create(), result.unwrap()).
fn is_fixture_value(node: Node) -> bool {
    match node.kind() {
        "call_expression" => is_constructor_call(node),
        "struct_expression" | "macro_invocation" => true,
        _ => true, // literals, etc. are test data (fixture-like)
    }
}

/// Count Rust assertion macros that have a message argument.
/// assert!(expr, "msg") has 1+ top-level commas in token_tree.
/// assert_eq!(a, b, "msg") has 2+ top-level commas in token_tree.
fn count_assertion_messages_rust(assertion_query: &Query, fn_node: Node, source: &[u8]) -> usize {
    let assertion_idx = match assertion_query.capture_index_for_name("assertion") {
        Some(idx) => idx,
        None => return 0,
    };
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(assertion_query, fn_node, source);
    let mut count = 0;
    while let Some(m) = matches.next() {
        for cap in m.captures.iter().filter(|c| c.index == assertion_idx) {
            let node = cap.node;
            let macro_name = node
                .child_by_field_name("macro")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or("");

            // Find token_tree child
            let token_tree = (0..node.child_count()).find_map(|i| {
                let child = node.child(i)?;
                if child.kind() == "token_tree" {
                    Some(child)
                } else {
                    None
                }
            });

            if let Some(tt) = token_tree {
                // Count top-level commas in token_tree.
                // token_tree includes outer delimiters "(", ")".
                // Only count commas that are direct children of this token_tree
                // (not inside nested token_tree children).
                let mut comma_count = 0;
                for i in 0..tt.child_count() {
                    if let Some(child) = tt.child(i) {
                        if child.kind() == "," {
                            comma_count += 1;
                        }
                    }
                }

                // assert!(expr): needs 1+ comma for msg
                // assert_eq!(a, b): needs 2+ commas for msg
                let min_commas = if macro_name.contains("_eq") || macro_name.contains("_ne") {
                    2
                } else {
                    1
                };
                if comma_count >= min_commas {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Count fixture-like `let` declarations in a Rust function body.
/// Excludes method calls on local variables (action/assertion prep).
fn count_fixture_lets(fn_node: Node) -> usize {
    let body = match fn_node.child_by_field_name("body") {
        Some(n) => n,
        None => return 0,
    };

    let mut count = 0;
    let mut cursor = body.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "let_declaration" {
                match node.child_by_field_name("value") {
                    Some(value) => {
                        if is_fixture_value(value) {
                            count += 1;
                        }
                    }
                    None => count += 1, // `let x;` without value — count conservatively
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    count
}

fn extract_functions_from_tree(source: &str, file_path: &str, root: Node) -> Vec<TestFunction> {
    let test_query = cached_query(&TEST_QUERY_CACHE, TEST_FUNCTION_QUERY);
    let assertion_query = cached_query(&ASSERTION_QUERY_CACHE, ASSERTION_QUERY);
    let mock_query = cached_query(&MOCK_QUERY_CACHE, MOCK_USAGE_QUERY);
    let mock_assign_query = cached_query(&MOCK_ASSIGN_QUERY_CACHE, MOCK_ASSIGNMENT_QUERY);
    let how_not_what_query = cached_query(&HOW_NOT_WHAT_QUERY_CACHE, HOW_NOT_WHAT_QUERY);
    let private_query = cached_query(
        &PRIVATE_IN_ASSERTION_QUERY_CACHE,
        PRIVATE_IN_ASSERTION_QUERY,
    );
    let wait_query = cached_query(&WAIT_AND_SEE_QUERY_CACHE, WAIT_AND_SEE_QUERY);

    let source_bytes = source.as_bytes();

    // test_function.scm captures @test_attr (attribute_item).
    // The corresponding function_item is the next sibling of attribute_item.
    let attr_idx = test_query
        .capture_index_for_name("test_attr")
        .expect("no @test_attr capture");

    let mut test_matches: Vec<TestMatch> = Vec::new();
    let mut seen_fn_bytes: std::collections::HashSet<usize> = std::collections::HashSet::new();

    {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(test_query, root, source_bytes);
        while let Some(m) = matches.next() {
            let attr_capture = match m.captures.iter().find(|c| c.index == attr_idx) {
                Some(c) => c,
                None => continue,
            };
            let attr_node = attr_capture.node;
            let attr_start_row = attr_node.start_position().row;

            // Walk next siblings to find the function_item
            let mut sibling = attr_node.next_sibling();
            while let Some(s) = sibling {
                if s.kind() == "function_item" {
                    let fn_start_byte = s.start_byte();
                    if seen_fn_bytes.insert(fn_start_byte) {
                        let name = s
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source_bytes).ok())
                            .unwrap_or("")
                            .to_string();
                        if !name.is_empty() {
                            test_matches.push(TestMatch {
                                name,
                                fn_start_byte,
                                fn_end_byte: s.end_byte(),
                                fn_start_row: s.start_position().row,
                                fn_end_row: s.end_position().row,
                                attr_start_row,
                            });
                        }
                    }
                    break;
                }
                // Skip over other attribute_items or whitespace nodes
                // If we hit something that is not an attribute_item, stop
                if s.kind() != "attribute_item"
                    && s.kind() != "line_comment"
                    && s.kind() != "block_comment"
                {
                    break;
                }
                sibling = s.next_sibling();
            }
        }
    }

    let mut functions = Vec::new();
    for tm in &test_matches {
        let fn_node = match root.descendant_for_byte_range(tm.fn_start_byte, tm.fn_end_byte) {
            Some(n) => n,
            None => continue,
        };

        let line = tm.fn_start_row + 1;
        let end_line = tm.fn_end_row + 1;
        let line_count = end_line - line + 1;

        let assertion_count = count_captures(assertion_query, "assertion", fn_node, source_bytes);
        let mock_count = count_captures(mock_query, "mock", fn_node, source_bytes);
        let mock_classes = collect_mock_class_names(
            mock_assign_query,
            fn_node,
            source_bytes,
            extract_mock_class_name,
        );
        let how_not_what_count =
            count_captures(how_not_what_query, "how_pattern", fn_node, source_bytes);

        let private_in_assertion_count = count_captures_within_context(
            assertion_query,
            "assertion",
            private_query,
            "private_access",
            fn_node,
            source_bytes,
        );

        let fixture_count = count_fixture_lets(fn_node);

        // T108: wait-and-see detection
        let has_wait = has_any_match(wait_query, "wait", fn_node, source_bytes);

        // T107: assertion message count
        let assertion_message_count =
            count_assertion_messages_rust(assertion_query, fn_node, source_bytes);

        // T106: duplicate literal count
        let duplicate_literal_count = count_duplicate_literals(
            assertion_query,
            fn_node,
            source_bytes,
            &["integer_literal", "float_literal", "string_literal"],
        );

        // Suppression comment is the line before the attribute_item
        let suppressed_rules = extract_suppression_from_previous_line(source, tm.attr_start_row);

        functions.push(TestFunction {
            name: tm.name.clone(),
            file: file_path.to_string(),
            line,
            end_line,
            analysis: TestAnalysis {
                assertion_count,
                mock_count,
                mock_classes,
                line_count,
                how_not_what_count: how_not_what_count + private_in_assertion_count,
                fixture_count,
                has_wait,
                assertion_message_count,
                duplicate_literal_count,
                suppressed_rules,
            },
        });
    }

    functions
}

impl LanguageExtractor for RustExtractor {
    fn extract_test_functions(&self, source: &str, file_path: &str) -> Vec<TestFunction> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        extract_functions_from_tree(source, file_path, tree.root_node())
    }

    fn extract_file_analysis(&self, source: &str, file_path: &str) -> FileAnalysis {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => {
                return FileAnalysis {
                    file: file_path.to_string(),
                    functions: Vec::new(),
                    has_pbt_import: false,
                    has_contract_import: false,
                    has_error_test: false,
                    has_relational_assertion: false,
                    parameterized_count: 0,
                };
            }
        };

        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let functions = extract_functions_from_tree(source, file_path, root);

        let param_query = cached_query(&PARAMETERIZED_QUERY_CACHE, PARAMETERIZED_QUERY);
        let parameterized_count = count_captures(param_query, "parameterized", root, source_bytes);

        let pbt_query = cached_query(&IMPORT_PBT_QUERY_CACHE, IMPORT_PBT_QUERY);
        let has_pbt_import = has_any_match(pbt_query, "pbt_import", root, source_bytes);

        let contract_query = cached_query(&IMPORT_CONTRACT_QUERY_CACHE, IMPORT_CONTRACT_QUERY);
        let has_contract_import =
            has_any_match(contract_query, "contract_import", root, source_bytes);

        let error_test_query = cached_query(&ERROR_TEST_QUERY_CACHE, ERROR_TEST_QUERY);
        let has_error_test = has_any_match(error_test_query, "error_test", root, source_bytes);

        let relational_query = cached_query(
            &RELATIONAL_ASSERTION_QUERY_CACHE,
            RELATIONAL_ASSERTION_QUERY,
        );
        let has_relational_assertion =
            has_any_match(relational_query, "relational", root, source_bytes);

        FileAnalysis {
            file: file_path.to_string(),
            functions,
            has_pbt_import,
            has_contract_import,
            has_error_test,
            has_relational_assertion,
            parameterized_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/rust/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/lang-rust", ""),
            name
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    // --- Basic parser ---

    #[test]
    fn parse_rust_source() {
        let source = "#[test]\nfn test_example() {\n    assert_eq!(1, 1);\n}\n";
        let mut parser = RustExtractor::parser();
        let tree = parser.parse(source, None).unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn rust_extractor_implements_language_extractor() {
        let extractor = RustExtractor::new();
        let _: &dyn exspec_core::extractor::LanguageExtractor = &extractor;
    }

    // --- Test function extraction (TC-01, TC-02, TC-03) ---

    #[test]
    fn extract_single_test() {
        // TC-01: #[test] function is extracted
        let source = fixture("t001_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.rs");
        assert_eq!(funcs.len(), 1, "should extract exactly 1 test function");
        assert_eq!(funcs[0].name, "test_create_user");
    }

    #[test]
    fn non_test_function_not_extracted() {
        // TC-02: functions without #[test] are not extracted
        let source = "fn helper() -> i32 { 42 }\n";
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "helper.rs");
        assert_eq!(funcs.len(), 0, "non-test fn should not be extracted");
    }

    #[test]
    fn extract_tokio_test() {
        // TC-03: #[tokio::test] is extracted
        let source =
            "#[tokio::test]\nasync fn test_async_operation() {\n    assert_eq!(1, 1);\n}\n";
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "tokio_test.rs");
        assert_eq!(funcs.len(), 1, "should extract #[tokio::test] function");
        assert_eq!(funcs[0].name, "test_async_operation");
    }

    // --- Assertion detection (TC-04, TC-05, TC-06, TC-07) ---

    #[test]
    fn assertion_count_zero_for_violation() {
        // TC-04: assertion-free test has count 0
        let source = fixture("t001_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation.rs");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 0,
            "violation file should have 0 assertions"
        );
    }

    #[test]
    fn assertion_count_positive_for_pass() {
        // TC-05: assert_eq! is counted
        let source = fixture("t001_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pass file should have >= 1 assertion"
        );
    }

    #[test]
    fn all_assert_macros_counted() {
        // TC-06: assert!, assert_eq!, assert_ne! all counted
        let source = "#[test]\nfn test_all_asserts() {\n    assert!(true);\n    assert_eq!(1, 1);\n    assert_ne!(1, 2);\n}\n";
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_asserts.rs");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 3,
            "should count assert!, assert_eq!, assert_ne!"
        );
    }

    #[test]
    fn debug_assert_counted() {
        // TC-07: debug_assert! is also counted
        let source = "#[test]\nfn test_debug_assert() {\n    debug_assert!(true);\n}\n";
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_debug.rs");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 1,
            "debug_assert! should be counted"
        );
    }

    // --- Mock detection (TC-08, TC-09, TC-10, TC-11) ---

    #[test]
    fn mock_pattern_detected() {
        // TC-08: MockXxx::new() is detected
        let source = "#[test]\nfn test_with_mock() {\n    let mock_svc = MockService::new();\n    assert_eq!(mock_svc.len(), 0);\n}\n";
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_mock.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.mock_count >= 1,
            "MockService::new() should be detected"
        );
    }

    #[test]
    fn mock_count_for_violation() {
        // TC-09: mock_count > 5 triggers T002
        let source = fixture("t002_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.mock_count > 5,
            "violation file should have > 5 mocks, got {}",
            funcs[0].analysis.mock_count
        );
    }

    #[test]
    fn mock_count_for_pass() {
        // TC-10: mock_count <= 5 passes
        let source = fixture("t002_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_pass.rs");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.mock_count, 1,
            "pass file should have 1 mock"
        );
        assert_eq!(funcs[0].analysis.mock_classes, vec!["repo"]);
    }

    #[test]
    fn mock_class_name_extraction() {
        // TC-11: mock class name stripping
        assert_eq!(extract_mock_class_name("mock_service"), "service");
        assert_eq!(extract_mock_class_name("mock_db"), "db");
        assert_eq!(extract_mock_class_name("service"), "service");
        assert_eq!(extract_mock_class_name("mockService"), "Service");
    }

    // --- Giant test (TC-12, TC-13) ---

    #[test]
    fn giant_test_line_count() {
        // TC-12: > 50 lines triggers T003
        let source = fixture("t003_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_violation.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.line_count > 50,
            "violation file line_count should > 50, got {}",
            funcs[0].analysis.line_count
        );
    }

    #[test]
    fn short_test_line_count() {
        // TC-13: <= 50 lines passes
        let source = fixture("t003_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_pass.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.line_count <= 50,
            "pass file line_count should <= 50, got {}",
            funcs[0].analysis.line_count
        );
    }

    // --- File-level rules (TC-14, TC-15, TC-16, TC-17, TC-18) ---

    #[test]
    fn file_analysis_detects_parameterized() {
        // TC-14: #[rstest] detected
        let source = fixture("t004_pass.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_pass.rs");
        assert!(
            fa.parameterized_count >= 1,
            "should detect #[rstest], got {}",
            fa.parameterized_count
        );
    }

    #[test]
    fn file_analysis_no_parameterized() {
        // TC-15: no #[rstest] means parameterized_count = 0
        let source = fixture("t004_violation.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_violation.rs");
        assert_eq!(
            fa.parameterized_count, 0,
            "violation file should have 0 parameterized"
        );
    }

    #[test]
    fn file_analysis_pbt_import() {
        // TC-16: use proptest detected
        let source = fixture("t005_pass.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_pass.rs");
        assert!(fa.has_pbt_import, "should detect proptest import");
    }

    #[test]
    fn file_analysis_no_pbt_import() {
        // TC-17: no PBT import
        let source = fixture("t005_violation.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_violation.rs");
        assert!(!fa.has_pbt_import, "should not detect PBT import");
    }

    #[test]
    fn file_analysis_no_contract() {
        // TC-18: T008 always INFO for Rust (no contract library)
        let source = fixture("t008_violation.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_violation.rs");
        assert!(!fa.has_contract_import, "Rust has no contract library");
    }

    // --- prop_assert detection (#10) ---

    #[test]
    fn prop_assert_counts_as_assertion() {
        // #10: prop_assert_eq! should be counted as assertion
        let source = fixture("t001_proptest_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_proptest_pass.rs");
        assert_eq!(funcs.len(), 1, "should extract test from proptest! macro");
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "prop_assert_eq! should be counted, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    // --- Inline suppression (TC-19) ---

    #[test]
    fn suppressed_test_has_suppressed_rules() {
        // TC-19: // exspec-ignore: T001 suppresses T001
        let source = fixture("suppressed.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "suppressed.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0]
                .analysis
                .suppressed_rules
                .iter()
                .any(|r| r.0 == "T001"),
            "T001 should be suppressed, got: {:?}",
            funcs[0].analysis.suppressed_rules
        );
    }

    // --- Query capture name verification (#14) ---

    fn make_query(scm: &str) -> Query {
        Query::new(&rust_language(), scm).unwrap()
    }

    #[test]
    fn query_capture_names_test_function() {
        let q = make_query(include_str!("../queries/test_function.scm"));
        assert!(
            q.capture_index_for_name("test_attr").is_some(),
            "test_function.scm must define @test_attr capture"
        );
    }

    #[test]
    fn query_capture_names_assertion() {
        let q = make_query(include_str!("../queries/assertion.scm"));
        assert!(
            q.capture_index_for_name("assertion").is_some(),
            "assertion.scm must define @assertion capture"
        );
    }

    #[test]
    fn query_capture_names_mock_usage() {
        let q = make_query(include_str!("../queries/mock_usage.scm"));
        assert!(
            q.capture_index_for_name("mock").is_some(),
            "mock_usage.scm must define @mock capture"
        );
    }

    #[test]
    fn query_capture_names_mock_assignment() {
        let q = make_query(include_str!("../queries/mock_assignment.scm"));
        assert!(
            q.capture_index_for_name("var_name").is_some(),
            "mock_assignment.scm must define @var_name (required by collect_mock_class_names .expect())"
        );
    }

    #[test]
    fn query_capture_names_parameterized() {
        let q = make_query(include_str!("../queries/parameterized.scm"));
        assert!(
            q.capture_index_for_name("parameterized").is_some(),
            "parameterized.scm must define @parameterized capture"
        );
    }

    #[test]
    fn query_capture_names_import_pbt() {
        let q = make_query(include_str!("../queries/import_pbt.scm"));
        assert!(
            q.capture_index_for_name("pbt_import").is_some(),
            "import_pbt.scm must define @pbt_import capture"
        );
    }

    // Comment-only file by design (Rust has no contract validation library).
    // This assertion will fail when a real library is added.
    // When that happens, update the has_any_match call site in extract_file_analysis() accordingly.
    #[test]
    fn query_capture_names_import_contract_comment_only() {
        let q = make_query(include_str!("../queries/import_contract.scm"));
        assert!(
            q.capture_index_for_name("contract_import").is_none(),
            "Rust import_contract.scm is intentionally comment-only"
        );
    }

    // --- T103: missing-error-test ---

    #[test]
    fn error_test_should_panic() {
        let source = fixture("t103_pass.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass.rs");
        assert!(
            fa.has_error_test,
            "#[should_panic] should set has_error_test"
        );
    }

    #[test]
    fn error_test_unwrap_err() {
        let source = fixture("t103_pass_unwrap_err.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_unwrap_err.rs");
        assert!(fa.has_error_test, ".unwrap_err() should set has_error_test");
    }

    #[test]
    fn error_test_no_patterns() {
        let source = fixture("t103_violation.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_violation.rs");
        assert!(
            !fa.has_error_test,
            "no error patterns should set has_error_test=false"
        );
    }

    #[test]
    fn error_test_is_err_only_not_sufficient() {
        let source = fixture("t103_is_err_only.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_is_err_only.rs");
        assert!(
            !fa.has_error_test,
            ".is_err() alone should not count as error test (weak proxy)"
        );
    }

    #[test]
    fn query_capture_names_error_test() {
        let q = make_query(include_str!("../queries/error_test.scm"));
        assert!(
            q.capture_index_for_name("error_test").is_some(),
            "error_test.scm must define @error_test capture"
        );
    }

    // --- T105: deterministic-no-metamorphic ---

    #[test]
    fn relational_assertion_pass_contains() {
        let source = fixture("t105_pass.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass.rs");
        assert!(
            fa.has_relational_assertion,
            ".contains() should set has_relational_assertion"
        );
    }

    #[test]
    fn relational_assertion_violation() {
        let source = fixture("t105_violation.rs");
        let extractor = RustExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_violation.rs");
        assert!(
            !fa.has_relational_assertion,
            "only assert_eq! should not set has_relational_assertion"
        );
    }

    #[test]
    fn query_capture_names_relational_assertion() {
        let q = make_query(include_str!("../queries/relational_assertion.scm"));
        assert!(
            q.capture_index_for_name("relational").is_some(),
            "relational_assertion.scm must define @relational capture"
        );
    }

    // --- T101: how-not-what ---

    #[test]
    fn how_not_what_expect_method() {
        let source = fixture("t101_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.rs");
        assert!(
            funcs[0].analysis.how_not_what_count > 0,
            "mock.expect_save() should trigger how_not_what, got {}",
            funcs[0].analysis.how_not_what_count
        );
    }

    #[test]
    fn how_not_what_pass() {
        let source = fixture("t101_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_pass.rs");
        assert_eq!(
            funcs[0].analysis.how_not_what_count, 0,
            "no mock patterns should have how_not_what_count=0"
        );
    }

    #[test]
    fn how_not_what_private_field_limited_by_token_tree() {
        // Rust macro arguments are token_tree (not AST), so field_expression
        // with _name inside assert_eq!() is not detectable.
        // Private field access outside macros IS detected as field_expression,
        // but count_captures_within_context requires it to be inside an
        // assertion node (macro_invocation), which doesn't contain field_expression.
        let source = fixture("t101_private_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.rs");
        assert_eq!(
            funcs[0].analysis.how_not_what_count, 0,
            "Rust token_tree limitation: private field access in test is not detected"
        );
    }

    #[test]
    fn query_capture_names_how_not_what() {
        let q = make_query(include_str!("../queries/how_not_what.scm"));
        assert!(
            q.capture_index_for_name("how_pattern").is_some(),
            "how_not_what.scm must define @how_pattern capture"
        );
    }

    #[test]
    fn query_capture_names_private_in_assertion() {
        let q = make_query(include_str!("../queries/private_in_assertion.scm"));
        assert!(
            q.capture_index_for_name("private_access").is_some(),
            "private_in_assertion.scm must define @private_access capture"
        );
    }

    // --- T102: fixture-sprawl ---

    #[test]
    fn fixture_count_for_violation() {
        let source = fixture("t102_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_violation.rs");
        assert_eq!(
            funcs[0].analysis.fixture_count, 7,
            "expected 7 let bindings as fixture_count"
        );
    }

    #[test]
    fn fixture_count_for_pass() {
        let source = fixture("t102_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_pass.rs");
        assert_eq!(
            funcs[0].analysis.fixture_count, 1,
            "expected 1 let binding as fixture_count"
        );
    }

    #[test]
    fn fixture_count_excludes_method_calls_on_locals() {
        let source = fixture("t102_method_chain.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_method_chain.rs");
        assert_eq!(
            funcs[0].analysis.fixture_count, 6,
            "scoped calls (3) + struct (1) + macro (1) + builder chain (1) = 6, method calls on locals excluded"
        );
    }

    // --- T108: wait-and-see ---

    #[test]
    fn wait_and_see_violation_sleep() {
        let source = fixture("t108_violation_sleep.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_violation_sleep.rs");
        assert!(!funcs.is_empty());
        for func in &funcs {
            assert!(
                func.analysis.has_wait,
                "test '{}' should have has_wait=true",
                func.name
            );
        }
    }

    #[test]
    fn wait_and_see_pass_no_sleep() {
        let source = fixture("t108_pass_no_sleep.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_pass_no_sleep.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            !funcs[0].analysis.has_wait,
            "test without sleep should have has_wait=false"
        );
    }

    #[test]
    fn query_capture_names_wait_and_see() {
        let q = make_query(include_str!("../queries/wait_and_see.scm"));
        assert!(
            q.capture_index_for_name("wait").is_some(),
            "wait_and_see.scm must define @wait capture"
        );
    }

    // --- T107: assertion-roulette ---

    #[test]
    fn t107_violation_no_messages() {
        let source = fixture("t107_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_violation.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 2,
            "should have multiple assertions"
        );
        assert_eq!(
            funcs[0].analysis.assertion_message_count, 0,
            "no assertion should have a message"
        );
    }

    #[test]
    fn t107_pass_with_messages() {
        let source = fixture("t107_pass_with_messages.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_pass_with_messages.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_message_count >= 1,
            "assertions with messages should be counted"
        );
    }

    // --- T109: undescriptive-test-name ---

    #[test]
    fn t109_violation_names_detected() {
        let source = fixture("t109_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_violation.rs");
        assert!(!funcs.is_empty());
        for func in &funcs {
            assert!(
                exspec_core::rules::is_undescriptive_test_name(&func.name),
                "test '{}' should be undescriptive",
                func.name
            );
        }
    }

    #[test]
    fn t109_pass_descriptive_names() {
        let source = fixture("t109_pass.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_pass.rs");
        assert!(!funcs.is_empty());
        for func in &funcs {
            assert!(
                !exspec_core::rules::is_undescriptive_test_name(&func.name),
                "test '{}' should be descriptive",
                func.name
            );
        }
    }

    // --- T106: duplicate-literal-assertion ---

    #[test]
    fn t106_violation_duplicate_literal() {
        let source = fixture("t106_violation.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_violation.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count >= 3,
            "42 appears 3 times, should be >= 3: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    #[test]
    fn t106_pass_no_duplicates() {
        let source = fixture("t106_pass_no_duplicates.rs");
        let extractor = RustExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_pass_no_duplicates.rs");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count < 3,
            "each literal appears once: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }
}
