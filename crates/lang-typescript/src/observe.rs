use std::collections::HashMap;
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use super::{cached_query, TypeScriptExtractor};

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

/// A production (non-test) function or method extracted from source code.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductionFunction {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub class_name: Option<String>,
    pub is_exported: bool,
}

impl TypeScriptExtractor {
    /// Extract all production functions/methods from TypeScript source code.
    pub fn extract_production_functions(
        &self,
        source: &str,
        file_path: &str,
    ) -> Vec<ProductionFunction> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let query = cached_query(&PRODUCTION_FUNCTION_QUERY_CACHE, PRODUCTION_FUNCTION_QUERY);
        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        let idx_name = query
            .capture_index_for_name("name")
            .expect("@name capture not found in production_function.scm");
        let idx_exported_function = query
            .capture_index_for_name("exported_function")
            .expect("@exported_function capture not found");
        let idx_function = query
            .capture_index_for_name("function")
            .expect("@function capture not found");
        let idx_method = query
            .capture_index_for_name("method")
            .expect("@method capture not found");
        let idx_exported_arrow = query
            .capture_index_for_name("exported_arrow")
            .expect("@exported_arrow capture not found");
        let idx_arrow = query
            .capture_index_for_name("arrow")
            .expect("@arrow capture not found");

        // Use HashMap keyed by (line, name) to deduplicate overlapping patterns.
        // Exported patterns and non-exported patterns match the same node;
        // match order is implementation-dependent, so we upgrade is_exported
        // to true if any pattern marks it exported.
        let mut dedup: HashMap<(usize, String), ProductionFunction> = HashMap::new();

        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        while let Some(m) = matches.next() {
            let name_node = match m.captures.iter().find(|c| c.index == idx_name) {
                Some(c) => c.node,
                None => continue,
            };
            let name = name_node.utf8_text(source_bytes).unwrap_or("").to_string();
            // Use the @name node's line for consistent deduplication across patterns
            let line = name_node.start_position().row + 1; // 1-indexed

            let (is_exported, class_name) = if m
                .captures
                .iter()
                .any(|c| c.index == idx_exported_function || c.index == idx_exported_arrow)
            {
                (true, None)
            } else if m
                .captures
                .iter()
                .any(|c| c.index == idx_function || c.index == idx_arrow)
            {
                (false, None)
            } else if let Some(c) = m.captures.iter().find(|c| c.index == idx_method) {
                let (cname, exported) = find_class_info(c.node, source_bytes);
                (exported, cname)
            } else {
                continue;
            };

            dedup
                .entry((line, name.clone()))
                .and_modify(|existing| {
                    if is_exported {
                        existing.is_exported = true;
                    }
                })
                .or_insert(ProductionFunction {
                    name,
                    file: file_path.to_string(),
                    line,
                    class_name,
                    is_exported,
                });
        }

        let mut results: Vec<ProductionFunction> = dedup.into_values().collect();
        results.sort_by_key(|f| f.line);
        results
    }
}

