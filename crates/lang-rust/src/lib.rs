use std::collections::BTreeSet;
use std::sync::OnceLock;

use exspec_core::extractor::{FileAnalysis, LanguageExtractor, TestAnalysis, TestFunction};
use exspec_core::suppress::parse_suppression;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const TEST_FUNCTION_QUERY: &str = include_str!("../queries/test_function.scm");
const ASSERTION_QUERY: &str = include_str!("../queries/assertion.scm");
const MOCK_USAGE_QUERY: &str = include_str!("../queries/mock_usage.scm");
const MOCK_ASSIGNMENT_QUERY: &str = include_str!("../queries/mock_assignment.scm");
const PARAMETERIZED_QUERY: &str = include_str!("../queries/parameterized.scm");
const IMPORT_PBT_QUERY: &str = include_str!("../queries/import_pbt.scm");
const IMPORT_CONTRACT_QUERY: &str = include_str!("../queries/import_contract.scm");

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

fn count_captures(query: &Query, capture_name: &str, node: Node, source: &[u8]) -> usize {
    let idx = query
        .capture_index_for_name(capture_name)
        .expect("capture not found");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    let mut count = 0;
    while let Some(m) = matches.next() {
        count += m.captures.iter().filter(|c| c.index == idx).count();
    }
    count
}

fn has_any_match(query: &Query, capture_name: &str, node: Node, source: &[u8]) -> bool {
    let idx = match query.capture_index_for_name(capture_name) {
        Some(i) => i,
        None => return false,
    };
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    while let Some(m) = matches.next() {
        if m.captures.iter().any(|c| c.index == idx) {
            return true;
        }
    }
    false
}

fn collect_mock_class_names(query: &Query, node: Node, source: &[u8]) -> Vec<String> {
    let var_idx = query
        .capture_index_for_name("var_name")
        .expect("no @var_name capture");
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    let mut names = BTreeSet::new();
    while let Some(m) = matches.next() {
        for c in m.captures.iter().filter(|c| c.index == var_idx) {
            if let Ok(var) = c.node.utf8_text(source) {
                names.insert(extract_mock_class_name(var));
            }
        }
    }
    names.into_iter().collect()
}

fn extract_suppression_from_previous_line(
    source: &str,
    start_row: usize,
) -> Vec<exspec_core::rules::RuleId> {
    if start_row == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let prev_line = lines.get(start_row - 1).unwrap_or(&"");
    parse_suppression(prev_line)
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

fn extract_functions_from_tree(source: &str, file_path: &str, root: Node) -> Vec<TestFunction> {
    let test_query = cached_query(&TEST_QUERY_CACHE, TEST_FUNCTION_QUERY);
    let assertion_query = cached_query(&ASSERTION_QUERY_CACHE, ASSERTION_QUERY);
    let mock_query = cached_query(&MOCK_QUERY_CACHE, MOCK_USAGE_QUERY);
    let mock_assign_query = cached_query(&MOCK_ASSIGN_QUERY_CACHE, MOCK_ASSIGNMENT_QUERY);

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
        let mock_classes = collect_mock_class_names(mock_assign_query, fn_node, source_bytes);
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

        FileAnalysis {
            file: file_path.to_string(),
            functions,
            has_pbt_import,
            has_contract_import,
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
}
