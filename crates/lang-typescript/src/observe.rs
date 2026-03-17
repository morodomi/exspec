use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use super::{cached_query, TypeScriptExtractor};

// Re-export core types for backward compatibility
pub use exspec_core::observe::{
    BarrelReExport, FileMapping, ImportMapping, MappingStrategy, ObserveExtractor,
    ProductionFunction,
};

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const IMPORT_MAPPING_QUERY: &str = include_str!("../queries/import_mapping.scm");
static IMPORT_MAPPING_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const RE_EXPORT_QUERY: &str = include_str!("../queries/re_export.scm");
static RE_EXPORT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const EXPORTED_SYMBOL_QUERY: &str = include_str!("../queries/exported_symbol.scm");
static EXPORTED_SYMBOL_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

/// A route extracted from a NestJS controller.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub http_method: String,
    pub path: String,
    pub handler_name: String,
    pub class_name: String,
    pub file: String,
    pub line: usize,
}

/// A gap-relevant decorator extracted from source code.
#[derive(Debug, Clone, PartialEq)]
pub struct DecoratorInfo {
    pub name: String,
    pub arguments: Vec<String>,
    pub target_name: String,
    pub class_name: String,
    pub file: String,
    pub line: usize,
}

/// HTTP method decorators recognized as route indicators.
const HTTP_METHODS: &[&str] = &["Get", "Post", "Put", "Patch", "Delete", "Head", "Options"];

/// Decorators relevant to gap analysis (guard/pipe/validation).
const GAP_RELEVANT_DECORATORS: &[&str] = &[
    "UseGuards",
    "UsePipes",
    "IsEmail",
    "IsNotEmpty",
    "MinLength",
    "MaxLength",
    "IsOptional",
    "IsString",
    "IsNumber",
    "IsInt",
    "IsBoolean",
    "IsDate",
    "IsEnum",
    "IsArray",
    "ValidateNested",
    "Min",
    "Max",
    "Matches",
    "IsUrl",
    "IsUUID",
];

impl ObserveExtractor for TypeScriptExtractor {
    fn extract_production_functions(
        &self,
        source: &str,
        file_path: &str,
    ) -> Vec<ProductionFunction> {
        self.extract_production_functions_impl(source, file_path)
    }

    fn extract_imports(&self, source: &str, file_path: &str) -> Vec<ImportMapping> {
        self.extract_imports_impl(source, file_path)
    }

    fn extract_all_import_specifiers(&self, source: &str) -> Vec<(String, Vec<String>)> {
        self.extract_all_import_specifiers_impl(source)
    }

    fn extract_barrel_re_exports(&self, source: &str, file_path: &str) -> Vec<BarrelReExport> {
        self.extract_barrel_re_exports_impl(source, file_path)
    }

    fn source_extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn index_file_names(&self) -> &[&str] {
        &["index.ts", "index.tsx"]
    }

    fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
        production_stem(path)
    }

    fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
        test_stem(path)
    }

    fn is_non_sut_helper(&self, file_path: &str, is_known_production: bool) -> bool {
        is_non_sut_helper(file_path, is_known_production)
    }

    fn file_exports_any_symbol(&self, file_path: &Path, symbols: &[String]) -> bool {
        file_exports_any_symbol(file_path, symbols)
    }
}

impl TypeScriptExtractor {
    /// Layer 1: Map test files to production files by filename convention.
    pub fn map_test_files(
        &self,
        production_files: &[String],
        test_files: &[String],
    ) -> Vec<FileMapping> {
        exspec_core::observe::map_test_files(self, production_files, test_files)
    }

    /// Extract NestJS routes from a controller source file.
    pub fn extract_routes(&self, source: &str, file_path: &str) -> Vec<Route> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();

        let mut routes = Vec::new();

        // Find all class declarations (including exported ones)
        for node in iter_children(tree.root_node()) {
            // Find class_declaration and its parent (for decorator search)
            let (container, class_node) = match node.kind() {
                "export_statement" => {
                    let cls = node
                        .named_children(&mut node.walk())
                        .find(|c| c.kind() == "class_declaration");
                    match cls {
                        Some(c) => (node, c),
                        None => continue,
                    }
                }
                "class_declaration" => (node, node),
                _ => continue,
            };

            // @Controller decorator may be on container (export_statement) or class_declaration
            let (base_path, class_name) =
                match extract_controller_info(container, class_node, source_bytes) {
                    Some(info) => info,
                    None => continue,
                };

            let class_body = match class_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            let mut decorator_acc: Vec<Node> = Vec::new();
            for child in iter_children(class_body) {
                match child.kind() {
                    "decorator" => decorator_acc.push(child),
                    "method_definition" => {
                        let handler_name = child
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source_bytes).ok())
                            .unwrap_or("")
                            .to_string();
                        let line = child.start_position().row + 1;

                        for dec in &decorator_acc {
                            if let Some((dec_name, dec_arg)) =
                                extract_decorator_call(*dec, source_bytes)
                            {
                                if HTTP_METHODS.contains(&dec_name.as_str()) {
                                    let sub_path = dec_arg.unwrap_or_default();
                                    routes.push(Route {
                                        http_method: dec_name.to_uppercase(),
                                        path: normalize_path(&base_path, &sub_path),
                                        handler_name: handler_name.clone(),
                                        class_name: class_name.clone(),
                                        file: file_path.to_string(),
                                        line,
                                    });
                                }
                            }
                        }
                        decorator_acc.clear();
                    }
                    _ => {}
                }
            }
        }

        routes
    }

    /// Extract gap-relevant decorators (guards, pipes, validators) from source.
    pub fn extract_decorators(&self, source: &str, file_path: &str) -> Vec<DecoratorInfo> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();

        let mut decorators = Vec::new();

        for node in iter_children(tree.root_node()) {
            let (container, class_node) = match node.kind() {
                "export_statement" => {
                    let cls = node
                        .named_children(&mut node.walk())
                        .find(|c| c.kind() == "class_declaration");
                    match cls {
                        Some(c) => (node, c),
                        None => continue,
                    }
                }
                "class_declaration" => (node, node),
                _ => continue,
            };

            let class_name = class_node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source_bytes).ok())
                .unwrap_or("")
                .to_string();

            // BLOCK 1 fix: extract class-level gap-relevant decorators
            // Decorators on the class/container (e.g., @UseGuards at class level)
            let class_level_decorators: Vec<Node> = find_decorators_on_node(container, class_node);
            collect_gap_decorators(
                &class_level_decorators,
                &class_name, // target_name = class name for class-level
                &class_name,
                file_path,
                source_bytes,
                &mut decorators,
            );

            let class_body = match class_node.child_by_field_name("body") {
                Some(b) => b,
                None => continue,
            };

            let mut decorator_acc: Vec<Node> = Vec::new();
            for child in iter_children(class_body) {
                match child.kind() {
                    "decorator" => decorator_acc.push(child),
                    "method_definition" => {
                        let method_name = child
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source_bytes).ok())
                            .unwrap_or("")
                            .to_string();

                        collect_gap_decorators(
                            &decorator_acc,
                            &method_name,
                            &class_name,
                            file_path,
                            source_bytes,
                            &mut decorators,
                        );
                        decorator_acc.clear();
                    }
                    // DTO field definitions: decorators are children of the field node
                    "public_field_definition" => {
                        let field_name = child
                            .child_by_field_name("name")
                            .and_then(|n| n.utf8_text(source_bytes).ok())
                            .unwrap_or("")
                            .to_string();

                        let field_decorators: Vec<Node> = iter_children(child)
                            .filter(|c| c.kind() == "decorator")
                            .collect();
                        collect_gap_decorators(
                            &field_decorators,
                            &field_name,
                            &class_name,
                            file_path,
                            source_bytes,
                            &mut decorators,
                        );
                        decorator_acc.clear();
                    }
                    _ => {}
                }
            }
        }

        decorators
    }

    fn extract_production_functions_impl(
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

/// Iterate over all children of a node (named + anonymous).
fn iter_children(node: Node) -> impl Iterator<Item = Node> {
    (0..node.child_count()).filter_map(move |i| node.child(i))
}

/// Extract @Controller base path and class name.
/// `container` is the node that holds decorators (export_statement or class_declaration).
/// `class_node` is the class_declaration itself.
fn extract_controller_info(
    container: Node,
    class_node: Node,
    source: &[u8],
) -> Option<(String, String)> {
    let class_name = class_node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())?
        .to_string();

    // Look for @Controller decorator in both container and class_node
    for search_node in [container, class_node] {
        for i in 0..search_node.child_count() {
            let child = match search_node.child(i) {
                Some(c) => c,
                None => continue,
            };
            if child.kind() != "decorator" {
                continue;
            }
            if let Some((name, arg)) = extract_decorator_call(child, source) {
                if name == "Controller" {
                    let base_path = arg.unwrap_or_default();
                    return Some((base_path, class_name));
                }
            }
        }
    }
    None
}

/// Collect gap-relevant decorators from an accumulator into the output vec.
fn collect_gap_decorators(
    decorator_acc: &[Node],
    target_name: &str,
    class_name: &str,
    file_path: &str,
    source: &[u8],
    output: &mut Vec<DecoratorInfo>,
) {
    for dec in decorator_acc {
        if let Some((dec_name, _)) = extract_decorator_call(*dec, source) {
            if GAP_RELEVANT_DECORATORS.contains(&dec_name.as_str()) {
                let args = extract_decorator_args(*dec, source);
                output.push(DecoratorInfo {
                    name: dec_name,
                    arguments: args,
                    target_name: target_name.to_string(),
                    class_name: class_name.to_string(),
                    file: file_path.to_string(),
                    line: dec.start_position().row + 1,
                });
            }
        }
    }
}

/// Extract the name and first string argument from a decorator call.
/// Returns (name, Some(path)) for string literals, (name, Some("<dynamic>")) for
/// non-literal arguments (variables, objects), and (name, None) for no arguments.
fn extract_decorator_call(decorator_node: Node, source: &[u8]) -> Option<(String, Option<String>)> {
    for i in 0..decorator_node.child_count() {
        let child = match decorator_node.child(i) {
            Some(c) => c,
            None => continue,
        };

        match child.kind() {
            "call_expression" => {
                let func_node = child.child_by_field_name("function")?;
                let name = func_node.utf8_text(source).ok()?.to_string();
                let args_node = child.child_by_field_name("arguments")?;

                if args_node.named_child_count() == 0 {
                    // No arguments: @Get()
                    return Some((name, None));
                }
                // Try first string argument
                let first_string = find_first_string_arg(args_node, source);
                if first_string.is_some() {
                    return Some((name, first_string));
                }
                // Non-literal argument (variable, object, etc.): mark as dynamic
                return Some((name, Some("<dynamic>".to_string())));
            }
            "identifier" => {
                let name = child.utf8_text(source).ok()?.to_string();
                return Some((name, None));
            }
            _ => {}
        }
    }
    None
}

/// Extract all identifier arguments from a decorator call.
/// e.g., @UseGuards(AuthGuard, RoleGuard) -> ["AuthGuard", "RoleGuard"]
fn extract_decorator_args(decorator_node: Node, source: &[u8]) -> Vec<String> {
    let mut args = Vec::new();
    for i in 0..decorator_node.child_count() {
        let child = match decorator_node.child(i) {
            Some(c) => c,
            None => continue,
        };
        if child.kind() == "call_expression" {
            if let Some(args_node) = child.child_by_field_name("arguments") {
                for j in 0..args_node.named_child_count() {
                    if let Some(arg) = args_node.named_child(j) {
                        if let Ok(text) = arg.utf8_text(source) {
                            args.push(text.to_string());
                        }
                    }
                }
            }
        }
    }
    args
}

