use std::collections::BTreeSet;

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

/// Count captures of `inner_capture` from `inner_query` that fall within
/// byte ranges of `outer_capture` from `outer_query`.
///
/// 2-pass approach:
/// 1. Collect byte ranges from outer_query's outer_capture
/// 2. Count inner_query's inner_capture matches that fall within those ranges
pub fn count_captures_within_context(
    outer_query: &Query,
    outer_capture: &str,
    inner_query: &Query,
    inner_capture: &str,
    node: Node,
    source: &[u8],
) -> usize {
    let outer_idx = match outer_query.capture_index_for_name(outer_capture) {
        Some(i) => i,
        None => return 0,
    };
    let inner_idx = match inner_query.capture_index_for_name(inner_capture) {
        Some(i) => i,
        None => return 0,
    };

    // Pass 1: collect byte ranges of outer captures
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(outer_query, node, source);
        while let Some(m) = matches.next() {
            for c in m.captures.iter().filter(|c| c.index == outer_idx) {
                ranges.push((c.node.start_byte(), c.node.end_byte()));
            }
        }
    }

    if ranges.is_empty() {
        return 0;
    }

    // Pass 2: count inner captures that fall within any outer range
    let mut count = 0;
    {
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
    }

    count
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
}
