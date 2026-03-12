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
const SKIP_TEST_QUERY: &str = include_str!("../queries/skip_test.scm");

fn python_language() -> tree_sitter::Language {
    tree_sitter_python::LANGUAGE.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&python_language(), source).expect("invalid query"))
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
static SKIP_TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

pub struct PythonExtractor;

impl PythonExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn parser() -> Parser {
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE;
        parser
            .set_language(&language.into())
            .expect("failed to load Python grammar");
        parser
    }
}

impl Default for PythonExtractor {
    fn default() -> Self {
        Self::new()
    }
}

struct TestMatch {
    name: String,
    dedup_id: usize,
    fn_start_byte: usize,
    fn_end_byte: usize,
    fn_start_row: usize,
    fn_end_row: usize,
    decorated_start_byte: Option<usize>,
    decorated_end_byte: Option<usize>,
    decorated_start_row: Option<usize>,
}

fn is_in_non_test_class(root: Node, start_byte: usize, end_byte: usize, source: &[u8]) -> bool {
    let Some(node) = root.descendant_for_byte_range(start_byte, end_byte) else {
        return false;
    };
    // Walk all ancestors, record the outermost class name
    let mut outermost_class_name: Option<String> = None;
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_definition" {
            if let Some(name_node) = parent.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    outermost_class_name = Some(name.to_string());
                }
            } else {
                outermost_class_name = Some(String::new());
            }
        }
        current = parent.parent();
    }
    match outermost_class_name {
        None => false, // module-level
        Some(name) => !name.starts_with("Test") && !name.starts_with("test_"),
    }
}

fn is_pytest_fixture_decorator(decorated_node: Node, source: &[u8]) -> bool {
    let mut cursor = decorated_node.walk();
    for child in decorated_node.children(&mut cursor) {
        if child.kind() != "decorator" {
            continue;
        }
        let Ok(text) = child.utf8_text(source) else {
            continue;
        };
        // Strip leading '@' and trailing '(...)' or whitespace to get the decorator name
        let trimmed = text.trim_start_matches('@');
        let name = trimmed.split('(').next().unwrap_or("").trim();
        if name == "pytest.fixture" || name == "fixture" {
            return true;
        }
    }
    false
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
    let skip_query = cached_query(&SKIP_TEST_QUERY_CACHE, SKIP_TEST_QUERY);

    let name_idx = test_query
        .capture_index_for_name("name")
        .expect("no @name capture");
    let function_idx = test_query
        .capture_index_for_name("function")
        .expect("no @function capture");
    let decorated_idx = test_query
        .capture_index_for_name("decorated")
        .expect("no @decorated capture");

    let source_bytes = source.as_bytes();

    let mut test_matches = Vec::new();
    let mut decorated_fn_ids = std::collections::HashSet::new();
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

            let decorated_capture = m.captures.iter().find(|c| c.index == decorated_idx);
            let fn_capture = m.captures.iter().find(|c| c.index == function_idx);

            if let Some(dec) = decorated_capture {
                let inner_fn = dec
                    .node
                    .child_by_field_name("definition")
                    .unwrap_or(dec.node);
                // Always register the inner function as "has a decorated match" so the
                // dedup retain step removes any bare @function match for the same node.
                decorated_fn_ids.insert(inner_fn.id());
                // Skip @pytest.fixture / @fixture decorated functions — they are
                // test data providers, not test functions (prevents T001 FPs).
                if is_pytest_fixture_decorator(dec.node, source_bytes) {
                    continue;
                }
                test_matches.push(TestMatch {
                    name,
                    dedup_id: inner_fn.id(),
                    fn_start_byte: inner_fn.start_byte(),
                    fn_end_byte: inner_fn.end_byte(),
                    fn_start_row: inner_fn.start_position().row,
                    fn_end_row: inner_fn.end_position().row,
                    decorated_start_byte: Some(dec.node.start_byte()),
                    decorated_end_byte: Some(dec.node.end_byte()),
                    decorated_start_row: Some(dec.node.start_position().row),
                });
            } else if let Some(fn_c) = fn_capture {
                test_matches.push(TestMatch {
                    name,
                    dedup_id: fn_c.node.id(),
                    fn_start_byte: fn_c.node.start_byte(),
                    fn_end_byte: fn_c.node.end_byte(),
                    fn_start_row: fn_c.node.start_position().row,
                    fn_end_row: fn_c.node.end_position().row,
                    decorated_start_byte: None,
                    decorated_end_byte: None,
                    decorated_start_row: None,
                });
            }
        }
    }

    test_matches
        .retain(|tm| tm.decorated_start_byte.is_some() || !decorated_fn_ids.contains(&tm.dedup_id));

    // Filter out methods in non-test classes (e.g., UserService.test_connection)
    test_matches.retain(|tm| {
        let check_byte = tm.decorated_start_byte.unwrap_or(tm.fn_start_byte);
        let check_end = tm.decorated_end_byte.unwrap_or(tm.fn_end_byte);
        !is_in_non_test_class(root, check_byte, check_end, source_bytes)
    });

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

        let mock_scope = match (tm.decorated_start_byte, tm.decorated_end_byte) {
            (Some(start), Some(end)) => root
                .descendant_for_byte_range(start, end)
                .unwrap_or(fn_node),
            _ => fn_node,
        };
        let mock_count = count_captures(mock_query, "mock", mock_scope, source_bytes);

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

        // Fixture count: number of function parameters (excluding `self`)
        let fixture_count = count_function_params(fn_node, source_bytes);

        // T108: wait-and-see detection
        let has_wait = has_any_match(wait_query, "wait", fn_node, source_bytes);

        // #64: skip-only test detection
        let has_skip_call = has_any_match(skip_query, "skip", fn_node, source_bytes);

        // T107: assertion message count
        let assertion_message_count =
            count_assertion_messages_py(assertion_query, fn_node, source_bytes);

        // T106: duplicate literal count
        let duplicate_literal_count = count_duplicate_literals(
            assertion_query,
            fn_node,
            source_bytes,
            &["integer", "float", "string"],
        );

        let suppress_row = tm.decorated_start_row.unwrap_or(tm.fn_start_row);
        let suppressed_rules = extract_suppression_from_previous_line(source, suppress_row);

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
                has_skip_call,
                assertion_message_count,
                duplicate_literal_count,
                suppressed_rules,
            },
        });
    }

    functions
}

