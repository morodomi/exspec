use std::collections::{BTreeSet, HashMap};

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use crate::rules::RuleId;
use crate::suppress::parse_suppression;

pub fn count_captures(query: &Query, capture_name: &str, node: Node, source: &[u8]) -> usize {
    let idx = match query.capture_index_for_name(capture_name) {
        Some(i) => i,
        None => return 0,
    };
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    let mut count = 0;
    while let Some(m) = matches.next() {
        count += m.captures.iter().filter(|c| c.index == idx).count();
    }
    count
}

pub fn has_any_match(query: &Query, capture_name: &str, node: Node, source: &[u8]) -> bool {
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

pub fn collect_mock_class_names<F>(
    query: &Query,
    node: Node,
    source: &[u8],
    extract_name: F,
) -> Vec<String>
where
    F: Fn(&str) -> String,
{
    let var_idx = match query.capture_index_for_name("var_name") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    let mut names = BTreeSet::new();
    while let Some(m) = matches.next() {
        for c in m.captures.iter().filter(|c| c.index == var_idx) {
            if let Ok(var) = c.node.utf8_text(source) {
                names.insert(extract_name(var));
            }
        }
    }
    names.into_iter().collect()
}

/// Collect byte ranges of all captures matching `capture_name` in `query`.
fn collect_capture_ranges(
    query: &Query,
    capture_name: &str,
    node: Node,
    source: &[u8],
) -> Vec<(usize, usize)> {
    let idx = match query.capture_index_for_name(capture_name) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let mut ranges = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, node, source);
    while let Some(m) = matches.next() {
        for c in m.captures.iter().filter(|c| c.index == idx) {
            ranges.push((c.node.start_byte(), c.node.end_byte()));
        }
    }
    ranges
}

/// Count captures of `inner_capture` from `inner_query` that fall within
/// byte ranges of `outer_capture` from `outer_query`.
pub fn count_captures_within_context(
    outer_query: &Query,
    outer_capture: &str,
    inner_query: &Query,
    inner_capture: &str,
    node: Node,
    source: &[u8],
) -> usize {
    let ranges = collect_capture_ranges(outer_query, outer_capture, node, source);
    if ranges.is_empty() {
        return 0;
    }

    let inner_idx = match inner_query.capture_index_for_name(inner_capture) {
        Some(i) => i,
        None => return 0,
    };

    let mut count = 0;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(inner_query, node, source);
    while let Some(m) = matches.next() {
        for c in m.captures.iter().filter(|c| c.index == inner_idx) {
            let start = c.node.start_byte();
            let end = c.node.end_byte();
            if ranges.iter().any(|(rs, re)| start >= *rs && end <= *re) {
                count += 1;
            }
        }
    }

    count
}

// Literals considered too common to flag as duplicates.
// Cross-language superset: Python (True/False/None), JS (null/undefined), PHP/Ruby (nil).
const TRIVIAL_LITERALS: &[&str] = &[
    "0",
    "1",
    "2",
    "true",
    "false",
    "True",
    "False",
    "None",
    "null",
    "undefined",
    "nil",
    "\"\"",
    "''",
    "0.0",
    "1.0",
];