/// Walk up from a method_definition node to find the containing class name and export status.
fn find_class_info(method_node: Node, source: &[u8]) -> (Option<String>, bool) {
    let mut current = method_node.parent();
    while let Some(node) = current {
        if node.kind() == "class_body" {
            if let Some(class_node) = node.parent() {
                let class_kind = class_node.kind();
                if class_kind == "class_declaration"
                    || class_kind == "class"
                    || class_kind == "abstract_class_declaration"
                {
                    let class_name = class_node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source).ok())
                        .map(|s| s.to_string());

                    // Check if class is inside an export_statement
                    let is_exported = class_node
                        .parent()
                        .is_some_and(|p| p.kind() == "export_statement");

                    return (class_name, is_exported);
                }
            }
        }
        current = node.parent();
    }
    (None, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/typescript/observe/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/lang-typescript", ""),
            name
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
    }

    // TC1: exported function declarations are extracted with is_exported: true
    #[test]
    fn exported_functions_extracted() {
        // Given: exported_functions.ts with `export function findAll()` and `export function findById()`
        let source = fixture("exported_functions.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "exported_functions.ts");

        // Then: findAll and findById are extracted with is_exported: true
        let exported: Vec<&ProductionFunction> = funcs.iter().filter(|f| f.is_exported).collect();
        let names: Vec<&str> = exported.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"findAll"), "expected findAll in {names:?}");
        assert!(
            names.contains(&"findById"),
            "expected findById in {names:?}"
        );
    }

    // TC2: non-exported function has is_exported: false
    #[test]
    fn non_exported_function_has_flag_false() {
        // Given: exported_functions.ts with `function internalHelper()`
        let source = fixture("exported_functions.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "exported_functions.ts");

        // Then: internalHelper has is_exported: false
        let helper = funcs.iter().find(|f| f.name == "internalHelper");
        assert!(helper.is_some(), "expected internalHelper to be extracted");
        assert!(!helper.unwrap().is_exported);
    }

    // TC3: class methods include class_name
    #[test]
    fn class_methods_with_class_name() {
        // Given: class_methods.ts with class UsersController { findAll(), create(), validate() }
        let source = fixture("class_methods.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "class_methods.ts");

        // Then: findAll, create, validate have class_name: Some("UsersController")
        let controller_methods: Vec<&ProductionFunction> = funcs
            .iter()
            .filter(|f| f.class_name.as_deref() == Some("UsersController"))
            .collect();
        let names: Vec<&str> = controller_methods.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"findAll"), "expected findAll in {names:?}");
        assert!(names.contains(&"create"), "expected create in {names:?}");
        assert!(
            names.contains(&"validate"),
            "expected validate in {names:?}"
        );
    }

    // TC4: exported class methods are is_exported: true, non-exported class methods are false
    #[test]
    fn exported_class_is_exported() {
        // Given: class_methods.ts with exported UsersController and non-exported InternalService
        let source = fixture("class_methods.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "class_methods.ts");

        // Then: UsersController methods → is_exported: true
        let controller_methods: Vec<&ProductionFunction> = funcs
            .iter()
            .filter(|f| f.class_name.as_deref() == Some("UsersController"))
            .collect();
        assert!(
            controller_methods.iter().all(|f| f.is_exported),
            "all UsersController methods should be exported"
        );

        // Then: InternalService methods → is_exported: false
        let internal_methods: Vec<&ProductionFunction> = funcs
            .iter()
            .filter(|f| f.class_name.as_deref() == Some("InternalService"))
            .collect();
        assert!(
            !internal_methods.is_empty(),
            "expected InternalService methods"
        );
        assert!(
            internal_methods.iter().all(|f| !f.is_exported),
            "all InternalService methods should not be exported"
        );
    }

    // TC5: arrow function exports are extracted with is_exported: true
    #[test]
    fn arrow_exports_extracted() {
        // Given: arrow_exports.ts with `export const findAll = () => ...`
        let source = fixture("arrow_exports.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "arrow_exports.ts");

        // Then: findAll, findById are is_exported: true
        let exported: Vec<&ProductionFunction> = funcs.iter().filter(|f| f.is_exported).collect();
        let names: Vec<&str> = exported.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"findAll"), "expected findAll in {names:?}");
        assert!(
            names.contains(&"findById"),
            "expected findById in {names:?}"
        );
    }

    // TC6: non-exported arrow function has is_exported: false
    #[test]
    fn non_exported_arrow_flag_false() {
        // Given: arrow_exports.ts with `const internalFn = () => ...`
        let source = fixture("arrow_exports.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "arrow_exports.ts");

        // Then: internalFn has is_exported: false
        let internal = funcs.iter().find(|f| f.name == "internalFn");
        assert!(internal.is_some(), "expected internalFn to be extracted");
        assert!(!internal.unwrap().is_exported);
    }

    // TC7: mixed file extracts all types with correct export status
    #[test]
    fn mixed_file_all_types() {
        // Given: mixed.ts with function declarations, arrow functions, and class methods
        let source = fixture("mixed.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "mixed.ts");

        // Then: all functions are extracted
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        // Exported: getUser, createUser, UserService.findAll, UserService.deleteById
        assert!(names.contains(&"getUser"), "expected getUser in {names:?}");
        assert!(
            names.contains(&"createUser"),
            "expected createUser in {names:?}"
        );
        // Non-exported: formatName, validateInput, PrivateHelper.transform
        assert!(
            names.contains(&"formatName"),
            "expected formatName in {names:?}"
        );
        assert!(
            names.contains(&"validateInput"),
            "expected validateInput in {names:?}"
        );

        // Verify export status
        let get_user = funcs.iter().find(|f| f.name == "getUser").unwrap();
        assert!(get_user.is_exported);
        let format_name = funcs.iter().find(|f| f.name == "formatName").unwrap();
        assert!(!format_name.is_exported);

        // Verify class methods have class_name
        let find_all = funcs
            .iter()
            .find(|f| f.name == "findAll" && f.class_name.is_some())
            .unwrap();
        assert_eq!(find_all.class_name.as_deref(), Some("UserService"));
        assert!(find_all.is_exported);

        let transform = funcs.iter().find(|f| f.name == "transform").unwrap();
        assert_eq!(transform.class_name.as_deref(), Some("PrivateHelper"));
        assert!(!transform.is_exported);
    }

    // TC8: decorated methods (NestJS) are correctly extracted
    #[test]
    fn decorated_methods_extracted() {
        // Given: nestjs_controller.ts with @Get(), @Post(), @Delete() decorated methods
        let source = fixture("nestjs_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "nestjs_controller.ts");

        // Then: findAll, create, remove are extracted with class_name and is_exported
        let names: Vec<&str> = funcs.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"findAll"), "expected findAll in {names:?}");
        assert!(names.contains(&"create"), "expected create in {names:?}");
        assert!(names.contains(&"remove"), "expected remove in {names:?}");

        for func in &funcs {
            assert_eq!(func.class_name.as_deref(), Some("UsersController"));
            assert!(func.is_exported);
        }
    }

    // TC9: line numbers match actual source positions
    #[test]
    fn line_numbers_correct() {
        // Given: exported_functions.ts
        let source = fixture("exported_functions.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "exported_functions.ts");

        // Then: line numbers correspond to actual positions (1-indexed)
        let find_all = funcs.iter().find(|f| f.name == "findAll").unwrap();
        assert_eq!(find_all.line, 1, "findAll should be on line 1");

        let find_by_id = funcs.iter().find(|f| f.name == "findById").unwrap();
        assert_eq!(find_by_id.line, 5, "findById should be on line 5");

        let helper = funcs.iter().find(|f| f.name == "internalHelper").unwrap();
        assert_eq!(helper.line, 9, "internalHelper should be on line 9");
    }

    // TC10: empty source returns empty Vec
    #[test]
    fn empty_source_returns_empty() {
        // Given: empty source code
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions from empty string
        let funcs = extractor.extract_production_functions("", "empty.ts");

        // Then: returns empty Vec
        assert!(funcs.is_empty());
    }

    // TC11: abstract class methods are extracted with class_name and export status
    #[test]
    fn abstract_class_methods_extracted() {
        // Given: abstract_class.ts with exported and non-exported abstract classes
        let source = fixture("abstract_class.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract production functions
        let funcs = extractor.extract_production_functions(&source, "abstract_class.ts");

        // Then: concrete methods are extracted (abstract methods have no body → method_signature, not method_definition)
        let validate = funcs.iter().find(|f| f.name == "validate");
        assert!(validate.is_some(), "expected validate to be extracted");
        let validate = validate.unwrap();
        assert_eq!(validate.class_name.as_deref(), Some("BaseService"));
        assert!(validate.is_exported);

        let process = funcs.iter().find(|f| f.name == "process");
        assert!(process.is_some(), "expected process to be extracted");
        let process = process.unwrap();
        assert_eq!(process.class_name.as_deref(), Some("InternalBase"));
        assert!(!process.is_exported);
    }
}
