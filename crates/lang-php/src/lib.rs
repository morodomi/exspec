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

fn php_language() -> tree_sitter::Language {
    tree_sitter_php::LANGUAGE_PHP.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&php_language(), source).expect("invalid query"))
}

static TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static MOCK_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static MOCK_ASSIGN_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static PARAMETERIZED_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static IMPORT_PBT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static IMPORT_CONTRACT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

pub struct PhpExtractor;

impl PhpExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn parser() -> Parser {
        let mut parser = Parser::new();
        let language = tree_sitter_php::LANGUAGE_PHP;
        parser
            .set_language(&language.into())
            .expect("failed to load PHP grammar");
        parser
    }
}

impl Default for PhpExtractor {
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

fn extract_mock_class_name(var_name: &str) -> String {
    // PHP uses $mockDb or $mock_db patterns
    // Strip $ prefix first
    let name = var_name.strip_prefix('$').unwrap_or(var_name);
    // camelCase: strip "mock" prefix
    if let Some(stripped) = name.strip_prefix("mock") {
        if !stripped.is_empty() && stripped.starts_with(|c: char| c.is_uppercase()) {
            return stripped.to_string();
        }
    }
    // snake_case: strip "mock_" prefix
    if let Some(stripped) = name.strip_prefix("mock_") {
        if !stripped.is_empty() {
            return stripped.to_string();
        }
    }
    name.to_string()
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

/// Check if the method has a `/** @test */` docblock comment on the preceding line(s).
fn has_docblock_test_annotation(source: &str, start_row: usize) -> bool {
    if start_row == 0 {
        return false;
    }
    let lines: Vec<&str> = source.lines().collect();
    // Look up to 5 lines above for /** ... @test ... */
    let start = start_row.saturating_sub(5);
    for i in (start..start_row).rev() {
        if let Some(line) = lines.get(i) {
            let trimmed = line.trim();
            if trimmed.contains("@test") {
                return true;
            }
            // Stop scanning at non-comment lines
            if !trimmed.starts_with('*')
                && !trimmed.starts_with("/**")
                && !trimmed.starts_with("*/")
                && !trimmed.is_empty()
            {
                break;
            }
        }
    }
    false
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

    let source_bytes = source.as_bytes();

    // Collect matches from test_function.scm query
    let name_idx = test_query
        .capture_index_for_name("name")
        .expect("no @name capture");
    let function_idx = test_query
        .capture_index_for_name("function")
        .expect("no @function capture");

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

    // Also detect methods with /** @test */ docblock annotation
    // These are method_declaration nodes where the name does NOT start with test_
    // but have a @test docblock. We need to walk the tree for these.
    detect_docblock_test_methods(root, source, &mut test_matches);

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

fn detect_docblock_test_methods(root: Node, source: &str, matches: &mut Vec<TestMatch>) {
    let source_bytes = source.as_bytes();
    let mut cursor = root.walk();

    // Walk all method_declaration nodes
    fn visit(
        cursor: &mut tree_sitter::TreeCursor,
        source: &str,
        source_bytes: &[u8],
        matches: &mut Vec<TestMatch>,
    ) {
        loop {
            let node = cursor.node();
            if node.kind() == "method_declaration" {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source_bytes) {
                        // Skip methods already matched by test* prefix query or #[Test] attribute
                        if !name.starts_with("test") {
                            // Check for @test docblock
                            if has_docblock_test_annotation(source, node.start_position().row) {
                                matches.push(TestMatch {
                                    name: name.to_string(),
                                    fn_start_byte: node.start_byte(),
                                    fn_end_byte: node.end_byte(),
                                    fn_start_row: node.start_position().row,
                                    fn_end_row: node.end_position().row,
                                });
                            }
                        }
                    }
                }
            }
            if cursor.goto_first_child() {
                visit(cursor, source, source_bytes, matches);
                cursor.goto_parent();
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if cursor.goto_first_child() {
        visit(&mut cursor, source, source_bytes, matches);
    }
}

impl LanguageExtractor for PhpExtractor {
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
            "{}/tests/fixtures/php/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/lang-php", ""),
            name
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    // --- Phase 1 preserved tests ---

    #[test]
    fn parse_php_source() {
        let source = "<?php\nfunction test_example(): void {}\n";
        let mut parser = PhpExtractor::parser();
        let tree = parser.parse(source, None).unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn php_extractor_implements_language_extractor() {
        let extractor = PhpExtractor::new();
        let _: &dyn exspec_core::extractor::LanguageExtractor = &extractor;
    }

    // --- Test function extraction ---

    #[test]
    fn extract_single_phpunit_test() {
        let source = fixture("t001_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "test_create_user");
        assert_eq!(funcs[0].line, 5);
    }

    #[test]
    fn extract_multiple_phpunit_tests_excludes_helpers() {
        let source = fixture("multiple_tests.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "multiple_tests.php");
        assert_eq!(funcs.len(), 3);
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["test_add", "test_subtract", "test_multiply"]);
    }

    #[test]
    fn extract_test_with_attribute() {
        let source = fixture("t001_pass_attribute.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_attribute.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "createUser");
    }

    #[test]
    fn extract_pest_test() {
        let source = fixture("t001_pass_pest.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_pest.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "creates a user");
    }

    #[test]
    fn line_count_calculation() {
        let source = fixture("t001_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.php");
        assert_eq!(
            funcs[0].analysis.line_count,
            funcs[0].end_line - funcs[0].line + 1
        );
    }

    // --- Assertion detection ---

    #[test]
    fn assertion_count_zero_for_violation() {
        let source = fixture("t001_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.assertion_count, 0);
    }

    #[test]
    fn assertion_count_positive_for_pass() {
        let source = fixture("t001_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.php");
        assert_eq!(funcs[0].analysis.assertion_count, 1);
    }

    #[test]
    fn pest_expect_assertion_counted() {
        let source = fixture("t001_pass_pest.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_pest.php");
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "expected >= 1, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn pest_violation_zero_assertions() {
        let source = fixture("t001_violation_pest.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation_pest.php");
        assert_eq!(funcs[0].analysis.assertion_count, 0);
    }

    // --- camelCase test detection ---

    #[test]
    fn extract_camelcase_phpunit_test() {
        let source = fixture("t001_pass_camelcase.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_camelcase.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "testCreateUser");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    #[test]
    fn extract_docblock_test() {
        let source = fixture("t001_pass_docblock.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_docblock.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "creates_a_user");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    // --- Mock class name extraction ---

    #[test]
    fn mock_class_name_extraction() {
        assert_eq!(extract_mock_class_name("$mockDb"), "Db");
        assert_eq!(extract_mock_class_name("$mock_payment"), "payment");
        assert_eq!(extract_mock_class_name("$service"), "service");
        assert_eq!(extract_mock_class_name("$mockUserService"), "UserService");
    }

    // --- Mock detection ---

    #[test]
    fn mock_count_for_violation() {
        let source = fixture("t002_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 6);
    }

    #[test]
    fn mock_count_for_pass() {
        let source = fixture("t002_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_pass.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].analysis.mock_count, 1);
        assert_eq!(funcs[0].analysis.mock_classes, vec!["Repo"]);
    }

    #[test]
    fn mock_classes_for_violation() {
        let source = fixture("t002_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.php");
        assert!(
            funcs[0].analysis.mock_classes.len() >= 4,
            "expected >= 4 mock classes, got: {:?}",
            funcs[0].analysis.mock_classes
        );
    }

    // --- Giant test ---

    #[test]
    fn giant_test_line_count() {
        let source = fixture("t003_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_violation.php");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count > 50);
    }

    #[test]
    fn short_test_line_count() {
        let source = fixture("t003_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t003_pass.php");
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].analysis.line_count <= 50);
    }

    // --- Inline suppression ---

    #[test]
    fn suppressed_test_has_suppressed_rules() {
        let source = fixture("suppressed.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "suppressed.php");
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
        let source = fixture("t002_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t002_violation.php");
        assert!(funcs[0].analysis.suppressed_rules.is_empty());
    }

    // --- File analysis: parameterized ---

    #[test]
    fn file_analysis_detects_parameterized() {
        let source = fixture("t004_pass.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_pass.php");
        assert!(
            fa.parameterized_count >= 1,
            "expected parameterized_count >= 1, got {}",
            fa.parameterized_count
        );
    }

    #[test]
    fn file_analysis_no_parameterized() {
        let source = fixture("t004_violation.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_violation.php");
        assert_eq!(fa.parameterized_count, 0);
    }

    #[test]
    fn file_analysis_pest_parameterized() {
        let source = fixture("t004_pass_pest.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t004_pass_pest.php");
        assert!(
            fa.parameterized_count >= 1,
            "expected parameterized_count >= 1, got {}",
            fa.parameterized_count
        );
    }

    // --- File analysis: PBT import ---

    #[test]
    fn file_analysis_no_pbt_import() {
        // PHP PBT is not mature; always returns false
        let source = fixture("t005_violation.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t005_violation.php");
        assert!(!fa.has_pbt_import);
    }

    // --- File analysis: contract import ---

    #[test]
    fn file_analysis_detects_contract_import() {
        let source = fixture("t008_pass.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_pass.php");
        assert!(fa.has_contract_import);
    }

    #[test]
    fn file_analysis_no_contract_import() {
        let source = fixture("t008_violation.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t008_violation.php");
        assert!(!fa.has_contract_import);
    }

    // --- FQCN attribute detection ---

    #[test]
    fn extract_fqcn_attribute_test() {
        let source = fixture("t001_pass_fqcn_attribute.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_fqcn_attribute.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "creates_a_user");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    // --- Pest arrow function detection ---

    #[test]
    fn extract_pest_arrow_function() {
        let source = fixture("t001_pass_pest_arrow.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_pest_arrow.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "creates a user");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    #[test]
    fn extract_pest_arrow_function_chained() {
        let source = fixture("t001_pass_pest_arrow_chained.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_pest_arrow_chained.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "adds numbers");
        assert!(funcs[0].analysis.assertion_count >= 1);
    }

    // --- File analysis preserves functions ---

    #[test]
    fn file_analysis_preserves_test_functions() {
        let source = fixture("t001_pass.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t001_pass.php");
        assert_eq!(fa.functions.len(), 1);
        assert_eq!(fa.functions[0].name, "test_create_user");
    }
}