/// Count the maximum number of times any non-trivial literal appears
/// within assertion nodes of the given function node.
///
/// `assertion_query` must have an `@assertion` capture.
/// `literal_kinds` lists the tree-sitter node kind names that represent literals
/// for the target language (e.g., `["integer", "float", "string"]` for Python).
pub fn count_duplicate_literals(
    assertion_query: &Query,
    node: Node,
    source: &[u8],
    literal_kinds: &[&str],
) -> usize {
    let ranges = collect_capture_ranges(assertion_query, "assertion", node, source);
    if ranges.is_empty() {
        return 0;
    }

    // Walk tree, collect literals within assertion ranges
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        let start = n.start_byte();
        let end = n.end_byte();

        // Prune subtrees that don't overlap with any assertion range
        let overlaps_any = ranges.iter().any(|(rs, re)| end > *rs && start < *re);
        if !overlaps_any {
            continue;
        }

        if literal_kinds.contains(&n.kind()) {
            let in_assertion = ranges.iter().any(|(rs, re)| start >= *rs && end <= *re);
            if in_assertion {
                if let Ok(text) = n.utf8_text(source) {
                    if !TRIVIAL_LITERALS.contains(&text) {
                        *counts.entry(text.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        for i in 0..n.child_count() {
            if let Some(child) = n.child(i) {
                stack.push(child);
            }
        }
    }

    counts.values().copied().max().unwrap_or(0)
}

pub fn extract_suppression_from_previous_line(source: &str, start_row: usize) -> Vec<RuleId> {
    if start_row == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let prev_line = lines.get(start_row - 1).unwrap_or(&"");
    parse_suppression(prev_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppression_from_first_line_returns_empty() {
        assert!(extract_suppression_from_previous_line("any source", 0).is_empty());
    }

    #[test]
    fn suppression_from_previous_line_parses_comment() {
        let source = "// exspec-ignore: T001\nfn test_foo() {}";
        let result = extract_suppression_from_previous_line(source, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "T001");
    }

    #[test]
    fn suppression_from_previous_line_no_comment() {
        let source = "// normal comment\nfn test_foo() {}";
        let result = extract_suppression_from_previous_line(source, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn suppression_out_of_bounds_returns_empty() {
        let source = "single line";
        let result = extract_suppression_from_previous_line(source, 5);
        assert!(result.is_empty());
    }

    // --- count_captures_within_context ---

    fn python_language() -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    #[test]
    fn count_captures_within_context_basic() {
        // assert obj._count == 1 -> _count is inside assert_statement (@assertion)
        let source = "def test_foo():\n    assert obj._count == 1\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let private_query = Query::new(
            &python_language(),
            "(attribute attribute: (identifier) @private_access (#match? @private_access \"^_[^_]\"))",
        )
        .unwrap();

        let count = count_captures_within_context(
            &assertion_query,
            "assertion",
            &private_query,
            "private_access",
            root,
            source.as_bytes(),
        );
        assert_eq!(count, 1, "should detect _count inside assert statement");
    }

    #[test]
    fn count_captures_within_context_outside() {
        // _count is outside assert -> should not count
        let source = "def test_foo():\n    x = obj._count\n    assert x == 1\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let private_query = Query::new(
            &python_language(),
            "(attribute attribute: (identifier) @private_access (#match? @private_access \"^_[^_]\"))",
        )
        .unwrap();

        let count = count_captures_within_context(
            &assertion_query,
            "assertion",
            &private_query,
            "private_access",
            root,
            source.as_bytes(),
        );
        assert_eq!(count, 0, "_count is outside assert, should not count");
    }

    #[test]
    fn count_captures_within_context_no_outer() {
        // No assert statement at all
        let source = "def test_foo():\n    x = obj._count\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let private_query = Query::new(
            &python_language(),
            "(attribute attribute: (identifier) @private_access (#match? @private_access \"^_[^_]\"))",
        )
        .unwrap();

        let count = count_captures_within_context(
            &assertion_query,
            "assertion",
            &private_query,
            "private_access",
            root,
            source.as_bytes(),
        );
        assert_eq!(count, 0, "no assertions, should return 0");
    }

    #[test]
    fn count_captures_missing_capture_returns_zero() {
        let lang = python_language();
        // Query with capture @assertion, but we ask for nonexistent name
        let query = Query::new(&lang, "(assert_statement) @assertion").unwrap();
        let source = "def test_foo():\n    assert True\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let count = count_captures(&query, "nonexistent", root, source.as_bytes());
        assert_eq!(count, 0, "missing capture name should return 0, not panic");
    }

    #[test]
    fn collect_mock_class_names_missing_capture_returns_empty() {
        let lang = python_language();
        // Query without @var_name capture
        let query = Query::new(&lang, "(assert_statement) @assertion").unwrap();
        let source = "def test_foo():\n    assert True\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let names = collect_mock_class_names(&query, root, source.as_bytes(), |s| s.to_string());
        assert!(
            names.is_empty(),
            "missing @var_name capture should return empty vec, not panic"
        );
    }

    #[test]
    fn count_captures_within_context_missing_capture() {
        // Capture name doesn't exist in query -> defensive 0
        let source = "def test_foo():\n    assert obj._count == 1\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let private_query = Query::new(
            &python_language(),
            "(attribute attribute: (identifier) @private_access (#match? @private_access \"^_[^_]\"))",
        )
        .unwrap();

        // Wrong capture name for outer
        let count = count_captures_within_context(
            &assertion_query,
            "nonexistent",
            &private_query,
            "private_access",
            root,
            source.as_bytes(),
        );
        assert_eq!(count, 0, "missing outer capture should return 0");

        // Wrong capture name for inner
        let count = count_captures_within_context(
            &assertion_query,
            "assertion",
            &private_query,
            "nonexistent",
            root,
            source.as_bytes(),
        );
        assert_eq!(count, 0, "missing inner capture should return 0");
    }

    // --- count_duplicate_literals ---

    #[test]
    fn count_duplicate_literals_detects_repeated_value() {
        let source = "def test_foo():\n    assert calc(1) == 42\n    assert calc(2) == 42\n    assert calc(3) == 42\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let count = count_duplicate_literals(
            &assertion_query,
            root,
            source.as_bytes(),
            &["integer", "float", "string"],
        );
        assert_eq!(count, 3, "42 appears 3 times in assertions");
    }

    #[test]
    fn count_duplicate_literals_trivial_excluded() {
        // All literals are trivial (0, 1, 2) - should return 0
        let source =
            "def test_foo():\n    assert calc(1) == 0\n    assert calc(2) == 0\n    assert calc(1) == 0\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let count = count_duplicate_literals(
            &assertion_query,
            root,
            source.as_bytes(),
            &["integer", "float", "string"],
        );
        assert_eq!(count, 0, "0, 1, 2 are all trivial and should be excluded");
    }

    #[test]
    fn count_duplicate_literals_no_assertions() {
        let source = "def test_foo():\n    x = 42\n    y = 42\n    z = 42\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let assertion_query =
            Query::new(&python_language(), "(assert_statement) @assertion").unwrap();
        let count = count_duplicate_literals(
            &assertion_query,
            root,
            source.as_bytes(),
            &["integer", "float", "string"],
        );
        assert_eq!(count, 0, "no assertions, should return 0");
    }

    #[test]
    fn count_duplicate_literals_missing_capture() {
        let source = "def test_foo():\n    assert 42 == 42\n";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&python_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Query without @assertion capture
        let query = Query::new(&python_language(), "(assert_statement) @something_else").unwrap();
        let count = count_duplicate_literals(&query, root, source.as_bytes(), &["integer"]);
        assert_eq!(count, 0, "missing @assertion capture should return 0");
    }
}