/// Find the first string literal argument in an arguments node.
fn find_first_string_arg(args_node: Node, source: &[u8]) -> Option<String> {
    for i in 0..args_node.named_child_count() {
        let arg = args_node.named_child(i)?;
        if arg.kind() == "string" {
            let text = arg.utf8_text(source).ok()?;
            // Strip quotes
            let stripped = text.trim_matches(|c| c == '\'' || c == '"');
            if !stripped.is_empty() {
                return Some(stripped.to_string());
            }
        }
    }
    None
}

/// Normalize and combine base path and sub path.
/// e.g., ("users", ":id") -> "/users/:id"
/// e.g., ("", "health") -> "/health"
/// e.g., ("api/v1/users", "") -> "/api/v1/users"
fn normalize_path(base: &str, sub: &str) -> String {
    let base = base.trim_matches('/');
    let sub = sub.trim_matches('/');
    match (base.is_empty(), sub.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{sub}"),
        (false, true) => format!("/{base}"),
        (false, false) => format!("/{base}/{sub}"),
    }
}

/// Collect decorator nodes from both container and class_node.
/// For `export class`, decorators are on the export_statement, not class_declaration.
fn find_decorators_on_node<'a>(container: Node<'a>, class_node: Node<'a>) -> Vec<Node<'a>> {
    let mut result = Vec::new();
    for search_node in [container, class_node] {
        for i in 0..search_node.child_count() {
            if let Some(child) = search_node.child(i) {
                if child.kind() == "decorator" {
                    result.push(child);
                }
            }
        }
    }
    result
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

/// Check if a symbol node belongs to a type-only import.
/// Handles both `import type { X }` (statement-level) and `import { type X }` (specifier-level).
fn is_type_only_import(symbol_node: Node) -> bool {
    // Case 1: `import { type X }` — import_specifier has a "type" child
    let parent = symbol_node.parent();
    if let Some(p) = parent {
        if p.kind() == "import_specifier" {
            for i in 0..p.child_count() {
                if let Some(child) = p.child(i) {
                    if child.kind() == "type" {
                        return true;
                    }
                }
            }
        }
    }

    // Case 2: `import type { X }` — import_statement has a "type" child (before import_clause)
    // Walk up to import_statement
    let mut current = Some(symbol_node);
    while let Some(node) = current {
        if node.kind() == "import_statement" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "type" {
                        return true;
                    }
                }
            }
            break;
        }
        current = node.parent();
    }
    false
}

impl TypeScriptExtractor {
    fn extract_imports_impl(&self, source: &str, file_path: &str) -> Vec<ImportMapping> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);
        let symbol_idx = query.capture_index_for_name("symbol_name").unwrap();
        let specifier_idx = query.capture_index_for_name("module_specifier").unwrap();

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        let mut result = Vec::new();

        while let Some(m) = matches.next() {
            let mut symbol_node = None;
            let mut symbol = None;
            let mut specifier = None;
            let mut symbol_line = 0usize;
            for cap in m.captures {
                if cap.index == symbol_idx {
                    symbol_node = Some(cap.node);
                    symbol = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                    symbol_line = cap.node.start_position().row + 1;
                } else if cap.index == specifier_idx {
                    specifier = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                }
            }
            if let (Some(sym), Some(spec)) = (symbol, specifier) {
                // Filter: only relative paths (./ or ../)
                if !spec.starts_with("./") && !spec.starts_with("../") {
                    continue;
                }

                // Filter: skip type-only imports
                // `import type { X }` → import_statement has "type" keyword child
                // `import { type X }` → import_specifier has "type" keyword child
                if let Some(snode) = symbol_node {
                    if is_type_only_import(snode) {
                        continue;
                    }
                }

                result.push(ImportMapping {
                    symbol_name: sym.to_string(),
                    module_specifier: spec.to_string(),
                    file: file_path.to_string(),
                    line: symbol_line,
                    symbols: Vec::new(),
                });
            }
        }
        // Populate `symbols`: for each entry, collect all symbol_names that share the same
        // module_specifier in this file.
        let specifier_to_symbols: HashMap<String, Vec<String>> =
            result.iter().fold(HashMap::new(), |mut acc, im| {
                acc.entry(im.module_specifier.clone())
                    .or_default()
                    .push(im.symbol_name.clone());
                acc
            });
        for im in &mut result {
            im.symbols = specifier_to_symbols
                .get(&im.module_specifier)
                .cloned()
                .unwrap_or_default();
        }
        result
    }

    fn extract_all_import_specifiers_impl(&self, source: &str) -> Vec<(String, Vec<String>)> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);
        let symbol_idx = query.capture_index_for_name("symbol_name").unwrap();
        let specifier_idx = query.capture_index_for_name("module_specifier").unwrap();

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        // Map specifier -> symbols
        let mut specifier_symbols: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        while let Some(m) = matches.next() {
            let mut symbol_node = None;
            let mut symbol = None;
            let mut specifier = None;
            for cap in m.captures {
                if cap.index == symbol_idx {
                    symbol_node = Some(cap.node);
                    symbol = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                } else if cap.index == specifier_idx {
                    specifier = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                }
            }
            if let (Some(sym), Some(spec)) = (symbol, specifier) {
                // Skip relative imports (already handled by extract_imports)
                if spec.starts_with("./") || spec.starts_with("../") {
                    continue;
                }
                // Skip type-only imports
                if let Some(snode) = symbol_node {
                    if is_type_only_import(snode) {
                        continue;
                    }
                }
                specifier_symbols
                    .entry(spec.to_string())
                    .or_default()
                    .push(sym.to_string());
            }
        }

        specifier_symbols.into_iter().collect()
    }

    fn extract_barrel_re_exports_impl(
        &self,
        source: &str,
        _file_path: &str,
    ) -> Vec<BarrelReExport> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&RE_EXPORT_QUERY_CACHE, RE_EXPORT_QUERY);

        let symbol_idx = query.capture_index_for_name("symbol_name");
        let wildcard_idx = query.capture_index_for_name("wildcard");
        let specifier_idx = query
            .capture_index_for_name("from_specifier")
            .expect("@from_specifier capture not found in re_export.scm");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Group by match: each match corresponds to one export statement pattern.
        // Named re-export produces one match per symbol; wildcard produces one match.
        // We use a HashMap keyed by (from_specifier, is_wildcard) to group named symbols.
        struct ReExportEntry {
            symbols: Vec<String>,
            wildcard: bool,
        }
        let mut grouped: HashMap<String, ReExportEntry> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut from_spec = None;
            let mut sym_name = None;
            let mut is_wildcard = false;

            for cap in m.captures {
                if wildcard_idx == Some(cap.index) {
                    is_wildcard = true;
                } else if cap.index == specifier_idx {
                    from_spec = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                } else if symbol_idx == Some(cap.index) {
                    sym_name = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                }
            }

            let Some(spec) = from_spec else { continue };

            let entry = grouped.entry(spec).or_insert(ReExportEntry {
                symbols: Vec::new(),
                wildcard: false,
            });
            if is_wildcard {
                entry.wildcard = true;
            }
            if let Some(sym) = sym_name {
                if !sym.is_empty() && !entry.symbols.contains(&sym) {
                    entry.symbols.push(sym);
                }
            }
        }

        grouped
            .into_iter()
            .map(|(from_spec, entry)| BarrelReExport {
                symbols: entry.symbols,
                from_specifier: from_spec,
                wildcard: entry.wildcard,
            })
            .collect()
    }

    pub fn map_test_files_with_imports(
        &self,
        production_files: &[String],
        test_sources: &HashMap<String, String>,
        scan_root: &Path,
    ) -> Vec<FileMapping> {
        let test_file_list: Vec<String> = test_sources.keys().cloned().collect();

        // Layer 1: filename convention
        let mut mappings = self.map_test_files(production_files, &test_file_list);

        // Build canonical path -> production index lookup
        let canonical_root = match scan_root.canonicalize() {
            Ok(r) => r,
            Err(_) => return mappings,
        };
        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        for (idx, prod) in production_files.iter().enumerate() {
            if let Ok(canonical) = Path::new(prod).canonicalize() {
                canonical_to_idx.insert(canonical.to_string_lossy().into_owned(), idx);
            }
        }

        // Collect Layer 1 matched test files
        let layer1_matched: std::collections::HashSet<String> = mappings
            .iter()
            .flat_map(|m| m.test_files.iter().cloned())
            .collect();

        // Discover and parse tsconfig.json for alias resolution (Layer 2b)
        let tsconfig_paths =
            crate::tsconfig::discover_tsconfig(&canonical_root).and_then(|tsconfig_path| {
                let content = std::fs::read_to_string(&tsconfig_path)
                    .map_err(|e| {
                        eprintln!("[exspec] warning: failed to read tsconfig: {e}");
                    })
                    .ok()?;
                let tsconfig_dir = tsconfig_path.parent().unwrap_or(&canonical_root);
                crate::tsconfig::TsconfigPaths::from_str(&content, tsconfig_dir)
                    .or_else(|| {
                        eprintln!("[exspec] warning: failed to parse tsconfig paths, alias resolution disabled");
                        None
                    })
            });

        // Layer 2: import tracing for all test files (Layer 1 matched tests may
        // also import other production files not matched by filename convention)
        for (test_file, source) in test_sources {
            let imports = <Self as ObserveExtractor>::extract_imports(self, source, test_file);
            let from_file = Path::new(test_file);
            let mut matched_indices = std::collections::HashSet::new();

            // Helper: given a resolved file path, follow barrel re-exports if needed and
            // collect matching production-file indices.
            let collect_matches = |resolved: &str,
                                   symbols: &[String],
                                   indices: &mut HashSet<usize>| {
                if self.is_barrel_file(resolved) {
                    let barrel_path = PathBuf::from(resolved);
                    let resolved_files = exspec_core::observe::resolve_barrel_exports(
                        self,
                        &barrel_path,
                        symbols,
                        &canonical_root,
                    );
                    for prod in resolved_files {
                        let prod_str = prod.to_string_lossy().into_owned();
                        if !self
                            .is_non_sut_helper(&prod_str, canonical_to_idx.contains_key(&prod_str))
                        {
                            if let Some(&idx) = canonical_to_idx.get(&prod_str) {
                                indices.insert(idx);
                            }
                        }
                    }
                } else if !self.is_non_sut_helper(resolved, canonical_to_idx.contains_key(resolved))
                {
                    if let Some(&idx) = canonical_to_idx.get(resolved) {
                        indices.insert(idx);
                    }
                }
            };

            for import in &imports {
                if let Some(resolved) = exspec_core::observe::resolve_import_path(
                    self,
                    &import.module_specifier,
                    from_file,
                    &canonical_root,
                ) {
                    collect_matches(&resolved, &import.symbols, &mut matched_indices);
                }
            }

            // Layer 2b: tsconfig alias resolution
            if let Some(ref tc_paths) = tsconfig_paths {
                let alias_imports =
                    <Self as ObserveExtractor>::extract_all_import_specifiers(self, source);
                for (specifier, symbols) in &alias_imports {
                    let Some(alias_base) = tc_paths.resolve_alias(specifier) else {
                        continue;
                    };
                    if let Some(resolved) =
                        resolve_absolute_base_to_file(self, &alias_base, &canonical_root)
                    {
                        collect_matches(&resolved, symbols, &mut matched_indices);
                    }
                }
            }

            for idx in matched_indices {
                // Avoid duplicates: skip if already added by Layer 1
                if !mappings[idx].test_files.contains(test_file) {
                    mappings[idx].test_files.push(test_file.clone());
                }
            }
        }

        // Update strategy: if a production file had no Layer 1 matches but has Layer 2 matches,
        // set strategy to ImportTracing
        for mapping in &mut mappings {
            let has_layer1 = mapping
                .test_files
                .iter()
                .any(|t| layer1_matched.contains(t));
            if !has_layer1 && !mapping.test_files.is_empty() {
                mapping.strategy = MappingStrategy::ImportTracing;
            }
        }

        mappings
    }
}

