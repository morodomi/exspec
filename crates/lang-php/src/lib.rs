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
static HOW_NOT_WHAT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static PRIVATE_IN_ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static ERROR_TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static RELATIONAL_ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();
static WAIT_AND_SEE_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

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

/// Check if a PHP method has a #[DataProvider] attribute.
fn has_data_provider_attribute(fn_node: Node, source: &[u8]) -> bool {
    let mut cursor = fn_node.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "attribute_list" {
                let text = node.utf8_text(source).unwrap_or("");
                if text.contains("DataProvider") {
                    return true;
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

/// Count the number of parameters in a PHP method (formal_parameters).
fn count_method_params(fn_node: Node) -> usize {
    let params_node = match fn_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return 0,
    };

    let mut count = 0;
    let mut cursor = params_node.walk();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "simple_parameter" || node.kind() == "variadic_parameter" {
                count += 1;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    count
}

/// Count PHPUnit assertion calls that have a message argument (last arg is a string).
/// In tree-sitter-php, `arguments` contains `argument` children, each wrapping an expression.
fn count_assertion_messages_php(assertion_query: &Query, fn_node: Node, source: &[u8]) -> usize {
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
            // member_call_expression -> arguments -> argument children
            if let Some(args) = node.child_by_field_name("arguments") {
                let arg_count = args.named_child_count();
                if arg_count > 0 {
                    if let Some(last_arg_wrapper) = args.named_child(arg_count - 1) {
                        // argument node wraps the actual expression
                        let expr = if last_arg_wrapper.kind() == "argument" {
                            last_arg_wrapper.named_child(0)
                        } else {
                            Some(last_arg_wrapper)
                        };
                        if let Some(expr_node) = expr {
                            let kind = expr_node.kind();
                            if kind == "string" || kind == "encapsed_string" {
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

    // Dedup: docblock detector may re-add methods already matched by query
    let mut seen = std::collections::HashSet::new();
    test_matches.retain(|tm| seen.insert(tm.fn_start_byte));

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

        let fixture_count = if has_data_provider_attribute(fn_node, source_bytes) {
            0
        } else {
            count_method_params(fn_node)
        };

        // T108: wait-and-see detection
        let has_wait = has_any_match(wait_query, "wait", fn_node, source_bytes);

        // T107: assertion message count
        let assertion_message_count =
            count_assertion_messages_php(assertion_query, fn_node, source_bytes);

        // T106: duplicate literal count
        let duplicate_literal_count = count_duplicate_literals(
            assertion_query,
            fn_node,
            source_bytes,
            &["integer", "float", "string", "encapsed_string"],
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
                assertion_message_count,
                duplicate_literal_count,
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

    // --- T001 FP fix: Mockery + PHPUnit mock expectations (#38) ---

    #[test]
    fn t001_mockery_should_receive_counts_as_assertion() {
        // TC-01: $mock->shouldReceive('x')->once() -> assertion_count >= 1
        let source = fixture("t001_pass_mockery.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mockery.php");
        assert!(funcs.len() >= 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "shouldReceive() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mockery_should_have_received_counts_as_assertion() {
        // TC-02: $mock->shouldHaveReceived('x')->once() -> assertion_count >= 1
        let source = fixture("t001_pass_mockery.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mockery.php");
        // test_verifies_post_execution is the 2nd test
        assert!(funcs.len() >= 2);
        assert!(
            funcs[1].analysis.assertion_count >= 1,
            "shouldHaveReceived() should count as assertion, got {}",
            funcs[1].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mockery_should_not_have_received_counts_as_assertion() {
        // TC-03: $mock->shouldNotHaveReceived('x') -> assertion_count >= 1
        let source = fixture("t001_pass_mockery.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mockery.php");
        // test_negative_verification is the 3rd test
        assert!(funcs.len() >= 3);
        assert!(
            funcs[2].analysis.assertion_count >= 1,
            "shouldNotHaveReceived() should count as assertion, got {}",
            funcs[2].analysis.assertion_count
        );
    }

    #[test]
    fn t001_phpunit_mock_expects_not_this_not_counted() {
        // $mock->expects() is NOT counted as assertion (only $this->expects() is)
        let source = fixture("t001_violation_phpunit_mock.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_violation_phpunit_mock.php");
        assert_eq!(funcs.len(), 1);
        assert_eq!(
            funcs[0].analysis.assertion_count, 0,
            "$mock->expects() should NOT count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_mockery_multiple_expectations_counted() {
        // TC-06: 3x shouldReceive calls -> assertion_count >= 3
        let source = fixture("t001_pass_mockery.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_mockery.php");
        // test_multiple_mock_expectations is the 4th test
        assert!(funcs.len() >= 4);
        assert!(
            funcs[3].analysis.assertion_count >= 3,
            "3x shouldReceive() should count as >= 3 assertions, got {}",
            funcs[3].analysis.assertion_count
        );
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

    // --- Issue #8: FQCN false positive ---

    #[test]
    fn fqcn_rejects_non_phpunit_attribute() {
        let source = fixture("fqcn_false_positive.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "fqcn_false_positive.php");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(
            !names.contains(&"custom_attribute_method"),
            "custom #[\\MyApp\\Attributes\\Test] should NOT be detected: {names:?}"
        );
        assert!(
            names.contains(&"real_phpunit_attribute"),
            "real #[\\PHPUnit\\...\\Test] should be detected: {names:?}"
        );
        assert_eq!(funcs.len(), 1);
    }

    // --- Issue #7: Docblock double detection ---

    #[test]
    fn docblock_attribute_no_double_detection() {
        let source = fixture("docblock_double_detection.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "docblock_double_detection.php");
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            funcs.len(),
            3,
            "expected exactly 3 test functions (no duplicates): {names:?}"
        );
        assert!(names.contains(&"short_attribute_with_docblock"));
        assert!(names.contains(&"fqcn_attribute_with_docblock"));
        assert!(names.contains(&"docblock_only"));
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

    // --- Query capture name verification (#14) ---

    fn make_query(scm: &str) -> Query {
        Query::new(&php_language(), scm).unwrap()
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

    // Comment-only file by design (PHP PBT is not mature).
    // This assertion will fail when a real PBT library is added.
    // When that happens, update the has_any_match call site in extract_file_analysis() accordingly.
    #[test]
    fn query_capture_names_import_pbt_comment_only() {
        let q = make_query(include_str!("../queries/import_pbt.scm"));
        assert!(
            q.capture_index_for_name("pbt_import").is_none(),
            "PHP import_pbt.scm is intentionally comment-only"
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

    // --- T103: missing-error-test ---

    #[test]
    fn error_test_expect_exception() {
        let source = fixture("t103_pass.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass.php");
        assert!(
            fa.has_error_test,
            "$this->expectException should set has_error_test"
        );
    }

    #[test]
    fn error_test_pest_to_throw() {
        let source = fixture("t103_pass_pest.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_pass_pest.php");
        assert!(
            fa.has_error_test,
            "Pest ->toThrow() should set has_error_test"
        );
    }

    #[test]
    fn error_test_no_patterns() {
        let source = fixture("t103_violation.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t103_violation.php");
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
    fn relational_assertion_pass_greater_than() {
        let source = fixture("t105_pass.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_pass.php");
        assert!(
            fa.has_relational_assertion,
            "assertGreaterThan should set has_relational_assertion"
        );
    }

    #[test]
    fn relational_assertion_violation() {
        let source = fixture("t105_violation.php");
        let extractor = PhpExtractor::new();
        let fa = extractor.extract_file_analysis(&source, "t105_violation.php");
        assert!(
            !fa.has_relational_assertion,
            "only assertEquals should not set has_relational_assertion"
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
    fn how_not_what_expects() {
        let source = fixture("t101_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.php");
        assert!(
            funcs[0].analysis.how_not_what_count > 0,
            "->expects() should trigger how_not_what, got {}",
            funcs[0].analysis.how_not_what_count
        );
    }

    #[test]
    fn how_not_what_should_receive() {
        let source = fixture("t101_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_violation.php");
        assert!(
            funcs[1].analysis.how_not_what_count > 0,
            "->shouldReceive() should trigger how_not_what, got {}",
            funcs[1].analysis.how_not_what_count
        );
    }

    #[test]
    fn how_not_what_pass() {
        let source = fixture("t101_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_pass.php");
        assert_eq!(
            funcs[0].analysis.how_not_what_count, 0,
            "no mock patterns should have how_not_what_count=0"
        );
    }

    #[test]
    fn how_not_what_private_access() {
        let source = fixture("t101_private_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t101_private_violation.php");
        assert!(
            funcs[0].analysis.how_not_what_count > 0,
            "$obj->_name in assertion should trigger how_not_what, got {}",
            funcs[0].analysis.how_not_what_count
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
        let source = fixture("t102_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_violation.php");
        assert_eq!(
            funcs[0].analysis.fixture_count, 7,
            "expected 7 parameters as fixture_count"
        );
    }

    #[test]
    fn fixture_count_for_pass() {
        let source = fixture("t102_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_pass.php");
        assert_eq!(
            funcs[0].analysis.fixture_count, 0,
            "expected 0 parameters as fixture_count"
        );
    }

    #[test]
    fn fixture_count_zero_for_dataprovider_method() {
        let source = fixture("t102_dataprovider.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t102_dataprovider.php");
        // test_addition: 3 params but has #[DataProvider] -> fixture_count = 0
        let addition = funcs.iter().find(|f| f.name == "test_addition").unwrap();
        assert_eq!(
            addition.analysis.fixture_count, 0,
            "DataProvider params should not count as fixtures"
        );
        // addition_with_test_attr: 3 params + #[DataProvider] + #[Test] -> fixture_count = 0
        let with_attr = funcs
            .iter()
            .find(|f| f.name == "addition_with_test_attr")
            .unwrap();
        assert_eq!(
            with_attr.analysis.fixture_count, 0,
            "DataProvider params should not count as fixtures even with #[Test]"
        );
        // test_with_fixtures: 6 params, no DataProvider -> fixture_count = 6
        let fixtures = funcs
            .iter()
            .find(|f| f.name == "test_with_fixtures")
            .unwrap();
        assert_eq!(
            fixtures.analysis.fixture_count, 6,
            "non-DataProvider params should count as fixtures"
        );
    }

    // --- T108: wait-and-see ---

    #[test]
    fn wait_and_see_violation_sleep() {
        let source = fixture("t108_violation_sleep.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_violation_sleep.php");
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
        let source = fixture("t108_pass_no_sleep.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t108_pass_no_sleep.php");
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
        let source = fixture("t107_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_violation.php");
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
        let source = fixture("t107_pass_with_messages.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t107_pass_with_messages.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_message_count >= 1,
            "assertions with messages should be counted"
        );
    }

    // --- T109: undescriptive-test-name ---

    #[test]
    fn t109_violation_names_detected() {
        let source = fixture("t109_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_violation.php");
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
        let source = fixture("t109_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t109_pass.php");
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
        let source = fixture("t106_violation.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_violation.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count >= 3,
            "42 appears 3 times, should be >= 3: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    #[test]
    fn t106_pass_no_duplicates() {
        let source = fixture("t106_pass_no_duplicates.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t106_pass_no_duplicates.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.duplicate_literal_count < 3,
            "each literal appears once: got {}",
            funcs[0].analysis.duplicate_literal_count
        );
    }

    // --- T001 FP fix: expectException/Message/Code (#25) ---

    #[test]
    fn t001_expect_exception_counts_as_assertion() {
        // TC-07: $this->expectException() only -> T001 should NOT fire
        let source = fixture("t001_expect_exception.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_expect_exception.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "$this->expectException() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_expect_exception_message_counts_as_assertion() {
        // TC-08: $this->expectExceptionMessage() only -> T001 should NOT fire
        let source = fixture("t001_expect_exception_message.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_expect_exception_message.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "$this->expectExceptionMessage() should count as assertion, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    // --- #44: T001 FP: arbitrary-object ->assert*() and self::assert*() ---

    #[test]
    fn t001_response_assert_status() {
        let source = fixture("t001_pass_obj_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_obj_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_response_assert_status")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "$response->assertStatus() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_chained_response_assertions() {
        let source = fixture("t001_pass_obj_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_obj_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_chained_response_assertions")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 2,
            "chained ->assertStatus()->assertJsonCount() should count >= 2, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_assertion_helper_not_counted() {
        let source = fixture("t001_pass_obj_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_obj_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_assertion_helper_not_counted")
            .unwrap();
        assert_eq!(
            f.analysis.assertion_count, 0,
            "assertionHelper() should NOT count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_self_assert_equals() {
        let source = fixture("t001_pass_self_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_self_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_self_assert_equals")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "self::assertEquals() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_static_assert_true() {
        let source = fixture("t001_pass_self_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_self_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_static_assert_true")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "static::assertTrue() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_artisan_expects_output() {
        let source = fixture("t001_pass_artisan_expects.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_artisan_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_artisan_expects_output")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 2,
            "expectsOutput + assertExitCode should count >= 2, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_artisan_expects_question() {
        let source = fixture("t001_pass_artisan_expects.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_artisan_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_artisan_expects_question")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "expectsQuestion() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_expect_not_to_perform_assertions() {
        let source = fixture("t001_pass_artisan_expects.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_artisan_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_expect_not_to_perform_assertions")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "expectNotToPerformAssertions() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_expect_output_string() {
        let source = fixture("t001_pass_artisan_expects.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_artisan_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_expect_output_string")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "expectOutputString() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_existing_this_assert_still_works() {
        // Regression: $this->assertEquals still detected after Pattern A change
        let source = fixture("t001_pass.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass.php");
        assert_eq!(funcs.len(), 1);
        assert!(
            funcs[0].analysis.assertion_count >= 1,
            "$this->assertEquals() regression: should still count, got {}",
            funcs[0].analysis.assertion_count
        );
    }

    #[test]
    fn t001_bare_assert_counted() {
        let source = fixture("t001_pass_obj_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_obj_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_bare_assert_call")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "->assert() bare call should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_parent_assert_same() {
        // parent:: is relative_scope in tree-sitter-php; intentionally counted as oracle
        let source = fixture("t001_pass_self_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_self_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_parent_assert_same")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "parent::assertSame() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_artisan_expects_no_output() {
        let source = fixture("t001_pass_artisan_expects.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_artisan_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_artisan_expects_no_output")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "expectsNoOutput() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_named_class_assert_equals() {
        let source = fixture("t001_pass_named_class_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_named_class_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_assert_class_equals")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "Assert::assertEquals() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_fqcn_assert_same() {
        let source = fixture("t001_pass_named_class_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_named_class_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_fqcn_assert_same")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "PHPUnit\\Framework\\Assert::assertSame() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_named_class_assert_true() {
        let source = fixture("t001_pass_named_class_assert.php");
        let extractor = PhpExtractor::new();
        let funcs = extractor.extract_test_functions(&source, "t001_pass_named_class_assert.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_assert_class_true")
            .unwrap();
        assert!(
            f.analysis.assertion_count >= 1,
            "Assert::assertTrue() should count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_non_this_expects_not_counted() {
        let source = fixture("t001_violation_non_this_expects.php");
        let extractor = PhpExtractor::new();
        let funcs =
            extractor.extract_test_functions(&source, "t001_violation_non_this_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_event_emitter_expects_not_assertion")
            .unwrap();
        assert_eq!(
            f.analysis.assertion_count, 0,
            "$emitter->expects() should NOT count as assertion, got {}",
            f.analysis.assertion_count
        );
    }

    #[test]
    fn t001_mock_expects_not_this_not_counted() {
        let source = fixture("t001_violation_non_this_expects.php");
        let extractor = PhpExtractor::new();
        let funcs =
            extractor.extract_test_functions(&source, "t001_violation_non_this_expects.php");
        let f = funcs
            .iter()
            .find(|f| f.name == "test_mock_expects_not_this")
            .unwrap();
        assert_eq!(
            f.analysis.assertion_count, 0,
            "$mock->expects() should NOT count as assertion, got {}",
            f.analysis.assertion_count
        );
    }
}
