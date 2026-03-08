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

fn ts_language() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&ts_language(), source).expect("invalid query"))
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

pub struct TypeScriptExtractor;

impl TypeScriptExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn parser() -> Parser {
        let mut parser = Parser::new();
        let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT;
        parser
            .set_language(&language.into())
            .expect("failed to load TypeScript grammar");
        parser
    }
}

impl Default for TypeScriptExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Count fixture variables from enclosing describe() scopes.
/// Walk up the AST from the test function, find describe callback bodies,
/// and count `lexical_declaration` / `variable_declaration` direct children.
/// Accumulates across all enclosing describes (handles nesting).
fn count_enclosing_describe_fixtures(root: Node, test_start_byte: usize, source: &[u8]) -> usize {
    let Some(start_node) = root.descendant_for_byte_range(test_start_byte, test_start_byte) else {
        return 0;
    };

    let mut count = 0;
    let mut current = start_node.parent();
    while let Some(node) = current {
        // Look for statement_block that is a describe callback body
        if node.kind() == "statement_block" && is_describe_callback_body(node, source) {
            // Count direct children that are variable declarations
            let child_count = node.named_child_count();
            for i in 0..child_count {
                if let Some(child) = node.named_child(i) {
                    let kind = child.kind();
                    if kind == "lexical_declaration" || kind == "variable_declaration" {
                        // Count the number of variable declarators in this declaration
                        // e.g., `let a, b;` has 2 declarators
                        let declarator_count = (0..child.named_child_count())
                            .filter_map(|j| child.named_child(j))
                            .filter(|c| c.kind() == "variable_declarator")
                            .count();
                        count += declarator_count;
                    }
                }
            }
        }
        current = node.parent();
    }

    count
}

/// Check if a statement_block is the body of a describe() callback.
/// Pattern: statement_block → arrow_function/function_expression → arguments → call_expression(describe)
fn is_describe_callback_body(block: Node, source: &[u8]) -> bool {
    let parent = match block.parent() {
        Some(p) => p,
        None => return false,
    };
    let kind = parent.kind();
    if kind != "arrow_function" && kind != "function_expression" {
        return false;
    }
    let args = match parent.parent() {
        Some(p) if p.kind() == "arguments" => p,
        _ => return false,
    };
    let call = match args.parent() {
        Some(p) if p.kind() == "call_expression" => p,
        _ => return false,
    };
    // Check if the function being called is "describe"
    if let Some(func_node) = call.child_by_field_name("function") {
        if let Ok(name) = func_node.utf8_text(source) {
            return name == "describe" || name.starts_with("describe.");
        }
    }
    false
}

fn extract_mock_class_name(var_name: &str) -> String {
    // camelCase: strip "mock" prefix and lowercase first char
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

    let name_idx = test_query
        .capture_index_for_name("name")
        .expect("no @name capture");
    let function_idx = test_query
        .capture_index_for_name("function")
        .expect("no @function capture");

    let source_bytes = source.as_bytes();

    let mut test_matches = Vec::new();
    {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(test_query, root, source_bytes);
        while let Some(m) = matches.next() {
            let name_capture = match m.captures.iter().find(|c| c.index == name_idx) {
                Some(c) => c,
                None => continue,
            };
            let name = match name_capture.node.utf8_text(source_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let fn_capture = match m.captures.iter().find(|c| c.index == function_idx) {
                Some(c) => c,
                None => continue,
            };

            test_matches.push(TestMatch {
                name,
                fn_start_byte: fn_capture.node.start_byte(),
                fn_end_byte: fn_capture.node.end_byte(),
                fn_start_row: fn_capture.node.start_position().row,
                fn_end_row: fn_capture.node.end_position().row,
            });
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

        let fixture_count = count_enclosing_describe_fixtures(root, tm.fn_start_byte, source_bytes);

        // T108: wait-and-see detection
        let has_wait = has_any_match(wait_query, "wait", fn_node, source_bytes);

        // T106: duplicate literal count
        let duplicate_literal_count = count_duplicate_literals(
            assertion_query,
            fn_node,
            source_bytes,
            &["number", "string"],
        );

        let suppressed_rules = extract_suppression_from_previous_line(source, tm.fn_start_row);

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
                assertion_message_count: assertion_count, // T107 skipped for TypeScript: expect() has no msg arg
                duplicate_literal_count,
                suppressed_rules,
            },
        });
    }

    functions
}