/// Resolve a module specifier to an absolute file path.
/// Thin wrapper over core for backward compatibility.
pub fn resolve_import_path(
    module_specifier: &str,
    from_file: &Path,
    scan_root: &Path,
) -> Option<String> {
    let ext = crate::TypeScriptExtractor::new();
    exspec_core::observe::resolve_import_path(&ext, module_specifier, from_file, scan_root)
}

/// Resolve an already-computed absolute base path. Delegates to core.
fn resolve_absolute_base_to_file(
    ext: &dyn ObserveExtractor,
    base: &Path,
    canonical_root: &Path,
) -> Option<String> {
    exspec_core::observe::resolve_absolute_base_to_file(ext, base, canonical_root)
}

/// Type definition file: *.enum.*, *.interface.*, *.exception.*
/// Returns true if the file has a suffix pattern indicating a type definition.
fn is_type_definition_file(file_path: &str) -> bool {
    let Some(file_name) = Path::new(file_path).file_name().and_then(|f| f.to_str()) else {
        return false;
    };
    if let Some(stem) = Path::new(file_name).file_stem().and_then(|s| s.to_str()) {
        for suffix in &[".enum", ".interface", ".exception"] {
            if stem.ends_with(suffix) && stem != &suffix[1..] {
                return true;
            }
        }
    }
    false
}

/// Returns true if the resolved file path is a helper/non-SUT file that should be
/// excluded from Layer 2 import tracing.
///
/// Filtered patterns:
/// - Exact filenames: `constants.*`, `index.*`
/// - Suffix patterns: `*.enum.*`, `*.interface.*`, `*.exception.*` (skipped when `is_known_production`)
/// - Test utility paths: files under `/test/` or `/__tests__/`
fn is_non_sut_helper(file_path: &str, is_known_production: bool) -> bool {
    // Test-utility paths: files under /test/ or /__tests__/ directories.
    // Uses segment-based matching to avoid false positives (e.g., "contest/src/foo.ts").
    // Note: Windows path separators are intentionally not handled; this tool targets Unix-style paths.
    if file_path
        .split('/')
        .any(|seg| seg == "test" || seg == "__tests__")
    {
        return true;
    }

    let Some(file_name) = Path::new(file_path).file_name().and_then(|f| f.to_str()) else {
        return false;
    };

    // Exact-match barrel/constant files
    if matches!(
        file_name,
        "constants.ts"
            | "constants.js"
            | "constants.tsx"
            | "constants.jsx"
            | "index.ts"
            | "index.js"
            | "index.tsx"
            | "index.jsx"
    ) {
        return true;
    }

    // Suffix-match: *.enum.*, *.interface.*, *.exception.*
    // When is_known_production=true, type definition files are bypassed
    // (they are valid SUT targets when listed in production_files).
    if !is_known_production && is_type_definition_file(file_path) {
        return true;
    }

    false
}

/// Check if a TypeScript file exports any of the given symbol names.
/// Used to filter wildcard re-export targets by requested symbols.
fn file_exports_any_symbol(file_path: &Path, symbols: &[String]) -> bool {
    if symbols.is_empty() {
        return true;
    }
    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let mut parser = TypeScriptExtractor::parser();
    let tree = match parser.parse(&source, None) {
        Some(t) => t,
        None => return false,
    };
    let query = cached_query(&EXPORTED_SYMBOL_QUERY_CACHE, EXPORTED_SYMBOL_QUERY);
    let symbol_idx = query
        .capture_index_for_name("symbol_name")
        .expect("@symbol_name capture not found in exported_symbol.scm");

    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            if cap.index == symbol_idx {
                let name = cap.node.utf8_text(source_bytes).unwrap_or("");
                if symbols.iter().any(|s| s == name) {
                    return true;
                }
            }
        }
    }
    false
}

/// Resolve barrel re-exports. Thin wrapper over core for backward compatibility.
pub fn resolve_barrel_exports(
    barrel_path: &Path,
    symbols: &[String],
    scan_root: &Path,
) -> Vec<PathBuf> {
    let ext = crate::TypeScriptExtractor::new();
    exspec_core::observe::resolve_barrel_exports(&ext, barrel_path, symbols, scan_root)
}

fn production_stem(path: &str) -> Option<&str> {
    Path::new(path).file_stem()?.to_str()
}

