use std::sync::OnceLock;

use exspec_core::extractor::{FileAnalysis, LanguageExtractor, TestAnalysis, TestFunction};
use exspec_core::query_utils::{
    collect_mock_class_names, count_captures, extract_suppression_from_previous_line, has_any_match,
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
}
