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
    let idx = query
        .capture_index_for_name(capture_name)
        .expect("capture not found");
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
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_definition" {
            if let Some(name_node) = parent.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    return !name.starts_with("Test") && !name.starts_with("test_");
                }
            }
            return true; // class without parseable name -> exclude
        }
        current = parent.parent();
    }
    false // module-level -> not in non-test class
}

fn extract_functions_from_tree(source: &str, file_path: &str, root: Node) -> Vec<TestFunction> {
    let test_query = cached_query(&TEST_QUERY_CACHE, TEST_FUNCTION_QUERY);
    let assertion_query = cached_query(&ASSERTION_QUERY_CACHE, ASSERTION_QUERY);
    let mock_query = cached_query(&MOCK_QUERY_CACHE, MOCK_USAGE_QUERY);
    let mock_assign_query = cached_query(&MOCK_ASSIGN_QUERY_CACHE, MOCK_ASSIGNMENT_QUERY);

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
                decorated_fn_ids.insert(inner_fn.id());
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

        let mock_classes = collect_mock_class_names(mock_assign_query, fn_node, source_bytes);

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

    // --- File analysis preserves functions ---

    #[test]
    fn file_analysis_preserves_test_functions() {
        let source = fixture("t001_pass.py");
        let extractor = PythonExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t001_pass.py");
        assert_eq!(fa.functions.len(), 1);
        assert_eq!(fa.functions[0].name, "test_create_user");
    }
}