fn test_stem(path: &str) -> Option<&str> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    stem.strip_suffix(".spec")
        .or_else(|| stem.strip_suffix(".test"))
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

    // === Route Extraction Tests ===

    // RT1: basic NestJS controller routes
    #[test]
    fn basic_controller_routes() {
        // Given: nestjs_controller.ts with @Controller('users') + @Get, @Post, @Delete
        let source = fixture("nestjs_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_controller.ts");

        // Then: GET /users, POST /users, DELETE /users/:id
        assert_eq!(routes.len(), 3, "expected 3 routes, got {routes:?}");
        let methods: Vec<&str> = routes.iter().map(|r| r.http_method.as_str()).collect();
        assert!(methods.contains(&"GET"), "expected GET in {methods:?}");
        assert!(methods.contains(&"POST"), "expected POST in {methods:?}");
        assert!(
            methods.contains(&"DELETE"),
            "expected DELETE in {methods:?}"
        );

        let get_route = routes.iter().find(|r| r.http_method == "GET").unwrap();
        assert_eq!(get_route.path, "/users");

        let delete_route = routes.iter().find(|r| r.http_method == "DELETE").unwrap();
        assert_eq!(delete_route.path, "/users/:id");
    }

    // RT2: route path combination
    #[test]
    fn route_path_combination() {
        // Given: nestjs_routes_advanced.ts with @Controller('api/v1/users') + @Get('active')
        let source = fixture("nestjs_routes_advanced.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_routes_advanced.ts");

        // Then: GET /api/v1/users/active
        let active = routes
            .iter()
            .find(|r| r.handler_name == "findActive")
            .unwrap();
        assert_eq!(active.http_method, "GET");
        assert_eq!(active.path, "/api/v1/users/active");
    }

    // RT3: controller with no path argument
    #[test]
    fn controller_no_path() {
        // Given: nestjs_empty_controller.ts with @Controller() + @Get('health')
        let source = fixture("nestjs_empty_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_empty_controller.ts");

        // Then: GET /health
        assert_eq!(routes.len(), 1, "expected 1 route, got {routes:?}");
        assert_eq!(routes[0].http_method, "GET");
        assert_eq!(routes[0].path, "/health");
    }

    // RT4: method without route decorator is not extracted
    #[test]
    fn method_without_route_decorator() {
        // Given: nestjs_empty_controller.ts with helperMethod() (no decorator)
        let source = fixture("nestjs_empty_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_empty_controller.ts");

        // Then: helperMethod is not in routes
        let helper = routes.iter().find(|r| r.handler_name == "helperMethod");
        assert!(helper.is_none(), "helperMethod should not be a route");
    }

    // RT5: all HTTP methods
    #[test]
    fn all_http_methods() {
        // Given: nestjs_routes_advanced.ts with Get, Post, Put, Patch, Delete, Head, Options
        let source = fixture("nestjs_routes_advanced.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_routes_advanced.ts");

        // Then: 9 routes (Get appears 3 times)
        assert_eq!(routes.len(), 9, "expected 9 routes, got {routes:?}");
        let methods: Vec<&str> = routes.iter().map(|r| r.http_method.as_str()).collect();
        assert!(methods.contains(&"GET"));
        assert!(methods.contains(&"POST"));
        assert!(methods.contains(&"PUT"));
        assert!(methods.contains(&"PATCH"));
        assert!(methods.contains(&"DELETE"));
        assert!(methods.contains(&"HEAD"));
        assert!(methods.contains(&"OPTIONS"));
    }

    // RT6: UseGuards decorator extraction
    #[test]
    fn use_guards_decorator() {
        // Given: nestjs_guards_pipes.ts with @UseGuards(AuthGuard)
        let source = fixture("nestjs_guards_pipes.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract decorators
        let decorators = extractor.extract_decorators(&source, "nestjs_guards_pipes.ts");

        // Then: UseGuards with AuthGuard
        let guards: Vec<&DecoratorInfo> = decorators
            .iter()
            .filter(|d| d.name == "UseGuards")
            .collect();
        assert!(!guards.is_empty(), "expected UseGuards decorators");
        let auth_guard = guards
            .iter()
            .find(|d| d.arguments.contains(&"AuthGuard".to_string()));
        assert!(auth_guard.is_some(), "expected AuthGuard argument");
    }

    // RT7: only gap-relevant decorators (UseGuards, not Delete)
    #[test]
    fn multiple_decorators_on_method() {
        // Given: nestjs_controller.ts with @Delete(':id') @UseGuards(AuthGuard) on remove()
        let source = fixture("nestjs_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract decorators
        let decorators = extractor.extract_decorators(&source, "nestjs_controller.ts");

        // Then: UseGuards only (Delete is a route decorator, not gap-relevant)
        let names: Vec<&str> = decorators.iter().map(|d| d.name.as_str()).collect();
        assert!(
            names.contains(&"UseGuards"),
            "expected UseGuards in {names:?}"
        );
        assert!(
            !names.contains(&"Delete"),
            "Delete should not be in decorators"
        );
    }

    // RT8: class-validator decorators on DTO
    #[test]
    fn class_validator_on_dto() {
        // Given: nestjs_dto_validation.ts with @IsEmail, @IsNotEmpty on fields
        let source = fixture("nestjs_dto_validation.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract decorators
        let decorators = extractor.extract_decorators(&source, "nestjs_dto_validation.ts");

        // Then: IsEmail and IsNotEmpty extracted
        let names: Vec<&str> = decorators.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"IsEmail"), "expected IsEmail in {names:?}");
        assert!(
            names.contains(&"IsNotEmpty"),
            "expected IsNotEmpty in {names:?}"
        );
    }

    // RT9: UsePipes decorator
    #[test]
    fn use_pipes_decorator() {
        // Given: nestjs_guards_pipes.ts with @UsePipes(ValidationPipe)
        let source = fixture("nestjs_guards_pipes.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract decorators
        let decorators = extractor.extract_decorators(&source, "nestjs_guards_pipes.ts");

        // Then: UsePipes with ValidationPipe
        let pipes: Vec<&DecoratorInfo> =
            decorators.iter().filter(|d| d.name == "UsePipes").collect();
        assert!(!pipes.is_empty(), "expected UsePipes decorators");
        assert!(pipes[0].arguments.contains(&"ValidationPipe".to_string()));
    }

    // RT10: empty source returns empty for routes and decorators
    #[test]
    fn empty_source_returns_empty_routes_and_decorators() {
        // Given: empty source
        let extractor = TypeScriptExtractor::new();

        // When: extract routes and decorators
        let routes = extractor.extract_routes("", "empty.ts");
        let decorators = extractor.extract_decorators("", "empty.ts");

        // Then: both empty
        assert!(routes.is_empty());
        assert!(decorators.is_empty());
    }

    // RT11: non-NestJS class returns no routes
    #[test]
    fn non_nestjs_class_ignored() {
        // Given: class_methods.ts (plain class, no @Controller)
        let source = fixture("class_methods.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "class_methods.ts");

        // Then: empty
        assert!(routes.is_empty(), "expected no routes from plain class");
    }

    // RT12: handler_name and class_name correct
    #[test]
    fn route_handler_and_class_name() {
        // Given: nestjs_controller.ts
        let source = fixture("nestjs_controller.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_controller.ts");

        // Then: handler names and class name correct
        let handlers: Vec<&str> = routes.iter().map(|r| r.handler_name.as_str()).collect();
        assert!(handlers.contains(&"findAll"));
        assert!(handlers.contains(&"create"));
        assert!(handlers.contains(&"remove"));
        for route in &routes {
            assert_eq!(route.class_name, "UsersController");
        }
    }

    // RT13: class-level UseGuards decorator is extracted
    #[test]
    fn class_level_use_guards() {
        // Given: nestjs_guards_pipes.ts with @UseGuards(JwtAuthGuard) at class level
        let source = fixture("nestjs_guards_pipes.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract decorators
        let decorators = extractor.extract_decorators(&source, "nestjs_guards_pipes.ts");

        // Then: JwtAuthGuard class-level decorator is extracted
        let class_guards: Vec<&DecoratorInfo> = decorators
            .iter()
            .filter(|d| {
                d.name == "UseGuards"
                    && d.target_name == "ProtectedController"
                    && d.class_name == "ProtectedController"
            })
            .collect();
        assert!(
            !class_guards.is_empty(),
            "expected class-level UseGuards, got {decorators:?}"
        );
        assert!(class_guards[0]
            .arguments
            .contains(&"JwtAuthGuard".to_string()));
    }

    // RT14: non-literal controller path produces <dynamic>
    #[test]
    fn dynamic_controller_path() {
        // Given: nestjs_dynamic_routes.ts with @Controller(BASE_PATH)
        let source = fixture("nestjs_dynamic_routes.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract routes
        let routes = extractor.extract_routes(&source, "nestjs_dynamic_routes.ts");

        // Then: path contains <dynamic>
        assert_eq!(routes.len(), 1);
        assert!(
            routes[0].path.contains("<dynamic>"),
            "expected <dynamic> in path, got {:?}",
            routes[0].path
        );
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

    #[test]
    fn basic_spec_mapping() {
        // Given: a production file and its matching .spec test file in the same directory
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/users.service.ts".to_string()];
        let test_files = vec!["src/users.service.spec.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: the files are matched with FileNameConvention
        assert_eq!(
            mappings,
            vec![FileMapping {
                production_file: "src/users.service.ts".to_string(),
                test_files: vec!["src/users.service.spec.ts".to_string()],
                strategy: MappingStrategy::FileNameConvention,
            }]
        );
    }

    #[test]
    fn test_suffix_mapping() {
        // Given: a production file and its matching .test file
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/utils.ts".to_string()];
        let test_files = vec!["src/utils.test.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: the files are matched
        assert_eq!(
            mappings[0].test_files,
            vec!["src/utils.test.ts".to_string()]
        );
    }

    #[test]
    fn multiple_test_files() {
        // Given: one production file and both .spec and .test files
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/app.ts".to_string()];
        let test_files = vec!["src/app.spec.ts".to_string(), "src/app.test.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: both test files are matched
        assert_eq!(
            mappings[0].test_files,
            vec!["src/app.spec.ts".to_string(), "src/app.test.ts".to_string()]
        );
    }

    #[test]
    fn nestjs_controller() {
        // Given: a nested controller file and its matching spec file
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/users/users.controller.ts".to_string()];
        let test_files = vec!["src/users/users.controller.spec.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: the nested files are matched
        assert_eq!(
            mappings[0].test_files,
            vec!["src/users/users.controller.spec.ts".to_string()]
        );
    }

    #[test]
    fn no_matching_test() {
        // Given: a production file and an unrelated test file
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/orphan.ts".to_string()];
        let test_files = vec!["src/other.spec.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: the production file is still included with no tests
        assert_eq!(mappings[0].test_files, Vec::<String>::new());
    }

    #[test]
    fn different_directory_no_match() {
        // Given: matching stems in different directories
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/users.ts".to_string()];
        let test_files = vec!["test/users.spec.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: no match is created because Layer 1 is same-directory only
        assert_eq!(mappings[0].test_files, Vec::<String>::new());
    }

    #[test]
    fn empty_input() {
        // Given: no production files and no test files
        let extractor = TypeScriptExtractor::new();

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&[], &[]);

        // Then: an empty vector is returned
        assert!(mappings.is_empty());
    }

    #[test]
    fn tsx_files() {
        // Given: a TSX production file and its matching test file
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/App.tsx".to_string()];
        let test_files = vec!["src/App.test.tsx".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: the TSX files are matched
        assert_eq!(mappings[0].test_files, vec!["src/App.test.tsx".to_string()]);
    }

    #[test]
    fn unmatched_test_ignored() {
        // Given: one matching test file and one orphan test file
        let extractor = TypeScriptExtractor::new();
        let production_files = vec!["src/a.ts".to_string()];
        let test_files = vec!["src/a.spec.ts".to_string(), "src/b.spec.ts".to_string()];

        // When: map_test_files is called
        let mappings = extractor.map_test_files(&production_files, &test_files);

        // Then: only the matching test file is included
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].test_files, vec!["src/a.spec.ts".to_string()]);
    }

    #[test]
    fn stem_extraction() {
        // Given: production and test file paths with ts and tsx extensions
        // When: production_stem and test_stem are called
        // Then: the normalized stems are extracted correctly
        assert_eq!(
            production_stem("src/users.service.ts"),
            Some("users.service")
        );
        assert_eq!(production_stem("src/App.tsx"), Some("App"));
        assert_eq!(
            test_stem("src/users.service.spec.ts"),
            Some("users.service")
        );
        assert_eq!(test_stem("src/utils.test.ts"), Some("utils"));
        assert_eq!(test_stem("src/App.test.tsx"), Some("App"));
        assert_eq!(test_stem("src/invalid.ts"), None);
    }

    // === extract_imports Tests (IM1-IM7) ===

    // IM1: named import の symbol と specifier が抽出される
    #[test]
    fn im1_named_import_symbol_and_specifier() {
        // Given: import_named.ts with `import { UsersController } from './users.controller'`
        let source = fixture("import_named.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_named.ts");

        // Then: symbol: "UsersController", specifier: "./users.controller"
        let found = imports.iter().find(|i| i.symbol_name == "UsersController");
        assert!(
            found.is_some(),
            "expected UsersController in imports: {imports:?}"
        );
        assert_eq!(
            found.unwrap().module_specifier,
            "./users.controller",
            "wrong specifier"
        );
    }

    // IM2: 複数 named import (`{ A, B }`) が 2件返る (同specifier、異なるsymbol)
    #[test]
    fn im2_multiple_named_imports() {
        // Given: import_mixed.ts with `import { A, B } from './module'`
        let source = fixture("import_mixed.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_mixed.ts");

        // Then: A と B が両方返る (同じ ./module specifier)
        let from_module: Vec<&ImportMapping> = imports
            .iter()
            .filter(|i| i.module_specifier == "./module")
            .collect();
        let symbols: Vec<&str> = from_module.iter().map(|i| i.symbol_name.as_str()).collect();
        assert!(symbols.contains(&"A"), "expected A in symbols: {symbols:?}");
        assert!(symbols.contains(&"B"), "expected B in symbols: {symbols:?}");
        // at least 2 from ./module (IM2: { A, B } + IM3: { A as B } both in import_mixed.ts)
        assert!(
            from_module.len() >= 2,
            "expected at least 2 imports from ./module, got {from_module:?}"
        );
    }

    // IM3: エイリアス import (`{ A as B }`) で元の名前 "A" が返る
    #[test]
    fn im3_alias_import_original_name() {
        // Given: import_mixed.ts with `import { A as B } from './module'`
        let source = fixture("import_mixed.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_mixed.ts");

        // Then: symbol_name は "A" (エイリアス "B" ではなく元の名前)
        // import_mixed.ts has: { A, B } and { A as B } — both should yield A
        let a_count = imports.iter().filter(|i| i.symbol_name == "A").count();
        assert!(
            a_count >= 1,
            "expected at least one import with symbol_name 'A', got: {imports:?}"
        );
    }

    // IM4: default import の symbol と specifier が抽出される
    #[test]
    fn im4_default_import() {
        // Given: import_default.ts with `import UsersController from './users.controller'`
        let source = fixture("import_default.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_default.ts");

        // Then: symbol: "UsersController", specifier: "./users.controller"
        assert_eq!(imports.len(), 1, "expected 1 import, got {imports:?}");
        assert_eq!(imports[0].symbol_name, "UsersController");
        assert_eq!(imports[0].module_specifier, "./users.controller");
    }

    // IM5: npm パッケージ import (`@nestjs/testing`) が除外される (空Vec)
    #[test]
    fn im5_npm_package_excluded() {
        // Given: source with only `import { Test } from '@nestjs/testing'`
        let source = "import { Test } from '@nestjs/testing';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(source, "test.ts");

        // Then: 空Vec (npm パッケージは除外)
        assert!(imports.is_empty(), "expected empty vec, got {imports:?}");
    }

    // IM6: 相対 `../` パスが含まれる
    #[test]
    fn im6_relative_parent_path() {
        // Given: import_named.ts with `import { S } from '../services/s.service'`
        let source = fixture("import_named.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_named.ts");

        // Then: specifier: "../services/s.service"
        let found = imports
            .iter()
            .find(|i| i.module_specifier == "../services/s.service");
        assert!(
            found.is_some(),
            "expected ../services/s.service in imports: {imports:?}"
        );
        assert_eq!(found.unwrap().symbol_name, "S");
    }

    // IM7: 空ソースで空Vec が返る
    #[test]
    fn im7_empty_source_returns_empty() {
        // Given: empty source
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports("", "empty.ts");

        // Then: 空Vec
        assert!(imports.is_empty());
    }

    // IM8: namespace import (`import * as X from './module'`) が抽出される
    #[test]
    fn im8_namespace_import() {
        // Given: import_namespace.ts with `import * as UsersController from './users.controller'`
        let source = fixture("import_namespace.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_namespace.ts");

        // Then: UsersController が symbol_name として抽出される
        let found = imports.iter().find(|i| i.symbol_name == "UsersController");
        assert!(
            found.is_some(),
            "expected UsersController in imports: {imports:?}"
        );
        assert_eq!(found.unwrap().module_specifier, "./users.controller");

        // Then: helpers も相対パスなので抽出される
        let helpers = imports.iter().find(|i| i.symbol_name == "helpers");
        assert!(
            helpers.is_some(),
            "expected helpers in imports: {imports:?}"
        );
        assert_eq!(helpers.unwrap().module_specifier, "../utils/helpers");

        // Then: npm パッケージ (express) は除外される
        let express = imports.iter().find(|i| i.symbol_name == "express");
        assert!(
            express.is_none(),
            "npm package should be excluded: {imports:?}"
        );
    }

    // IM9: type-only import (`import type { X }`) が除外され、通常importは残る
    #[test]
    fn im9_type_only_import_excluded() {
        // Given: import_type_only.ts with type-only and normal imports
        let source = fixture("import_type_only.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_type_only.ts");

        // Then: `import type { UserService }` は除外される
        let user_service = imports.iter().find(|i| i.symbol_name == "UserService");
        assert!(
            user_service.is_none(),
            "type-only import should be excluded: {imports:?}"
        );

        // Then: `import { type CreateUserDto }` (inline type modifier) も除外される
        let create_dto = imports.iter().find(|i| i.symbol_name == "CreateUserDto");
        assert!(
            create_dto.is_none(),
            "inline type modifier import should be excluded: {imports:?}"
        );

        // Then: `import { UsersController }` は残る
        let controller = imports.iter().find(|i| i.symbol_name == "UsersController");
        assert!(
            controller.is_some(),
            "normal import should remain: {imports:?}"
        );
        assert_eq!(controller.unwrap().module_specifier, "./users.controller");
    }

    // === resolve_import_path Tests (RP1-RP5) ===

    // RP1: 拡張子なし specifier + 実在 `.ts` ファイル → Some(canonical path)
    #[test]
    fn rp1_resolve_ts_without_extension() {
        use std::io::Write as IoWrite;
        use tempfile::TempDir;

        // Given: scan_root/src/users.controller.ts が実在する
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let target = src_dir.join("users.controller.ts");
        std::fs::File::create(&target).unwrap();

        let from_file = src_dir.join("users.controller.spec.ts");

        // When: resolve_import_path("./users.controller", ...)
        let result = resolve_import_path("./users.controller", &from_file, dir.path());

        // Then: Some(canonical path)
        assert!(
            result.is_some(),
            "expected Some for existing .ts file, got None"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("users.controller.ts"),
            "expected path ending with users.controller.ts, got {resolved}"
        );
    }

    // RP2: 拡張子付き specifier (`.ts`) + 実在ファイル → Some(canonical path)
    #[test]
    fn rp2_resolve_ts_with_extension() {
        use tempfile::TempDir;

        // Given: scan_root/src/users.controller.ts が実在する
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let target = src_dir.join("users.controller.ts");
        std::fs::File::create(&target).unwrap();

        let from_file = src_dir.join("users.controller.spec.ts");

        // When: resolve_import_path("./users.controller.ts", ...) (拡張子付き)
        let result = resolve_import_path("./users.controller.ts", &from_file, dir.path());

        // Then: Some(canonical path)
        assert!(
            result.is_some(),
            "expected Some for existing file with explicit .ts extension"
        );
    }

    // RP3: 存在しないファイル → None
    #[test]
    fn rp3_nonexistent_file_returns_none() {
        use tempfile::TempDir;

        // Given: scan_root が空
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let from_file = src_dir.join("some.spec.ts");

        // When: resolve_import_path("./nonexistent", ...)
        let result = resolve_import_path("./nonexistent", &from_file, dir.path());

        // Then: None
        assert!(result.is_none(), "expected None for nonexistent file");
    }

    // RP4: scan_root 外のパス (`../../outside`) → None
    #[test]
    fn rp4_outside_scan_root_returns_none() {
        use tempfile::TempDir;

        // Given: scan_root/src/ から ../../outside を参照 (scan_root 外)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let from_file = src_dir.join("some.spec.ts");

        // When: resolve_import_path("../../outside", ...)
        let result = resolve_import_path("../../outside", &from_file, dir.path());

        // Then: None (path traversal ガード)
        assert!(result.is_none(), "expected None for path outside scan_root");
    }

    // RP5: 拡張子なし specifier + 実在 `.tsx` ファイル → Some(canonical path)
    #[test]
    fn rp5_resolve_tsx_without_extension() {
        use tempfile::TempDir;

        // Given: scan_root/src/App.tsx が実在する
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let target = src_dir.join("App.tsx");
        std::fs::File::create(&target).unwrap();

        let from_file = src_dir.join("App.test.tsx");

        // When: resolve_import_path("./App", ...)
        let result = resolve_import_path("./App", &from_file, dir.path());

        // Then: Some(canonical path ending in App.tsx)
        assert!(
            result.is_some(),
            "expected Some for existing .tsx file, got None"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("App.tsx"),
            "expected path ending with App.tsx, got {resolved}"
        );
    }

    // === map_test_files_with_imports Tests (MT1-MT4) ===

    // MT1: Layer 1 マッチ + Layer 2 マッチが共存 → 両方マッピングされる
    #[test]
    fn mt1_layer1_and_layer2_both_matched() {
        use tempfile::TempDir;

        // Given:
        //   production: src/users.controller.ts
        //   test (Layer 1 match): src/users.controller.spec.ts (same dir)
        //   test (Layer 2 match): test/users.controller.spec.ts (imports users.controller)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("users.controller.ts");
        std::fs::File::create(&prod_path).unwrap();

        let layer1_test = src_dir.join("users.controller.spec.ts");
        let layer1_source = r#"// Layer 1 spec
describe('UsersController', () => {});
"#;

        let layer2_test = test_dir.join("users.controller.spec.ts");
        let layer2_source = format!(
            "import {{ UsersController }} from '../src/users.controller';\ndescribe('cross', () => {{}});\n"
        );

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            layer1_test.to_string_lossy().into_owned(),
            layer1_source.to_string(),
        );
        test_sources.insert(
            layer2_test.to_string_lossy().into_owned(),
            layer2_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: 両方のテストがマッピングされる
        assert_eq!(mappings.len(), 1, "expected 1 FileMapping");
        let mapping = &mappings[0];
        assert!(
            mapping
                .test_files
                .contains(&layer1_test.to_string_lossy().into_owned()),
            "expected Layer 1 test in mapping, got {:?}",
            mapping.test_files
        );
        assert!(
            mapping
                .test_files
                .contains(&layer2_test.to_string_lossy().into_owned()),
            "expected Layer 2 test in mapping, got {:?}",
            mapping.test_files
        );
    }

    // MT2: クロスディレクトリ import → ImportTracing でマッチ
    #[test]
    fn mt2_cross_directory_import_tracing() {
        use tempfile::TempDir;

        // Given:
        //   production: src/services/user.service.ts
        //   test: test/user.service.spec.ts (imports user.service from cross-directory)
        //   Layer 1 は別ディレクトリのためマッチしない
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src").join("services");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("user.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("user.service.spec.ts");
        let test_source = format!(
            "import {{ UserService }} from '../src/services/user.service';\ndescribe('cross', () => {{}});\n"
        );

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(test_path.to_string_lossy().into_owned(), test_source);

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: ImportTracing でマッチ
        assert_eq!(mappings.len(), 1);
        let mapping = &mappings[0];
        assert!(
            mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected test in mapping via ImportTracing, got {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "expected ImportTracing strategy"
        );
    }

    // MT3: npm import のみ → 未マッチ
    #[test]
    fn mt3_npm_only_import_not_matched() {
        use tempfile::TempDir;

        // Given:
        //   production: src/users.controller.ts
        //   test: test/something.spec.ts (imports only from npm)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("users.controller.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("something.spec.ts");
        let test_source =
            "import { Test } from '@nestjs/testing';\ndescribe('npm', () => {});\n".to_string();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(test_path.to_string_lossy().into_owned(), test_source);

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: 未マッチ (test_files は空)
        assert_eq!(mappings.len(), 1);
        assert!(
            mappings[0].test_files.is_empty(),
            "expected no test files for npm-only import, got {:?}",
            mappings[0].test_files
        );
    }

    // MT4: 1テストが複数 production を import → 両方にマッピング
    #[test]
    fn mt4_one_test_imports_multiple_productions() {
        use tempfile::TempDir;

        // Given:
        //   production A: src/a.service.ts
        //   production B: src/b.service.ts
        //   test: test/ab.spec.ts (imports both A and B)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_a = src_dir.join("a.service.ts");
        let prod_b = src_dir.join("b.service.ts");
        std::fs::File::create(&prod_a).unwrap();
        std::fs::File::create(&prod_b).unwrap();

        let test_path = test_dir.join("ab.spec.ts");
        let test_source = format!(
            "import {{ A }} from '../src/a.service';\nimport {{ B }} from '../src/b.service';\ndescribe('ab', () => {{}});\n"
        );

        let production_files = vec![
            prod_a.to_string_lossy().into_owned(),
            prod_b.to_string_lossy().into_owned(),
        ];
        let mut test_sources = HashMap::new();
        test_sources.insert(test_path.to_string_lossy().into_owned(), test_source);

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: A と B 両方に test がマッピングされる
        assert_eq!(mappings.len(), 2, "expected 2 FileMappings (A and B)");
        for mapping in &mappings {
            assert!(
                mapping
                    .test_files
                    .contains(&test_path.to_string_lossy().into_owned()),
                "expected ab.spec.ts mapped to {}, got {:?}",
                mapping.production_file,
                mapping.test_files
            );
        }
    }

    // HELPER-01: constants.ts is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_constants_ts() {
        assert!(is_non_sut_helper("src/constants.ts", false));
    }

    // HELPER-02: index.ts is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_index_ts() {
        assert!(is_non_sut_helper("src/index.ts", false));
    }

    // HELPER-03: extension variants (.js/.tsx/.jsx) are also detected
    #[test]
    fn is_non_sut_helper_extension_variants() {
        assert!(is_non_sut_helper("src/constants.js", false));
        assert!(is_non_sut_helper("src/constants.tsx", false));
        assert!(is_non_sut_helper("src/constants.jsx", false));
        assert!(is_non_sut_helper("src/index.js", false));
        assert!(is_non_sut_helper("src/index.tsx", false));
        assert!(is_non_sut_helper("src/index.jsx", false));
    }

    // HELPER-04: similar but distinct filenames are NOT helpers
    #[test]
    fn is_non_sut_helper_rejects_non_helpers() {
        assert!(!is_non_sut_helper("src/my-constants.ts", false));
        assert!(!is_non_sut_helper("src/service.ts", false));
        assert!(!is_non_sut_helper("src/app.constants.ts", false));
        assert!(!is_non_sut_helper("src/constants-v2.ts", false));
    }

    // HELPER-05: directory named constants/app.ts is NOT a helper
    #[test]
    fn is_non_sut_helper_rejects_directory_name() {
        assert!(!is_non_sut_helper("constants/app.ts", false));
        assert!(!is_non_sut_helper("index/service.ts", false));
    }

    // HELPER-06: *.enum.ts is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_enum_ts() {
        // Given: a file with .enum.ts suffix
        let path = "src/enums/request-method.enum.ts";
        // When: is_non_sut_helper() is called
        // Then: returns true
        assert!(is_non_sut_helper(path, false));
    }

    // HELPER-07: *.interface.ts is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_interface_ts() {
        // Given: a file with .interface.ts suffix
        let path = "src/interfaces/middleware-configuration.interface.ts";
        // When: is_non_sut_helper() is called
        // Then: returns true
        assert!(is_non_sut_helper(path, false));
    }

    // HELPER-08: *.exception.ts is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_exception_ts() {
        // Given: a file with .exception.ts suffix
        let path = "src/errors/unknown-module.exception.ts";
        // When: is_non_sut_helper() is called
        // Then: returns true
        assert!(is_non_sut_helper(path, false));
    }

    // HELPER-09: file inside a test path is detected as non-SUT helper
    #[test]
    fn is_non_sut_helper_test_path() {
        // Given: a file located under a /test/ directory
        let path = "packages/core/test/utils/string.cleaner.ts";
        // When: is_non_sut_helper() is called
        // Then: returns true
        assert!(is_non_sut_helper(path, false));
        // __tests__ variant
        assert!(is_non_sut_helper(
            "packages/core/__tests__/utils/helper.ts",
            false
        ));
        // segment-based: "contest" should NOT match
        assert!(!is_non_sut_helper(
            "/home/user/projects/contest/src/service.ts",
            false
        ));
        assert!(!is_non_sut_helper("src/latest/foo.ts", false));
    }

    // HELPER-10: suffix-like but plain filename (not a suffix) is rejected
    #[test]
    fn is_non_sut_helper_rejects_plain_filename() {
        // Given: files whose name is exactly enum.ts / interface.ts / exception.ts
        // (the type keyword is the entire filename, not a suffix)
        // When: is_non_sut_helper() is called
        // Then: returns false (these may be real SUT files)
        assert!(!is_non_sut_helper("src/enum.ts", false));
        assert!(!is_non_sut_helper("src/interface.ts", false));
        assert!(!is_non_sut_helper("src/exception.ts", false));
    }

    // HELPER-11: extension variants (.js/.tsx/.jsx) with enum/interface suffix are detected
    #[test]
    fn is_non_sut_helper_enum_interface_extension_variants() {
        // Given: files with .enum or .interface suffix and non-.ts extension
        // When: is_non_sut_helper() is called
        // Then: returns true
        assert!(is_non_sut_helper("src/foo.enum.js", false));
        assert!(is_non_sut_helper("src/bar.interface.tsx", false));
    }

    // === is_type_definition_file unit tests (TD-01 ~ TD-05) ===

    // TD-01: *.enum.ts is a type definition file
    #[test]
    fn is_type_definition_file_enum() {
        assert!(is_type_definition_file("src/foo.enum.ts"));
    }

    // TD-02: *.interface.ts is a type definition file
    #[test]
    fn is_type_definition_file_interface() {
        assert!(is_type_definition_file("src/bar.interface.ts"));
    }

    // TD-03: *.exception.ts is a type definition file
    #[test]
    fn is_type_definition_file_exception() {
        assert!(is_type_definition_file("src/baz.exception.ts"));
    }

    // TD-04: regular service file is NOT a type definition file
    #[test]
    fn is_type_definition_file_service() {
        assert!(!is_type_definition_file("src/service.ts"));
    }

    // TD-05: constants.ts is NOT a type definition file (suffix check only, not exact-match)
    #[test]
    fn is_type_definition_file_constants() {
        // constants.ts has no .enum/.interface/.exception suffix
        assert!(!is_type_definition_file("src/constants.ts"));
    }

    // === is_non_sut_helper (production-aware) unit tests (PA-01 ~ PA-03) ===

    // PA-01: enum file with known_production=true bypasses suffix filter
    #[test]
    fn is_non_sut_helper_production_enum_bypassed() {
        // Given: an enum file known to be in production_files
        // When: is_non_sut_helper with is_known_production=true
        // Then: returns false (not filtered)
        assert!(!is_non_sut_helper("src/foo.enum.ts", true));
    }

    // PA-02: enum file with known_production=false is still filtered
    #[test]
    fn is_non_sut_helper_unknown_enum_filtered() {
        // Given: an enum file NOT in production_files
        // When: is_non_sut_helper with is_known_production=false
        // Then: returns true (filtered as before)
        assert!(is_non_sut_helper("src/foo.enum.ts", false));
    }

    // PA-03: constants.ts is filtered regardless of known_production
    #[test]
    fn is_non_sut_helper_constants_always_filtered() {
        // Given: constants.ts (exact-match filter, not suffix)
        // When: is_non_sut_helper with is_known_production=true
        // Then: returns true (exact-match is independent of production status)
        assert!(is_non_sut_helper("src/constants.ts", true));
    }

    // === Barrel Import Resolution Tests (BARREL-01 ~ BARREL-09) ===

    // BARREL-01: resolve_import_path がディレクトリの index.ts にフォールバックする
    #[test]
    fn barrel_01_resolve_directory_to_index_ts() {
        use tempfile::TempDir;

        // Given: scan_root/decorators/index.ts が存在
        let dir = TempDir::new().unwrap();
        let decorators_dir = dir.path().join("decorators");
        std::fs::create_dir_all(&decorators_dir).unwrap();
        std::fs::File::create(decorators_dir.join("index.ts")).unwrap();

        // from_file は scan_root/src/some.spec.ts (../../decorators → decorators/)
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let from_file = src_dir.join("some.spec.ts");

        // When: resolve_import_path("../decorators", from_file, scan_root)
        let result = resolve_import_path("../decorators", &from_file, dir.path());

        // Then: decorators/index.ts のパスを返す
        assert!(
            result.is_some(),
            "expected Some for directory with index.ts, got None"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("decorators/index.ts"),
            "expected path ending with decorators/index.ts, got {resolved}"
        );
    }

    // BARREL-02: extract_barrel_re_exports が named re-export をキャプチャする
    #[test]
    fn barrel_02_re_export_named_capture() {
        // Given: `export { Foo } from './foo'`
        let source = "export { Foo } from './foo';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_barrel_re_exports
        let re_exports = extractor.extract_barrel_re_exports(source, "index.ts");

        // Then: symbols=["Foo"], from="./foo", wildcard=false
        assert_eq!(
            re_exports.len(),
            1,
            "expected 1 re-export, got {re_exports:?}"
        );
        let re = &re_exports[0];
        assert_eq!(re.symbols, vec!["Foo".to_string()]);
        assert_eq!(re.from_specifier, "./foo");
        assert!(!re.wildcard);
    }

    // BARREL-03: extract_barrel_re_exports が wildcard re-export をキャプチャする
    #[test]
    fn barrel_03_re_export_wildcard_capture() {
        // Given: `export * from './foo'`
        let source = "export * from './foo';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_barrel_re_exports
        let re_exports = extractor.extract_barrel_re_exports(source, "index.ts");

        // Then: wildcard=true, from="./foo"
        assert_eq!(
            re_exports.len(),
            1,
            "expected 1 re-export, got {re_exports:?}"
        );
        let re = &re_exports[0];
        assert!(re.wildcard, "expected wildcard=true");
        assert_eq!(re.from_specifier, "./foo");
    }

    // BARREL-04: resolve_barrel_exports が 1ホップのバレルを解決する
    #[test]
    fn barrel_04_resolve_barrel_exports_one_hop() {
        use tempfile::TempDir;

        // Given:
        //   index.ts: export { Foo } from './foo'
        //   foo.ts: (実在)
        let dir = TempDir::new().unwrap();
        let index_path = dir.path().join("index.ts");
        std::fs::write(&index_path, "export { Foo } from './foo';").unwrap();
        let foo_path = dir.path().join("foo.ts");
        std::fs::File::create(&foo_path).unwrap();

        // When: resolve_barrel_exports(index_path, ["Foo"], scan_root)
        let result = resolve_barrel_exports(&index_path, &["Foo".to_string()], dir.path());

        // Then: [foo.ts] を返す
        assert_eq!(result.len(), 1, "expected 1 resolved file, got {result:?}");
        assert!(
            result[0].ends_with("foo.ts"),
            "expected foo.ts, got {:?}",
            result[0]
        );
    }

    // BARREL-05: resolve_barrel_exports が 2ホップのバレルを解決する
    #[test]
    fn barrel_05_resolve_barrel_exports_two_hops() {
        use tempfile::TempDir;

        // Given:
        //   index.ts: export * from './core'
        //   core/index.ts: export { Foo } from './foo'
        //   core/foo.ts: (実在)
        let dir = TempDir::new().unwrap();
        let index_path = dir.path().join("index.ts");
        std::fs::write(&index_path, "export * from './core';").unwrap();

        let core_dir = dir.path().join("core");
        std::fs::create_dir_all(&core_dir).unwrap();
        std::fs::write(core_dir.join("index.ts"), "export { Foo } from './foo';").unwrap();
        let foo_path = core_dir.join("foo.ts");
        std::fs::File::create(&foo_path).unwrap();

        // When: resolve_barrel_exports(index_path, ["Foo"], scan_root)
        let result = resolve_barrel_exports(&index_path, &["Foo".to_string()], dir.path());

        // Then: core/foo.ts を返す
        assert_eq!(result.len(), 1, "expected 1 resolved file, got {result:?}");
        assert!(
            result[0].ends_with("foo.ts"),
            "expected foo.ts, got {:?}",
            result[0]
        );
    }

    // BARREL-06: 循環バレルで無限ループしない
    #[test]
    fn barrel_06_circular_barrel_no_infinite_loop() {
        use tempfile::TempDir;

        // Given:
        //   a/index.ts: export * from '../b'
        //   b/index.ts: export * from '../a'
        let dir = TempDir::new().unwrap();
        let a_dir = dir.path().join("a");
        let b_dir = dir.path().join("b");
        std::fs::create_dir_all(&a_dir).unwrap();
        std::fs::create_dir_all(&b_dir).unwrap();
        std::fs::write(a_dir.join("index.ts"), "export * from '../b';").unwrap();
        std::fs::write(b_dir.join("index.ts"), "export * from '../a';").unwrap();

        let a_index = a_dir.join("index.ts");

        // When: resolve_barrel_exports — must NOT panic or hang
        let result = resolve_barrel_exports(&a_index, &["Foo".to_string()], dir.path());

        // Then: 空結果を返し、パニックしない
        assert!(
            result.is_empty(),
            "expected empty result for circular barrel, got {result:?}"
        );
    }

    // BARREL-07: Layer 2 で barrel 経由の import が production file にマッチする
    #[test]
    fn barrel_07_layer2_barrel_import_matches_production() {
        use tempfile::TempDir;

        // Given:
        //   production: src/foo.service.ts
        //   barrel: src/decorators/index.ts — export { Foo } from './foo.service'
        //           ただし src/decorators/foo.service.ts として re-export 先を指す
        //   test: test/foo.spec.ts — import { Foo } from '../src/decorators'
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let decorators_dir = src_dir.join("decorators");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&decorators_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        // Production file
        let prod_path = src_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        // Barrel: decorators/index.ts re-exports from ../foo.service
        std::fs::write(
            decorators_dir.join("index.ts"),
            "export { Foo } from '../foo.service';",
        )
        .unwrap();

        // Test imports from barrel directory
        let test_path = test_dir.join("foo.spec.ts");
        std::fs::write(
            &test_path,
            "import { Foo } from '../src/decorators';\ndescribe('foo', () => {});",
        )
        .unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports (barrel resolution enabled)
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: foo.service.ts に foo.spec.ts がマッピングされる
        assert_eq!(mappings.len(), 1, "expected 1 FileMapping");
        assert!(
            mappings[0]
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected foo.spec.ts mapped via barrel, got {:?}",
            mappings[0].test_files
        );
    }

    // BARREL-08: is_non_sut_helper フィルタが barrel 解決後のファイルに適用される
    #[test]
    fn barrel_08_non_sut_filter_applied_after_barrel_resolution() {
        use tempfile::TempDir;

        // Given:
        //   barrel: index.ts → export { SOME_CONST } from './constants'
        //   resolved: constants.ts (is_non_sut_helper → true)
        //   test imports from barrel
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        // Production file (real SUT)
        let prod_path = src_dir.join("user.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        // Barrel index: re-exports from constants
        std::fs::write(
            src_dir.join("index.ts"),
            "export { SOME_CONST } from './constants';",
        )
        .unwrap();
        // constants.ts (non-SUT helper)
        std::fs::File::create(src_dir.join("constants.ts")).unwrap();

        // Test imports from barrel (which resolves to constants.ts)
        let test_path = test_dir.join("barrel_const.spec.ts");
        std::fs::write(
            &test_path,
            "import { SOME_CONST } from '../src';\ndescribe('const', () => {});",
        )
        .unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: user.service.ts にはマッピングされない (constants.ts はフィルタ済み)
        assert_eq!(
            mappings.len(),
            1,
            "expected 1 FileMapping for user.service.ts"
        );
        assert!(
            mappings[0].test_files.is_empty(),
            "constants.ts should be filtered out, but got {:?}",
            mappings[0].test_files
        );
    }

    // BARREL-09: extract_imports が symbol 名を保持する (ImportMapping::symbols フィールド)
    #[test]
    fn barrel_09_extract_imports_retains_symbols() {
        // Given: `import { Foo, Bar } from './module'`
        let source = "import { Foo, Bar } from './module';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(source, "test.ts");

        // Then: Foo と Bar の両方が symbols として存在する
        // ImportMapping は symbol_name を 1件ずつ返すが、
        // 同一 module_specifier からの import は symbols Vec に集約される
        let from_module: Vec<&ImportMapping> = imports
            .iter()
            .filter(|i| i.module_specifier == "./module")
            .collect();
        let names: Vec<&str> = from_module.iter().map(|i| i.symbol_name.as_str()).collect();
        assert!(names.contains(&"Foo"), "expected Foo in symbols: {names:?}");
        assert!(names.contains(&"Bar"), "expected Bar in symbols: {names:?}");

        // BARREL-09 の本質: ImportMapping に symbols フィールドが存在し、
        // 同じ specifier からの import が集約されること
        // (現在の ImportMapping は symbol_name: String のみ → symbols: Vec<String> への移行が必要)
        let grouped = imports
            .iter()
            .filter(|i| i.module_specifier == "./module")
            .fold(Vec::<String>::new(), |mut acc, i| {
                acc.push(i.symbol_name.clone());
                acc
            });
        // symbols フィールドが実装されたら、1つの ImportMapping に ["Foo", "Bar"] が入る想定
        // 現時点では 2件の ImportMapping として返されることを確認
        assert_eq!(
            grouped.len(),
            2,
            "expected 2 symbols from ./module, got {grouped:?}"
        );

        // Verify symbols field aggregation: each ImportMapping from ./module
        // should have both Foo and Bar in its symbols Vec
        let first_import = imports
            .iter()
            .find(|i| i.module_specifier == "./module")
            .expect("expected at least one import from ./module");
        let symbols = &first_import.symbols;
        assert!(
            symbols.contains(&"Foo".to_string()),
            "symbols should contain Foo, got {symbols:?}"
        );
        assert!(
            symbols.contains(&"Bar".to_string()),
            "symbols should contain Bar, got {symbols:?}"
        );
        assert_eq!(
            symbols.len(),
            2,
            "expected exactly 2 symbols, got {symbols:?}"
        );
    }

    // BARREL-10: wildcard-only barrel で symbol フィルタが効く
    // NestJS パターン: index.ts → export * from './core' → core/index.ts → export * from './foo'
    // テストが { Foo } のみ import → foo.ts のみマッチ、bar.ts はマッチしない
    #[test]
    fn barrel_10_wildcard_barrel_symbol_filter() {
        use tempfile::TempDir;

        // Given:
        //   index.ts: export * from './core'
        //   core/index.ts: export * from './foo' + export * from './bar'
        //   core/foo.ts: export function Foo() {}
        //   core/bar.ts: export function Bar() {}
        let dir = TempDir::new().unwrap();
        let core_dir = dir.path().join("core");
        std::fs::create_dir_all(&core_dir).unwrap();

        std::fs::write(dir.path().join("index.ts"), "export * from './core';").unwrap();
        std::fs::write(
            core_dir.join("index.ts"),
            "export * from './foo';\nexport * from './bar';",
        )
        .unwrap();
        std::fs::write(core_dir.join("foo.ts"), "export function Foo() {}").unwrap();
        std::fs::write(core_dir.join("bar.ts"), "export function Bar() {}").unwrap();

        // When: resolve with symbols=["Foo"]
        let result = resolve_barrel_exports(
            &dir.path().join("index.ts"),
            &["Foo".to_string()],
            dir.path(),
        );

        // Then: foo.ts のみ返す (bar.ts は Foo を export していないのでマッチしない)
        assert_eq!(result.len(), 1, "expected 1 resolved file, got {result:?}");
        assert!(
            result[0].ends_with("foo.ts"),
            "expected foo.ts, got {:?}",
            result[0]
        );
    }

    // BARREL-11: wildcard barrel + symbols empty → 全ファイルを返す (保守的)
    #[test]
    fn barrel_11_wildcard_barrel_empty_symbols_match_all() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let core_dir = dir.path().join("core");
        std::fs::create_dir_all(&core_dir).unwrap();

        std::fs::write(dir.path().join("index.ts"), "export * from './core';").unwrap();
        std::fs::write(
            core_dir.join("index.ts"),
            "export * from './foo';\nexport * from './bar';",
        )
        .unwrap();
        std::fs::write(core_dir.join("foo.ts"), "export function Foo() {}").unwrap();
        std::fs::write(core_dir.join("bar.ts"), "export function Bar() {}").unwrap();

        // When: resolve with empty symbols (match all)
        let result = resolve_barrel_exports(&dir.path().join("index.ts"), &[], dir.path());

        // Then: both files returned
        assert_eq!(result.len(), 2, "expected 2 resolved files, got {result:?}");
    }

    // === Boundary Specification Tests (B1-B6) ===
    // These tests document CURRENT behavior at failure boundaries.
    // All assertions reflect known limitations, not desired future behavior.

    // TC-01: Boundary B1 — namespace re-export is NOT captured by extract_barrel_re_exports
    #[test]
    fn boundary_b1_ns_reexport_not_captured() {
        // Given: barrel index.ts with `export * as Ns from './validators'`
        let source = "export * as Validators from './validators';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_barrel_re_exports
        let re_exports = extractor.extract_barrel_re_exports(source, "index.ts");

        // Then: namespace re-export is NOT captured (empty vec)
        // Note: re_export.scm only handles `export { X } from` and `export * from`,
        //       not `export * as Ns from` (namespace export is a different AST node)
        assert!(
            re_exports.is_empty(),
            "expected empty re_exports for namespace export, got {:?}",
            re_exports
        );
    }

    // TC-02: Boundary B1 — namespace re-export causes test-to-code mapping miss (FN)
    #[test]
    fn boundary_b1_ns_reexport_mapping_miss() {
        use tempfile::TempDir;

        // Given:
        //   validators/foo.service.ts (production)
        //   index.ts: `export * as Validators from './validators'`
        //   validators/index.ts: `export { FooService } from './foo.service'`
        //   test/foo.spec.ts: `import { Validators } from '../index'`
        let dir = TempDir::new().unwrap();
        let validators_dir = dir.path().join("validators");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&validators_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        // Production file
        let prod_path = validators_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        // Root barrel: namespace re-export (B1 boundary)
        std::fs::write(
            dir.path().join("index.ts"),
            "export * as Validators from './validators';",
        )
        .unwrap();

        // validators/index.ts: named re-export
        std::fs::write(
            validators_dir.join("index.ts"),
            "export { FooService } from './foo.service';",
        )
        .unwrap();

        // Test imports via namespace re-export
        let test_path = test_dir.join("foo.spec.ts");
        std::fs::write(
            &test_path,
            "import { Validators } from '../index';\ndescribe('FooService', () => {});",
        )
        .unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: foo.service.ts has NO test_files (FN — namespace re-export not resolved)
        // Layer 1 (filename convention) produces no match either, so the mapping has no test_files.
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected no test_files mapped (FN: namespace re-export not resolved), got {:?}",
            all_test_files
        );
    }

    // TC-03: Boundary B2 — non-relative import is skipped by extract_imports
    #[test]
    fn boundary_b2_non_relative_import_skipped() {
        // Given: source with `import { Injectable } from '@nestjs/common'`
        let source = "import { Injectable } from '@nestjs/common';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(source, "app.service.ts");

        // Then: imports is empty (non-relative paths are excluded)
        assert!(
            imports.is_empty(),
            "expected empty imports for non-relative path, got {:?}",
            imports
        );
    }

    // TC-04: Boundary B2 — cross-package barrel import is unresolvable (FN)
    #[test]
    fn boundary_b2_cross_pkg_barrel_unresolvable() {
        use tempfile::TempDir;

        // Given:
        //   packages/core/ (scan_root)
        //   packages/core/src/foo.service.ts (production)
        //   packages/common/src/foo.ts (production, in different package)
        //   packages/core/test/foo.spec.ts: `import { Foo } from '@org/common'` (non-relative)
        let dir = TempDir::new().unwrap();
        let core_src = dir.path().join("packages").join("core").join("src");
        let core_test = dir.path().join("packages").join("core").join("test");
        let common_src = dir.path().join("packages").join("common").join("src");
        std::fs::create_dir_all(&core_src).unwrap();
        std::fs::create_dir_all(&core_test).unwrap();
        std::fs::create_dir_all(&common_src).unwrap();

        let prod_path = core_src.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let common_path = common_src.join("foo.ts");
        std::fs::File::create(&common_path).unwrap();

        let test_path = core_test.join("foo.spec.ts");
        std::fs::write(
            &test_path,
            "import { Foo } from '@org/common';\ndescribe('Foo', () => {});",
        )
        .unwrap();

        let scan_root = dir.path().join("packages").join("core");
        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports(scan_root=packages/core/)
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, &scan_root);

        // Then: packages/common/src/foo.ts has NO test_files (cross-package import not resolved)
        // Since `@org/common` is non-relative, extract_imports will skip it entirely.
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected no test_files mapped (FN: cross-package import not resolved), got {:?}",
            all_test_files
        );
    }

    // TC-05: Boundary B3 — tsconfig path alias is treated same as non-relative import
    #[test]
    fn boundary_b3_tsconfig_alias_not_resolved() {
        // Given: source with `import { FooService } from '@app/services/foo.service'`
        let source = "import { FooService } from '@app/services/foo.service';";
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(source, "app.module.ts");

        // Then: imports is empty (@app/ is non-relative, same code path as TC-03)
        // Note: tsconfig path aliases are treated identically to package imports.
        // Same root cause as B2 but different user expectation.
        assert!(
            imports.is_empty(),
            "expected empty imports for tsconfig alias, got {:?}",
            imports
        );
    }

    // TC-06: B4 — .enum.ts in production_files is NOT filtered (production-aware bypass)
    #[test]
    fn boundary_b4_enum_primary_target_filtered() {
        use tempfile::TempDir;

        // Given:
        //   src/route-paramtypes.enum.ts (production)
        //   test/route.spec.ts: `import { RouteParamtypes } from '../src/route-paramtypes.enum'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("route-paramtypes.enum.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("route.spec.ts");
        std::fs::write(
            &test_path,
            "import { RouteParamtypes } from '../src/route-paramtypes.enum';\ndescribe('Route', () => {});",
        )
        .unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: route-paramtypes.enum.ts IS mapped to route.spec.ts (production-aware bypass)
        let enum_mapping = mappings
            .iter()
            .find(|m| m.production_file.ends_with("route-paramtypes.enum.ts"));
        assert!(
            enum_mapping.is_some(),
            "expected mapping for route-paramtypes.enum.ts"
        );
        let enum_mapping = enum_mapping.unwrap();
        assert!(
            !enum_mapping.test_files.is_empty(),
            "expected test_files for route-paramtypes.enum.ts (production file), got empty"
        );
    }

    // TC-07: B4 — .interface.ts in production_files is NOT filtered (production-aware bypass)
    #[test]
    fn boundary_b4_interface_primary_target_filtered() {
        use tempfile::TempDir;

        // Given:
        //   src/user.interface.ts (production)
        //   test/user.spec.ts: `import { User } from '../src/user.interface'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("user.interface.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("user.spec.ts");
        std::fs::write(
            &test_path,
            "import { User } from '../src/user.interface';\ndescribe('User', () => {});",
        )
        .unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: user.interface.ts IS mapped to user.spec.ts (production-aware bypass)
        let iface_mapping = mappings
            .iter()
            .find(|m| m.production_file.ends_with("user.interface.ts"));
        assert!(
            iface_mapping.is_some(),
            "expected mapping for user.interface.ts"
        );
        let iface_mapping = iface_mapping.unwrap();
        assert!(
            !iface_mapping.test_files.is_empty(),
            "expected test_files for user.interface.ts (production file), got empty"
        );
    }

    // TC-08: Boundary B5 — dynamic import() is not captured by extract_imports
    #[test]
    fn boundary_b5_dynamic_import_not_extracted() {
        // Given: fixture("import_dynamic.ts") containing `const m = await import('./user.service')`
        let source = fixture("import_dynamic.ts");
        let extractor = TypeScriptExtractor::new();

        // When: extract_imports
        let imports = extractor.extract_imports(&source, "import_dynamic.ts");

        // Then: imports is empty (dynamic import() not captured by import_mapping.scm)
        assert!(
            imports.is_empty(),
            "expected empty imports for dynamic import(), got {:?}",
            imports
        );
    }

    // === tsconfig alias integration tests (OB-01 to OB-06) ===

    // OB-01: tsconfig alias basic — @app/foo.service -> src/foo.service.ts
    #[test]
    fn test_observe_tsconfig_alias_basic() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig.json: @app/* -> src/*
        //   src/foo.service.ts (production)
        //   test/foo.service.spec.ts: `import { FooService } from '@app/foo.service'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let tsconfig = dir.path().join("tsconfig.json");
        std::fs::write(
            &tsconfig,
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let prod_path = src_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("foo.service.spec.ts");
        let test_source =
            "import { FooService } from '@app/foo.service';\ndescribe('FooService', () => {});\n";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: foo.service.ts is mapped to foo.service.spec.ts via alias resolution
        let mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("foo.service.ts"))
            .expect("expected mapping for foo.service.ts");
        assert!(
            mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected foo.service.spec.ts in mapping via alias, got {:?}",
            mapping.test_files
        );
    }

    // OB-02: no tsconfig -> alias import produces no mapping
    #[test]
    fn test_observe_no_tsconfig_alias_ignored() {
        use tempfile::TempDir;

        // Given:
        //   NO tsconfig.json
        //   src/foo.service.ts (production)
        //   test/foo.service.spec.ts: `import { FooService } from '@app/foo.service'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_path = src_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("foo.service.spec.ts");
        let test_source =
            "import { FooService } from '@app/foo.service';\ndescribe('FooService', () => {});\n";

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports (no tsconfig.json present)
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: no test_files mapped (alias import skipped without tsconfig)
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected no test_files when tsconfig absent, got {:?}",
            all_test_files
        );
    }

    // OB-03: tsconfig alias + barrel -> resolves via barrel
    #[test]
    fn test_observe_tsconfig_alias_barrel() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig: @app/* -> src/*
        //   src/bar.service.ts (production)
        //   src/services/index.ts (barrel): `export { BarService } from '../bar.service'`
        //   test/bar.service.spec.ts: `import { BarService } from '@app/services'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let services_dir = src_dir.join("services");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&services_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let prod_path = src_dir.join("bar.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        std::fs::write(
            services_dir.join("index.ts"),
            "export { BarService } from '../bar.service';\n",
        )
        .unwrap();

        let test_path = test_dir.join("bar.service.spec.ts");
        let test_source =
            "import { BarService } from '@app/services';\ndescribe('BarService', () => {});\n";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: bar.service.ts is mapped via alias + barrel resolution
        let mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("bar.service.ts"))
            .expect("expected mapping for bar.service.ts");
        assert!(
            mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected bar.service.spec.ts mapped via alias+barrel, got {:?}",
            mapping.test_files
        );
    }

    // OB-04: mixed relative + alias imports -> both resolved
    #[test]
    fn test_observe_tsconfig_alias_mixed() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig: @app/* -> src/*
        //   src/foo.service.ts, src/bar.service.ts (productions)
        //   test/mixed.spec.ts:
        //     `import { FooService } from '@app/foo.service'`   (alias)
        //     `import { BarService } from '../src/bar.service'` (relative)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let foo_path = src_dir.join("foo.service.ts");
        let bar_path = src_dir.join("bar.service.ts");
        std::fs::File::create(&foo_path).unwrap();
        std::fs::File::create(&bar_path).unwrap();

        let test_path = test_dir.join("mixed.spec.ts");
        let test_source = "\
import { FooService } from '@app/foo.service';
import { BarService } from '../src/bar.service';
describe('Mixed', () => {});
";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![
            foo_path.to_string_lossy().into_owned(),
            bar_path.to_string_lossy().into_owned(),
        ];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: both foo.service.ts and bar.service.ts are mapped
        let foo_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("foo.service.ts"))
            .expect("expected mapping for foo.service.ts");
        assert!(
            foo_mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected mixed.spec.ts in foo mapping, got {:?}",
            foo_mapping.test_files
        );
        let bar_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("bar.service.ts"))
            .expect("expected mapping for bar.service.ts");
        assert!(
            bar_mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected mixed.spec.ts in bar mapping, got {:?}",
            bar_mapping.test_files
        );
    }

    // OB-05: tsconfig alias + is_non_sut_helper filter -> constants.ts is excluded
    #[test]
    fn test_observe_tsconfig_alias_helper_filtered() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig: @app/* -> src/*
        //   src/constants.ts (production, but filtered by is_non_sut_helper)
        //   test/constants.spec.ts: `import { APP_NAME } from '@app/constants'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let prod_path = src_dir.join("constants.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("constants.spec.ts");
        let test_source =
            "import { APP_NAME } from '@app/constants';\ndescribe('Constants', () => {});\n";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: constants.ts is filtered by is_non_sut_helper → no test_files
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected constants.ts filtered by is_non_sut_helper, got {:?}",
            all_test_files
        );
    }

    // OB-06: alias to nonexistent file -> no mapping, no error
    #[test]
    fn test_observe_tsconfig_alias_nonexistent() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig: @app/* -> src/*
        //   src/foo.service.ts (production)
        //   test/nonexistent.spec.ts: `import { Missing } from '@app/nonexistent'`
        //   (src/nonexistent.ts does NOT exist)
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let prod_path = src_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("nonexistent.spec.ts");
        let test_source =
            "import { Missing } from '@app/nonexistent';\ndescribe('Nonexistent', () => {});\n";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports (should not panic)
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: no mapping (nonexistent.ts not in production_files), no panic
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected no mapping for alias to nonexistent file, got {:?}",
            all_test_files
        );
    }

    // B3-update: boundary_b3_tsconfig_alias_resolved
    // With tsconfig.json present, @app/* alias SHOULD be resolved (FN → TP)
    #[test]
    fn boundary_b3_tsconfig_alias_resolved() {
        use tempfile::TempDir;

        // Given:
        //   tsconfig.json: @app/* -> src/*
        //   src/foo.service.ts (production)
        //   test/foo.service.spec.ts: `import { FooService } from '@app/services/foo.service'`
        let dir = TempDir::new().unwrap();
        let src_dir = dir.path().join("src");
        let services_dir = src_dir.join("services");
        let test_dir = dir.path().join("test");
        std::fs::create_dir_all(&services_dir).unwrap();
        std::fs::create_dir_all(&test_dir).unwrap();

        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .unwrap();

        let prod_path = services_dir.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        let test_path = test_dir.join("foo.service.spec.ts");
        let test_source = "import { FooService } from '@app/services/foo.service';\ndescribe('FooService', () => {});\n";
        std::fs::write(&test_path, test_source).unwrap();

        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports WITH tsconfig present
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, dir.path());

        // Then: foo.service.ts IS mapped (B3 resolved — FN → TP)
        let mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("foo.service.ts"))
            .expect("expected FileMapping for foo.service.ts");
        assert!(
            mapping
                .test_files
                .contains(&test_path.to_string_lossy().into_owned()),
            "expected tsconfig alias to be resolved (B3 fix), got {:?}",
            mapping.test_files
        );
    }

    // TC-09: Boundary B6 — import target outside scan_root is not mapped
    #[test]
    fn boundary_b6_import_outside_scan_root() {
        use tempfile::TempDir;

        // Given:
        //   packages/core/ (scan_root)
        //   packages/core/src/foo.service.ts (production)
        //   packages/common/src/shared.ts (outside scan_root)
        //   packages/core/test/foo.spec.ts: `import { Shared } from '../../common/src/shared'`
        let dir = TempDir::new().unwrap();
        let core_src = dir.path().join("packages").join("core").join("src");
        let core_test = dir.path().join("packages").join("core").join("test");
        let common_src = dir.path().join("packages").join("common").join("src");
        std::fs::create_dir_all(&core_src).unwrap();
        std::fs::create_dir_all(&core_test).unwrap();
        std::fs::create_dir_all(&common_src).unwrap();

        let prod_path = core_src.join("foo.service.ts");
        std::fs::File::create(&prod_path).unwrap();

        // shared.ts is outside scan_root (packages/core/)
        let shared_path = common_src.join("shared.ts");
        std::fs::File::create(&shared_path).unwrap();

        let test_path = core_test.join("foo.spec.ts");
        std::fs::write(
            &test_path,
            "import { Shared } from '../../common/src/shared';\ndescribe('Foo', () => {});",
        )
        .unwrap();

        let scan_root = dir.path().join("packages").join("core");
        // Only production files within scan_root are registered
        let production_files = vec![prod_path.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_path.to_string_lossy().into_owned(),
            std::fs::read_to_string(&test_path).unwrap(),
        );

        let extractor = TypeScriptExtractor::new();

        // When: map_test_files_with_imports(scan_root=packages/core/)
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, &scan_root);

        // Then: shared.ts outside scan_root is not in production_files, so no mapping occurs.
        // `../../common/src/shared` resolves outside scan_root; it won't be in production_files
        // and won't match foo.service.ts by filename either.
        let all_test_files: Vec<&String> =
            mappings.iter().flat_map(|m| m.test_files.iter()).collect();
        assert!(
            all_test_files.is_empty(),
            "expected no test_files (import target outside scan_root), got {:?}",
            all_test_files
        );
    }
}