impl LanguageExtractor for TypeScriptExtractor {
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
            "{}/tests/fixtures/typescript/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/lang-typescript", ""),
            name
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    // --- Phase 1 preserved tests ---

    #[test]
    fn parse_typescript_source() {
        let source = "const x: number = 42;\n";
        let mut parser = TypeScriptExtractor::parser();
        let tree = parser.parse(source, None).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn typescript_extractor_implements_language_extractor() {
        let extractor = TypeScriptExtractor::new();
        let _: &dyn exspec_core::extractor::LanguageExtractor = &extractor;
    }

    // --- Cycle 1: Test function extraction ---

    #[test]
    fn extract_single_test_function() {
        let source = fixture("t001_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "create user");
        assert_eq!(funcs[0].line, 1);
    }

    #[test]
    fn extract_multiple_tests_excludes_helpers_and_describe() {
        let source = fixture("multiple_tests.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "multiple_tests.test.ts");
        assert_eq!(funcs.len(), 3);
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["adds numbers", "subtracts numbers", "multiplies numbers"]
        );
    }

    #[test]
    fn line_count_calculation() {
        let source = fixture("t001_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.test.ts");
        assert_eq!(
            funcs[0].analysis.line_count,
            funcs[0].end_line - funcs[0].line + 1
        );
    }

    #[test]
    fn violation_file_extracts_function() {
        let source = fixture("t001_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "create user");
    }

    // --- Cycle 2: Assertion detection ---

    #[test]
    fn assertion_count_zero_for_violation() {
        let source = fixture("t001_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation.test.ts");
        assert_eq!(funcs[0].analysis.assertion_count, 0);
    }

    #[test]
    fn assertion_count_positive_for_pass() {
        let source = fixture("t001_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.test.ts");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    // --- Cycle 2: Mock detection ---

    #[test]
    fn mock_count_for_violation() {
        let source = fixture("t002_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 6);
    }

    #[test]
    fn mock_count_for_pass() {
        let source = fixture("t002_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 1);
        assert_eq!(funcs[0].analysis.mock_classes, vec!["Db"]);
    }

    #[test]
    fn mock_class_name_extraction() {
        assert_eq!(extract_mock_class_name("mockDb"), "Db");
        assert_eq!(
            extract_mock_class_name("mockPaymentService"),
            "PaymentService"
        );
        assert_eq!(extract_mock_class_name("myMock"), "myMock");
    }

    // --- Inline suppression ---

    #[test]
    fn suppressed_test_has_suppressed_rules() {
        let source = fixture("suppressed.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "suppressed.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 6);
        assert!(funcs[0]
            .analysis
            .suppressed_rules
            .iter()
            .any(|r| r.0 == "T002"));
    }

    #[test]
    fn non_suppressed_test_has_empty_suppressed_rules() {
        let source = fixture("t002_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.test.ts");
        assert!(funcs[0].analysis.suppressed_rules.is_empty());
    }

    // --- Giant test ---

    #[test]
    fn giant_test_line_count() {
        let source = fixture("t003_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_violation.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count > 50);
    }

    #[test]
    fn short_test_line_count() {
        let source = fixture("t003_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count <= 50);
    }

    // --- File analysis: parameterized ---

    #[test]
    fn file_analysis_detects_parameterized() {
        let source = fixture("t004_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_pass.test.ts");
        assert!(
            fa.parameterized_count >= 1,
            "expected parameterized_count >= 1, got {}",
            fa.parameterized_count
        );
    }

    #[test]
    fn file_analysis_no_parameterized() {
        let source = fixture("t004_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_violation.test.ts");
        assert_eq!(fa.parameterized_count, 0);
    }

    // --- File analysis: PBT import ---

    #[test]
    fn file_analysis_detects_pbt_import() {
        let source = fixture("t005_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_pass.test.ts");
        assert!(fa.has_pbt_import);
    }

    #[test]
    fn file_analysis_no_pbt_import() {
        let source = fixture("t005_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_violation.test.ts");
        assert!(!fa.has_pbt_import);
    }

    // --- File analysis: contract import ---

    #[test]
    fn file_analysis_detects_contract_import() {
        let source = fixture("t008_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_pass.test.ts");
        assert!(fa.has_contract_import);
    }

    #[test]
    fn file_analysis_no_contract_import() {
        let source = fixture("t008_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_violation.test.ts");
        assert!(!fa.has_contract_import);
    }

    // --- Suppression does not propagate from describe to inner tests ---

    #[test]
    fn suppression_on_describe_does_not_apply_to_inner_tests() {
        let source = fixture("describe_suppression.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "describe_suppression.test.ts");
        assert_eq!(funcs.len(), 2, "expected 2 test functions inside describe");
        for f in &funcs {
            assert!(
                f.analysis.suppressed_rules.is_empty(),
                "test '{}' should NOT have suppressed rules (suppression on describe does not propagate)",
                f.name
            );
            assert_eq!(
                f.analysis.assertion_count, 0,
                "test '{}' should have 0 assertions (T001 violation expected)",
                f.name
            );
        }
    }

    // --- File analysis preserves functions ---

    #[test]
    fn file_analysis_preserves_test_functions() {
        let source = fixture("t001_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t001_pass.test.ts");
        assert_eq!(fa.functions.len(), 1);
        assert_eq!(fa.functions[0].name, "create user");
    }

    // --- T101: how-not-what ---

    #[test]
    fn how_not_what_count_for_violation() {
        let source = fixture("t101_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.test.ts");
        assert_eq!(funcs.len(), 2);
        assert!(
            funcs[0].analysis.how_not_what_count > 0,
            "expected how_not_what_count > 0 for first test, got {}",
            funcs[0].analysis.how_not_what_count
        );
        assert!(
            funcs[1].analysis.how_not_what_count > 0,
            "expected how_not_what_count > 0 for second test, got {}",
            funcs[1].analysis.how_not_what_count
        );
    }

    #[test]
    fn how_not_what_count_zero_for_pass() {
        let source = fixture("t101_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.how_not_what_count, 0);
    }

    #[test]
    fn how_not_what_coexists_with_assertions() {
        let source = fixture("t101_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.test.ts");
        assert!(
            funcs[0].analysis.assertion_count > 0,
            "should also count as assertions"
        );
        assert!(
            funcs[0].analysis.how_not_what_count > 0,
            "should count as how-not-what"
        );
    }

    // --- Query capture name verification (#14) ---

    fn make_query(scm: &str) -> Query {
        Query::new(&ts_language(), scm).unwrap()
    }

    #[test]
    fn query_capture_names_test_function() {
        let q = make_query(include_str!("../queries/test_function.scm"));
        assert!(
            q.capture_index_for_name("name").is_some(),
            "test_function.scm must define @name capture"
        );
        assert!(
            q.capture_index_for_name("function").is_some(),
            "test_function.scm must define @function capture"
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

    #[test]
    fn query_capture_names_import_contract() {
        let q = make_query(include_str!("../queries/import_contract.scm"));
        assert!(
            q.capture_index_for_name("contract_import").is_some(),
            "import_contract.scm must define @contract_import capture"
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

    // --- T102: fixture-sprawl ---

    #[test]
    fn fixture_count_for_violation() {
        let source = fixture("t102_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_violation.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 6,
            "expected 6 describe-level let declarations"
        );
    }

    #[test]
    fn fixture_count_for_pass() {
        let source = fixture("t102_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 2,
            "expected 2 describe-level let declarations"
        );
    }

    #[test]
    fn fixture_count_nested_describe() {
        let source = fixture("t102_nested.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_nested.test.ts");
        assert_eq!(funcs.len(), 2);
        // Inner test sees outer (3) + inner (3) = 6
        let inner = funcs
            .iter()
            .find(|f| f.name == "test in nested describe inherits all fixtures")
            .unwrap();
        assert_eq!(
            inner.analysis.fixture_count, 6,
            "inner test should see outer + inner fixtures"
        );
        // Outer test sees only outer (3)
        let outer = funcs
            .iter()
            .find(|f| f.name == "test in outer describe only sees outer fixtures")
            .unwrap();
        assert_eq!(
            outer.analysis.fixture_count, 3,
            "outer test should see only outer fixtures"
        );
    }

    #[test]
    fn fixture_count_describe_each() {
        let source = fixture("t102_describe_each.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_describe_each.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 2,
            "describe.each should be recognized as describe scope"
        );
    }

    #[test]
    fn fixture_count_top_level_test_zero() {
        // A test outside describe should have 0 fixtures
        let source = "it('standalone test', () => { expect(1).toBe(1); });";
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(source, "top_level.test.ts");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 0,
            "top-level test should have 0 fixtures"
        );
    }

    // --- T101: private attribute access in assertions (#13) ---

    #[test]
    fn private_dot_notation_detected() {
        let source = fixture("t101_private_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.test.ts");
        // "checks internal count via dot notation" has expect(service._count) and expect(service._processed)
        let func = funcs
            .iter()
            .find(|f| f.name == "checks internal count via dot notation")
            .unwrap();
        assert!(
            func.analysis.how_not_what_count >= 2,
            "expected >= 2 private access in assertions (dot), got {}",
            func.analysis.how_not_what_count
        );
    }

    #[test]
    fn private_bracket_notation_detected() {
        let source = fixture("t101_private_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.test.ts");
        // "checks internal via bracket notation" has expect(service['_count']) and expect(service['_processed'])
        let func = funcs
            .iter()
            .find(|f| f.name == "checks internal via bracket notation")
            .unwrap();
        assert!(
            func.analysis.how_not_what_count >= 2,
            "expected >= 2 private access in assertions (bracket), got {}",
            func.analysis.how_not_what_count
        );
    }

    #[test]
    fn private_outside_expect_not_counted() {
        let source = fixture("t101_private_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.test.ts");
        // "private outside expect not counted": service._internal is outside expect()
        let func = funcs
            .iter()
            .find(|f| f.name == "private outside expect not counted")
            .unwrap();
        assert_eq!(
            func.analysis.how_not_what_count, 0,
            "private access outside expect should not count"
        );
    }

    #[test]
    fn private_adds_to_how_not_what() {
        let source = fixture("t101_private_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.test.ts");
        // "mixed private and mock verification" has toHaveBeenCalledWith (mock) + expect(service._lastCreated) (private)
        let func = funcs
            .iter()
            .find(|f| f.name == "mixed private and mock verification")
            .unwrap();
        assert!(
            func.analysis.how_not_what_count >= 2,
            "expected mock (1) + private (1) = >= 2, got {}",
            func.analysis.how_not_what_count
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

    // --- T103: missing-error-test ---

    #[test]
    fn error_test_to_throw() {
        let source = fixture("t103_pass_toThrow.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_toThrow.test.ts");
        assert!(fa.has_error_test, ".toThrow() should set has_error_test");
    }

    #[test]
    fn error_test_to_throw_error() {
        let source = fixture("t103_pass_toThrowError.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_toThrowError.test.ts");
        assert!(
            fa.has_error_test,
            ".toThrowError() should set has_error_test"
        );
    }

    #[test]
    fn error_test_rejects() {
        let source = fixture("t103_pass_rejects.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_rejects.test.ts");
        assert!(fa.has_error_test, ".rejects should set has_error_test");
    }

    #[test]
    fn error_test_false_positive_rejects_property() {
        let source = fixture("t103_false_positive_rejects_property.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor
            .extract_file_analysis(&source, "t103_false_positive_rejects_property.test.ts");
        assert!(
            !fa.has_error_test,
            "service.rejects should NOT set has_error_test"
        );
    }

    #[test]
    fn error_test_no_patterns() {
        let source = fixture("t103_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_violation.test.ts");
        assert!(
            !fa.has_error_test,
            "no error patterns should set has_error_test=false"
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
    fn relational_assertion_violation() {
        let source = fixture("t105_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_violation.test.ts");
        assert!(
            !fa.has_relational_assertion,
            "all toBe/toEqual file should not have relational"
        );
    }

    #[test]
    fn relational_assertion_pass_greater_than() {
        let source = fixture("t105_pass_relational.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass_relational.test.ts");
        assert!(
            fa.has_relational_assertion,
            "toBeGreaterThan should set has_relational_assertion"
        );
    }

    #[test]
    fn relational_assertion_pass_truthy() {
        let source = fixture("t105_pass_truthy.test.ts");
        let extractor = TypeScriptExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass_truthy.test.ts");
        assert!(
            fa.has_relational_assertion,
            "toBeTruthy should set has_relational_assertion"
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

    // --- T108: wait-and-see ---

    #[test]
    fn wait_and_see_violation_sleep() {
        let source = fixture("t108_violation_sleep.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_violation_sleep.test.ts");
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
        let source = fixture("t108_pass_no_sleep.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_pass_no_sleep.test.ts");
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

    // --- T109: undescriptive-test-name ---

    #[test]
    fn t109_violation_names_detected() {
        let source = fixture("t109_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_violation.test.ts");
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
        let source = fixture("t109_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_pass.test.ts");
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
        let source = fixture("t106_violation.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_violation.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count >= 3,
            "42 appears 3 times, should be >= 3: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    #[test]
    fn t106_pass_no_duplicates() {
        let source = fixture("t106_pass_no_duplicates.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_pass_no_duplicates.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count < 3,
            "each literal appears once: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    // --- T001 FP fix: rejects chain + expectTypeOf (#25) ---

    #[test]
    fn t001_expect_to_throw_already_covered() {
        // TC-04: expect(fn).toThrow() -> already matched
        let source = "import { it, expect } from 'vitest';\nit('throws', () => { expect(() => fn()).toThrow(); });";
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_throw.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "expect().toThrow() should already be covered, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_rejects_to_throw_counts_as_assertion() {
        // TC-05: expect(promise).rejects.toThrow() -> T001 should NOT fire
        let source = fixture("t001_rejects_to_throw.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_rejects_to_throw.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "expect().rejects.toThrow() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_expect_type_of_counts_as_assertion() {
        // TC-06: expectTypeOf() -> T001 should NOT fire
        let source = fixture("t001_expect_type_of.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_expect_type_of.test.ts");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "expectTypeOf() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t107_skipped_for_typescript() {
        // TypeScript expect() has no message argument, so T107 should never fire.
        // assertion_message_count must equal assertion_count to prevent T107 from triggering.
        let source = fixture("t107_pass.test.ts");
        let extractor = TypeScriptExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_pass.test.ts");
        assert_eq!(funcs.len(), 1);
        let analysis = &funcs[0].analysis;
        assert!(
            analysis.assertion_count >= 2,
            "fixture should have 2+ assertions: got {}",
            analysis.assertion_count
        );
        assert_eq!(
            analysis.assertion_message_count, analysis.assertion_count,
            "TS assertion_message_count should equal assertion_count to skip T107"
        );
    }
}