impl LanguageExtractor for PythonExtractor {
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

/// Count function parameters excluding `self`.
/// Uses tree-sitter Node API: function_definition → parameters → named_child_count.
/// Only named children are actual parameters (commas and parens are anonymous).
fn count_function_params(fn_node: Node, source: &[u8]) -> usize {
    // Navigate up to the function_definition node if we're inside it
    let mut node = fn_node;
    while node.kind() != "function_definition" {
        match node.parent() {
            Some(p) => node = p,
            None => return 0,
        }
    }
    let params = match node.child_by_field_name("parameters") {
        Some(p) => p,
        None => return 0,
    };
    let count = params.named_child_count();
    if count == 0 {
        return 0;
    }
    // Check if first named child is `self` or `cls` (classmethod)
    if let Some(first) = params.named_child(0) {
        if first
            .utf8_text(source)
            .map(|s| s == "self" || s == "cls")
            .unwrap_or(false)
        {
            return count - 1;
        }
    }
    count
}

/// Count assertion statements that have a failure message.
/// Python: `assert expr, "msg"` has named child "msg". `self.assert*(a, b, msg)` has 3+ args.
fn count_assertion_messages_py(assertion_query: &Query, fn_node: Node, source: &[u8]) -> usize {
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
            if node.kind() == "assert_statement" {
                // assert_statement fields: condition (required), msg (optional)
                // If named_child_count > 1, the second child is the message
                if node.named_child_count() > 1 {
                    count += 1;
                }
            } else if node.kind() == "call" {
                // self.assert*(a, b) -> 2 args, self.assert*(a, b, msg) -> 3 args
                // arguments node contains the args
                if let Some(args) = node.child_by_field_name("arguments") {
                    // For assertTrue(x, msg): 2+ args means message present
                    // For assertEqual(a, b, msg): 3+ args means message present
                    // Heuristic: if method name starts with "assert" and has >=3 args,
                    // or assertTrue/assertFalse with >=2 args, it has a message.
                    // Simpler: any self.assert* with an odd number of args for comparison
                    // asserts, or just check if last arg is a string.
                    //
                    // Actually simplest: for unittest methods, the message is typically
                    // the last argument. We can't reliably distinguish without knowing
                    // the method signature. Let's use: named_child_count >= 3 for
                    // assertEqual/assertIn etc., >= 2 for assertTrue/assertFalse.
                    //
                    // Simplification: just check if there are "many" args relative to
                    // the minimum needed. For now, check if last arg is a string literal.
                    let arg_count = args.named_child_count();
                    if arg_count > 0 {
                        if let Some(last_arg) = args.named_child(arg_count - 1) {
                            if last_arg.kind() == "string"
                                || last_arg.kind() == "concatenated_string"
                            {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

fn extract_mock_class_name(var_name: &str) -> String {
    if let Some(stripped) = var_name.strip_prefix("mock_") {
        if !stripped.is_empty() {
            return stripped.to_string();
        }
    }
    var_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/python/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/lang-python", ""),
            name
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    // --- Cycle 2: Test function extraction ---

    #[test]
    fn extract_single_test_function() {
        let source = fixture("t001_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "test_create_user");
        assert_eq!(funcs[0].line, 1);
    }

    #[test]
    fn extract_multiple_test_functions_excludes_helpers() {
        let source = fixture("multiple_tests.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "multiple_tests.py");
        assert_eq!(funcs.len(), 3);
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["test_first", "test_second", "test_third"]);
        assert!(!names.contains(&"helper"));
    }

    #[test]
    fn line_count_calculation() {
        let source = fixture("t001_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.py");
        assert_eq!(
            funcs[0].analysis.line_count,
            funcs[0].end_line - funcs[0].line + 1
        );
    }

    // --- Cycle 3: Assertion detection ---

    #[test]
    fn assertion_count_zero_for_violation() {
        let source = fixture("t001_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.assertion_count, 0);
    }

    #[test]
    fn assertion_count_positive_for_pass() {
        let source = fixture("t001_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.py");
        assert_eq!(funcs[0].analysis.assertion_count, 1);
    }

    #[test]
    fn unittest_self_assert_counted() {
        let source = fixture("unittest_style.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "unittest_style.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.assertion_count, 2);
    }

    // --- Cycle 3: Mock detection ---

    #[test]
    fn mock_count_for_violation() {
        let source = fixture("t002_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 6);
    }

    #[test]
    fn mock_count_for_pass() {
        let source = fixture("t002_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_pass.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 1);
        assert_eq!(funcs[0].analysis.mock_classes, vec!["db"]);
    }

    #[test]
    fn mock_class_name_extraction() {
        assert_eq!(extract_mock_class_name("mock_db"), "db");
        assert_eq!(
            extract_mock_class_name("mock_payment_service"),
            "payment_service"
        );
        assert_eq!(extract_mock_class_name("my_mock"), "my_mock");
    }

    // --- Giant test ---

    #[test]
    fn giant_test_line_count() {
        let source = fixture("t003_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_violation.py");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count > 50);
    }

    #[test]
    fn short_test_line_count() {
        let source = fixture("t003_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_pass.py");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count <= 50);
    }

    // --- Inline suppression ---

    #[test]
    fn suppressed_test_has_suppressed_rules() {
        let source = fixture("suppressed.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "suppressed.py");
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
        let source = fixture("t002_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.py");
        assert!(funcs[0].analysis.suppressed_rules.is_empty());
    }

    // --- Phase 1 preserved tests ---

    #[test]
    fn parse_python_source() {
        let source = "def test_example():\n    pass\n";
        let mut parser = PythonExtractor::parser();
        let tree = parser.parse(source, None).unwrap();
        assert_eq!(tree.root_node().kind(), "module");
    }

    #[test]
    fn python_extractor_implements_language_extractor() {
        let extractor = PythonExtractor::new();
        let _: &dyn exspec_core::extractor::LanguageExtractor = &extractor;
    }

    // --- File analysis: parameterized ---

    #[test]
    fn file_analysis_detects_parameterized() {
        let source = fixture("t004_pass.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_pass.py");
        assert!(
            fa.parameterized_count >= 1,
            "expected parameterized_count >= 1, got {}",
            fa.parameterized_count
        );
    }

    #[test]
    fn file_analysis_no_parameterized() {
        let source = fixture("t004_violation.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_violation.py");
        assert_eq!(fa.parameterized_count, 0);
    }

    // --- File analysis: PBT import ---

    #[test]
    fn file_analysis_detects_pbt_import() {
        let source = fixture("t005_pass.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_pass.py");
        assert!(fa.has_pbt_import);
    }

    #[test]
    fn file_analysis_no_pbt_import() {
        let source = fixture("t005_violation.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_violation.py");
        assert!(!fa.has_pbt_import);
    }

    // --- File analysis: contract import ---

    #[test]
    fn file_analysis_detects_contract_import() {
        let source = fixture("t008_pass.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_pass.py");
        assert!(fa.has_contract_import);
    }

    #[test]
    fn file_analysis_no_contract_import() {
        let source = fixture("t008_violation.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_violation.py");
        assert!(!fa.has_contract_import);
    }

    // --- Class method false positive filtering ---

    #[test]
    fn class_method_in_non_test_class_excluded() {
        let source = fixture("test_class_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_class_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_connection"),
            "UserService.test_connection should be excluded: {names:?}"
        );
        assert!(
            !names.contains(&"test_health"),
            "UserService.test_health should be excluded: {names:?}"
        );
    }

    #[test]
    fn class_method_in_test_class_included() {
        let source = fixture("test_class_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_class_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"test_create"),
            "TestUser.test_create should be included: {names:?}"
        );
        assert!(
            names.contains(&"test_delete"),
            "TestUser.test_delete should be included: {names:?}"
        );
    }

    #[test]
    fn standalone_test_function_included() {
        let source = fixture("test_class_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_class_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"test_standalone"),
            "module-level test_standalone should be included: {names:?}"
        );
    }

    #[test]
    fn decorated_class_method_in_test_class_included() {
        let source = fixture("test_class_decorated.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_class_decorated.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "test_create");
    }

    // --- Issue #6: Nested class outermost ancestor ---

    #[test]
    fn nested_class_test_outer_helper_included() {
        let source = fixture("nested_class.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "nested_class.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"test_nested_in_test_outer"),
            "TestOuter > Helper > test_foo should be INCLUDED: {names:?}"
        );
    }

    #[test]
    fn nested_class_non_test_outer_excluded() {
        let source = fixture("nested_class.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "nested_class.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_nested_in_non_test_outer"),
            "UserService > TestInner > test_foo should be EXCLUDED: {names:?}"
        );
    }

    #[test]
    fn nested_class_both_non_test_excluded() {
        let source = fixture("nested_class.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "nested_class.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_connection"),
            "ServiceA > ServiceB > test_connection should be EXCLUDED: {names:?}"
        );
    }

    // --- File analysis preserves functions ---

    #[test]
    fn file_analysis_preserves_test_functions() {
        let source = fixture("t001_pass.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t001_pass.py");
        assert_eq!(fa.functions.len(), 1);
        assert_eq!(fa.functions[0].name, "test_create_user");
    }

    // --- T101: how-not-what ---

    #[test]
    fn how_not_what_count_for_violation() {
        let source = fixture("t101_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.py");
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
        let source = fixture("t101_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_pass.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.how_not_what_count, 0);
    }

    #[test]
    fn how_not_what_coexists_with_assertions() {
        // assert_called_with counts as both assertion (T001 pass) and how-not-what (T101 fire)
        let source = fixture("t101_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.py");
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
        Query::new(&python_language(), scm).unwrap()
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
        assert!(
            q.capture_index_for_name("decorated").is_some(),
            "test_function.scm must define @decorated capture"
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
        let source = fixture("t102_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_violation.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 7,
            "expected 7 parameters as fixture_count"
        );
    }

    #[test]
    fn fixture_count_for_pass() {
        let source = fixture("t102_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_pass.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 2,
            "expected 2 parameters as fixture_count"
        );
    }

    #[test]
    fn fixture_count_self_excluded() {
        let source = fixture("t102_self_excluded.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_self_excluded.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 2,
            "self should be excluded from fixture_count"
        );
    }

    #[test]
    fn fixture_count_cls_excluded() {
        let source = fixture("t102_cls_excluded.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_cls_excluded.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.fixture_count, 2,
            "cls should be excluded from fixture_count"
        );
    }

    // --- T101: private attribute access in assertions (#13) ---

    #[test]
    fn private_in_assertion_detected() {
        let source = fixture("t101_private_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.py");
        // test_checks_internal_count has assert service._count and assert service._processed
        let func = funcs
            .iter()
            .find(|f| f.name == "test_checks_internal_count")
            .unwrap();
        assert!(
            func.analysis.how_not_what_count >= 2,
            "expected >= 2 private access in assertions, got {}",
            func.analysis.how_not_what_count
        );
    }

    #[test]
    fn private_outside_assertion_not_counted() {
        let source = fixture("t101_private_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.py");
        // test_private_outside_assertion: obj._internal is outside assert, assert value == 42 has no private
        let func = funcs
            .iter()
            .find(|f| f.name == "test_private_outside_assertion")
            .unwrap();
        assert_eq!(
            func.analysis.how_not_what_count, 0,
            "private access outside assertion should not count"
        );
    }

    #[test]
    fn dunder_not_counted() {
        let source = fixture("t101_private_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.py");
        // test_dunder_not_private: __class__, __dict__ should not match
        let func = funcs
            .iter()
            .find(|f| f.name == "test_dunder_not_private")
            .unwrap();
        assert_eq!(
            func.analysis.how_not_what_count, 0,
            "__dunder__ should not be counted as private access"
        );
    }

    #[test]
    fn private_adds_to_how_not_what() {
        let source = fixture("t101_private_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.py");
        // test_mixed_private_and_mock: has assert_called_with (mock) + assert service._last_created (private)
        let func = funcs
            .iter()
            .find(|f| f.name == "test_mixed_private_and_mock")
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
    fn error_test_pytest_raises() {
        let source = fixture("t103_pass_pytest_raises.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_pytest_raises.py");
        assert!(fa.has_error_test, "pytest.raises should set has_error_test");
    }

    #[test]
    fn error_test_assert_raises() {
        let source = fixture("t103_pass_assertRaises.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_assertRaises.py");
        assert!(
            fa.has_error_test,
            "self.assertRaises should set has_error_test"
        );
    }

    #[test]
    fn error_test_assert_raises_regex() {
        let source = fixture("t103_pass_assertRaisesRegex.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_assertRaisesRegex.py");
        assert!(
            fa.has_error_test,
            "self.assertRaisesRegex should set has_error_test"
        );
    }

    #[test]
    fn error_test_assert_warns() {
        let source = fixture("t103_pass_assertWarns.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_assertWarns.py");
        assert!(
            fa.has_error_test,
            "self.assertWarns should set has_error_test"
        );
    }

    #[test]
    fn error_test_assert_warns_regex() {
        let source = fixture("t103_pass_assertWarnsRegex.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_assertWarnsRegex.py");
        assert!(
            fa.has_error_test,
            "self.assertWarnsRegex should set has_error_test"
        );
    }

    #[test]
    fn error_test_false_positive_non_self_receiver() {
        let source = fixture("t103_false_positive_non_self_receiver.py");
        let extractor = PythonExtractor::new();
        let fa =
            extractor.extract_file_analysis(&source, "t103_false_positive_non_self_receiver.py");
        assert!(
            !fa.has_error_test,
            "mock_obj.assertRaises() should NOT set has_error_test"
        );
    }

    #[test]
    fn error_test_no_patterns() {
        let source = fixture("t103_violation.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_violation.py");
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
        let source = fixture("t105_violation.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_violation.py");
        assert!(
            !fa.has_relational_assertion,
            "all equality file should not have relational"
        );
    }

    #[test]
    fn relational_assertion_pass_greater_than() {
        let source = fixture("t105_pass_relational.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass_relational.py");
        assert!(
            fa.has_relational_assertion,
            "assert x > 0 should set has_relational_assertion"
        );
    }

    #[test]
    fn relational_assertion_pass_contains() {
        let source = fixture("t105_pass_contains.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass_contains.py");
        assert!(
            fa.has_relational_assertion,
            "assert x in y should set has_relational_assertion"
        );
    }

    #[test]
    fn relational_assertion_pass_unittest() {
        let source = fixture("t105_pass_unittest.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass_unittest.py");
        assert!(
            fa.has_relational_assertion,
            "self.assertGreater should set has_relational_assertion"
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
        let source = fixture("t108_violation_sleep.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_violation_sleep.py");
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
        let source = fixture("t108_pass_no_sleep.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_pass_no_sleep.py");
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
        let source = fixture("t107_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_violation.py");
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
        let source = fixture("t107_pass_with_messages.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_pass_with_messages.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_message_count >= 1,
            "assertions with messages should be counted"
        );
    }

    #[test]
    fn t107_pass_single_assert() {
        let source = fixture("t107_pass_single_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_pass_single_assert.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 1,
            "single assertion does not trigger T107"
        );
    }

    // --- T109: undescriptive-test-name ---

    #[test]
    fn t109_violation_names_detected() {
        let source = fixture("t109_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_violation.py");
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
        let source = fixture("t109_pass.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_pass.py");
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
        let source = fixture("t106_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_violation.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count >= 3,
            "42 appears 4 times, should be >= 3: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    #[test]
    fn t106_pass_no_duplicates() {
        let source = fixture("t106_pass_no_duplicates.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_pass_no_duplicates.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count < 3,
            "each literal appears once: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    // --- T001 FP fix: pytest.raises as assertion (#25) ---

    #[test]
    fn t001_pytest_raises_counts_as_assertion() {
        // TC-01: pytest.raises() only -> T001 should NOT fire
        let source = fixture("t001_pytest_raises.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_raises.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pytest.raises() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_pytest_raises_with_match_counts_as_assertion() {
        // TC-02: pytest.raises() with match -> T001 should NOT fire
        let source = fixture("t001_pytest_raises_with_match.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_raises_with_match.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pytest.raises() with match should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    // --- T001 FP fix: pytest.warns (#34) ---

    #[test]
    fn t001_pytest_warns_counts_as_assertion() {
        // TC-04: pytest.warns() only -> T001 should NOT fire
        let source = fixture("t001_pytest_warns.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_warns.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pytest.warns() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_pytest_warns_with_match_counts_as_assertion() {
        // TC-05: pytest.warns() with match -> T001 should NOT fire
        let source = fixture("t001_pytest_warns_with_match.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_warns_with_match.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pytest.warns() with match should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_self_assert_raises_already_covered() {
        // TC-03: self.assertRaises() -> already matched by ^assert pattern
        let source = "import unittest\n\nclass TestUser(unittest.TestCase):\n    def test_invalid(self):\n        self.assertRaises(ValueError, create_user, '')\n";
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_assert_raises.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "self.assertRaises() should already be covered, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    // --- T001 FP fix: pytest.fail() (#57) ---

    #[test]
    fn t001_pytest_fail_counts_as_assertion() {
        // TC-01: pytest.fail() only -> T001 should NOT fire
        let source = fixture("t001_pytest_fail.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_fail.py");
        assert_eq!(funcs.len(), 2);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "pytest.fail() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_no_assertions_still_fires() {
        // TC-02: no assertions, no pytest.fail() -> T001 BLOCK (control case)
        let source = fixture("t001_pytest_fail.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pytest_fail.py");
        assert_eq!(funcs.len(), 2);
        assert_eq!(
            funcs[1].analysis.assertion_count, 0,
            "test_no_assertions should have 0 assertions, got {}",
            funcs[1].analysis.assertion_count
        );
    }

    // --- T001 FP fix: mock.assert_*() methods (#38) ---

    #[test]
    fn t001_mock_assert_called_once_counts_as_assertion() {
        // TC-07: mock.assert_called_once() -> assertion_count >= 1
        let source = fixture("t001_pass_mock_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mock_assert.py");
        assert!(funcs.len() >= 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "mock.assert_called_once() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mock_assert_called_once_with_counts_as_assertion() {
        // TC-08: mock.assert_called_once_with(arg) -> assertion_count >= 1
        let source = fixture("t001_pass_mock_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mock_assert.py");
        assert!(funcs.len() >= 2);
        assert!(
            funcs[1].analysis.assertion_count >= 1,
            "mock.assert_called_once_with() should count as assertion, got {}",
            funcs[1].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mock_assert_not_called_counts_as_assertion() {
        // TC-09: mock.assert_not_called() -> assertion_count >= 1
        let source = fixture("t001_pass_mock_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mock_assert.py");
        assert!(funcs.len() >= 3);
        assert!(
            funcs[2].analysis.assertion_count >= 1,
            "mock.assert_not_called() should count as assertion, got {}",
            funcs[2].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mock_assert_has_calls_counts_as_assertion() {
        // TC-10: mock.assert_has_calls([...]) -> assertion_count >= 1
        let source = fixture("t001_pass_mock_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mock_assert.py");
        assert!(funcs.len() >= 4);
        assert!(
            funcs[3].analysis.assertion_count >= 1,
            "mock.assert_has_calls() should count as assertion, got {}",
            funcs[3].analysis.assertion_count
        );
    }

    #[test]
    fn t001_chained_mock_assert_counts_as_assertion() {
        // TC-11: mock.return_value.assert_called_once() -> assertion_count >= 1
        let source = fixture("t001_pass_mock_assert.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mock_assert.py");
        assert!(funcs.len() >= 5);
        assert!(
            funcs[4].analysis.assertion_count >= 1,
            "chained mock.assert_called_once() should count as assertion, got {}",
            funcs[4].analysis.assertion_count
        );
    }

    // --- T001 FP fix: obj.assert*() without underscore (#62) ---

    #[test]
    fn t001_assert_no_underscore_counts_as_assertion() {
        // obj.assertoutcome() without underscore should count as assertion
        let source = fixture("t001_pass_assert_no_underscore.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_assert_no_underscore.py");
        assert!(funcs.len() >= 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "reprec.assertoutcome() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_assert_status_no_underscore_counts_as_assertion() {
        // obj.assertStatus() without underscore should count as assertion
        let source = fixture("t001_pass_assert_no_underscore.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_assert_no_underscore.py");
        assert!(funcs.len() >= 2);
        assert!(
            funcs[1].analysis.assertion_count >= 1,
            "response.assertStatus() should count as assertion, got {}",
            funcs[1].analysis.assertion_count
        );
    }

    #[test]
    fn t001_self_assert_equal_no_double_count_regression() {
        // TC-12: self.assertEqual still works, no double-count
        let source = "import unittest\n\nclass TestMath(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(1 + 1, 2)\n";
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_math.py");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 1,
            "self.assertEqual should count as exactly 1 assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t106_pass_trivial_literals() {
        let source = fixture("t106_pass_trivial_literals.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_pass_trivial_literals.py");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count < 3,
            "0 is trivial, should not count: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    // --- TC-11: Python custom helper + config -> T001 does NOT fire ---
    #[test]
    fn t001_custom_helper_with_config_no_violation() {
        use exspec_core::query_utils::apply_custom_assertion_fallback;
        use exspec_core::rules::{evaluate_rules, Config};

        let source = fixture("t001_custom_helper.py");
        let extractor = PythonExtractor::new();
        let mut analysis = extractor.extract_file_analysis(&source, "t001_custom_helper.py");
        let patterns = vec!["util.assertEqual(".to_string()];
        apply_custom_assertion_fallback(&mut analysis, &source, &patterns);

        let config = Config::default();
        let diags = evaluate_rules(&analysis.functions, &config);
        let t001_diags: Vec<_> = diags.iter().filter(|d| d.rule.0 == "T001").collect();
        // test_with_custom_helper: should pass (custom pattern match)
        // test_with_standard_assert: should pass (standard assert)
        // test_no_assertion_at_all: should BLOCK (no assertion)
        assert_eq!(
            t001_diags.len(),
            1,
            "only test_no_assertion_at_all should trigger T001"
        );
        assert!(
            t001_diags[0].message.contains("assertion-free"),
            "should be T001 assertion-free"
        );
    }

    // --- TC-12: Same test WITHOUT config -> T001 fires for custom helper ---
    #[test]
    fn t001_custom_helper_without_config_fires() {
        use exspec_core::rules::{evaluate_rules, Config};

        let source = fixture("t001_custom_helper.py");
        let extractor = PythonExtractor::new();
        let analysis = extractor.extract_file_analysis(&source, "t001_custom_helper.py");

        let config = Config::default();
        let diags = evaluate_rules(&analysis.functions, &config);
        let t001_diags: Vec<_> = diags.iter().filter(|d| d.rule.0 == "T001").collect();
        // Without config, only test_no_assertion_at_all should fire.
        // test_with_custom_helper has util.assertEqual() which is now detected
        // by the broadened obj.assert*() pattern (#62).
        assert_eq!(
            t001_diags.len(),
            1,
            "only test_no_assertion_at_all should trigger T001 (util.assertEqual is now detected)"
        );
    }

    // --- TC-13: Standard assert + custom config -> T001 does NOT fire ---
    #[test]
    fn t001_standard_assert_with_custom_config_still_passes() {
        use exspec_core::query_utils::apply_custom_assertion_fallback;
        use exspec_core::rules::{evaluate_rules, Config};

        let source = "def test_standard():\n    assert True\n";
        let extractor = PythonExtractor::new();
        let mut analysis = extractor.extract_file_analysis(source, "test_standard.py");
        let patterns = vec!["util.assertEqual(".to_string()];
        apply_custom_assertion_fallback(&mut analysis, source, &patterns);

        let config = Config::default();
        let diags = evaluate_rules(&analysis.functions, &config);
        let t001_diags: Vec<_> = diags.iter().filter(|d| d.rule.0 == "T001").collect();
        assert!(t001_diags.is_empty(), "standard assert should still work");
    }

    // --- TC-15: Pattern only in comment -> T001 does NOT fire (documented behavior) ---
    #[test]
    fn t001_custom_pattern_in_comment_prevents_t001() {
        use exspec_core::query_utils::apply_custom_assertion_fallback;
        use exspec_core::rules::{evaluate_rules, Config};

        let source = "def test_commented():\n    # util.assertEqual(x, 1)\n    pass\n";
        let extractor = PythonExtractor::new();
        let mut analysis = extractor.extract_file_analysis(source, "test_commented.py");
        let patterns = vec!["util.assertEqual(".to_string()];
        apply_custom_assertion_fallback(&mut analysis, source, &patterns);

        let config = Config::default();
        let diags = evaluate_rules(&analysis.functions, &config);
        let t001_diags: Vec<_> = diags.iter().filter(|d| d.rule.0 == "T001").collect();
        assert!(
            t001_diags.is_empty(),
            "comment match is included by design - T001 should not fire"
        );
    }

    // --- #64: T001 FP: skip-only test exclusion ---

    #[test]
    fn t001_skip_only_pytest_skip() {
        let source = fixture("t001_pass_skip_only.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_skip_only.py");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_skipped_feature")
            .expect("test_skipped_feature not found");
        assert!(
            f.analysis.has_skip_call,
            "pytest.skip() should set has_skip_call=true"
        );
    }

    #[test]
    fn t001_skip_only_self_skip_test() {
        let source = fixture("t001_pass_skip_only.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_skip_only.py");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_incomplete")
            .expect("test_incomplete not found");
        assert!(
            f.analysis.has_skip_call,
            "self.skipTest() should set has_skip_call=true"
        );
    }

    #[test]
    fn t001_skip_only_no_t001_block() {
        use exspec_core::rules::{evaluate_rules, Config, RuleId, Severity};

        let source = fixture("t001_pass_skip_only.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_skip_only.py");
        let diags: Vec<_> = evaluate_rules(&funcs, &Config::default())
            .into_iter()
            .filter(|d| d.rule == RuleId::new("T001") && d.severity == Severity::Block)
            .collect();
        assert!(
            diags.is_empty(),
            "Expected 0 T001 BLOCKs for skip-only fixture, got {}: {:?}",
            diags.len(),
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn t110_skip_only_fixture_produces_info() {
        use exspec_core::rules::{evaluate_rules, Config, RuleId, Severity};

        let source = fixture("t110_violation.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t110_violation.py");
        let diags: Vec<_> = evaluate_rules(&funcs, &Config::default())
            .into_iter()
            .filter(|d| d.rule == RuleId::new("T110") && d.severity == Severity::Info)
            .collect();
        assert_eq!(diags.len(), 1, "Expected exactly one T110 INFO: {diags:?}");
    }

    #[test]
    fn t110_existing_skip_only_fixture_produces_two_infos() {
        use exspec_core::rules::{evaluate_rules, Config, RuleId, Severity};

        let source = fixture("t001_pass_skip_only.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_skip_only.py");
        let diags: Vec<_> = evaluate_rules(&funcs, &Config::default())
            .into_iter()
            .filter(|d| d.rule == RuleId::new("T110") && d.severity == Severity::Info)
            .collect();
        assert_eq!(
            diags.len(),
            2,
            "Expected both existing skip-only tests to emit T110 INFO: {diags:?}"
        );
    }

    #[test]
    fn query_capture_names_skip_test() {
        let q = make_query(include_str!("../queries/skip_test.scm"));
        assert!(
            q.capture_index_for_name("skip").is_some(),
            "skip_test.scm must define @skip capture"
        );
    }

    // --- #56: pytest fixture with test_ prefix false positive ---

    #[test]
    fn pytest_fixture_decorated_test_excluded() {
        // TC-01: @pytest.fixture decorated test_data -> NOT included
        let source = fixture("test_fixture_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_fixture_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_data"),
            "@pytest.fixture test_data should be excluded: {names:?}"
        );
    }

    #[test]
    fn pytest_fixture_with_parens_excluded() {
        // TC-02: @pytest.fixture() with parens -> NOT included
        let source = fixture("test_fixture_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_fixture_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_config"),
            "@pytest.fixture() test_config should be excluded: {names:?}"
        );
    }

    #[test]
    fn bare_fixture_decorator_excluded() {
        // TC-03: @fixture (from pytest import fixture) -> NOT included
        let source = fixture("test_fixture_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_fixture_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"test_input"),
            "@fixture test_input should be excluded: {names:?}"
        );
    }

    #[test]
    fn patch_decorated_real_test_included() {
        // TC-04: @patch("x") decorated real test -> IS included (no regression)
        let source = fixture("test_fixture_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_fixture_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"test_something"),
            "@patch decorated test_something should be included: {names:?}"
        );
    }

    #[test]
    fn mixed_fixture_and_real_tests_evaluated() {
        // TC-05: Mixed file -> fixtures excluded, real tests evaluated normally
        use exspec_core::rules::{evaluate_rules, Config};

        let source = fixture("test_fixture_false_positive.py");
        let extractor = PythonExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "test_fixture_false_positive.py");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();

        // 3 real tests: test_something, test_real_function, test_uses_fixture
        assert_eq!(
            funcs.len(),
            3,
            "expected 3 real tests (fixtures excluded): {names:?}"
        );

        // T001 should fire only on test_uses_fixture (assertion-free)
        let config = Config::default();
        let diags = evaluate_rules(&funcs, &config);
        let t001_diags: Vec<_> = diags.iter().filter(|d| d.rule.0 == "T001").collect();
        assert_eq!(
            t001_diags.len(),
            1,
            "only test_uses_fixture should trigger T001: {t001_diags:?}"
        );
        assert!(
            t001_diags[0].message.contains("test_uses_fixture"),
            "T001 should reference test_uses_fixture: {}",
            t001_diags[0].message
        );
    }
}
