use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use super::{cached_query, TypeScriptExtractor};

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const IMPORT_MAPPING_QUERY: &str = include_str!("../queries/import_mapping.scm");
static IMPORT_MAPPING_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

/// A production (non-test) function or method extracted from source code.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductionFunction {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub class_name: Option<String>,
    pub is_exported: bool,
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct FileMapping {
    pub production_file: String,
    pub test_files: Vec<String>,
    pub strategy: MappingStrategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MappingStrategy {
    FileNameConvention,
    ImportTracing,
}

/// An import statement extracted from a TypeScript source file.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportMapping {
    pub symbol_name: String,
    pub module_specifier: String,
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

impl TypeScriptExtractor {
    pub fn map_test_files(
        &self,
        production_files: &[String],
        test_files: &[String],
    ) -> Vec<FileMapping> {
        let mut tests_by_key: HashMap<(String, String), Vec<String>> = HashMap::new();

        for test_file in test_files {
            let Some(stem) = test_stem(test_file) else {
                continue;
            };
            let directory = Path::new(test_file)
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
                .unwrap_or_default();

            tests_by_key
                .entry((directory, stem.to_string()))
                .or_default()
                .push(test_file.clone());
        }

        production_files
            .iter()
            .map(|production_file| {
                let test_matches = production_stem(production_file)
                    .and_then(|stem| {
                        let directory = Path::new(production_file)
                            .parent()
                            .map(|parent| parent.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        tests_by_key.get(&(directory, stem.to_string())).cloned()
                    })
                    .unwrap_or_default();

                FileMapping {
                    production_file: production_file.clone(),
                    test_files: test_matches,
                    strategy: MappingStrategy::FileNameConvention,
                }
            })
            .collect()
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

/// Extract import statements from TypeScript source.
/// Returns only relative imports (starting with "." or ".."); npm packages are excluded.
impl TypeScriptExtractor {
    pub fn extract_imports(&self, source: &str, file_path: &str) -> Vec<ImportMapping> {
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
            let mut symbol = None;
            let mut specifier = None;
            let mut symbol_line = 0usize;
            for cap in m.captures {
                if cap.index == symbol_idx {
                    symbol = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                    symbol_line = cap.node.start_position().row + 1;
                } else if cap.index == specifier_idx {
                    specifier = Some(cap.node.utf8_text(source_bytes).unwrap_or(""));
                }
            }
            if let (Some(sym), Some(spec)) = (symbol, specifier) {
                // Filter: only relative paths (./ or ../)
                if spec.starts_with("./") || spec.starts_with("../") {
                    result.push(ImportMapping {
                        symbol_name: sym.to_string(),
                        module_specifier: spec.to_string(),
                        file: file_path.to_string(),
                        line: symbol_line,
                    });
                }
            }
        }
        result
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

        // Layer 2: import tracing for all test files (Layer 1 matched tests may
        // also import other production files not matched by filename convention)
        for (test_file, source) in test_sources {
            let imports = self.extract_imports(source, test_file);
            let from_file = Path::new(test_file);
            let mut matched_indices = std::collections::HashSet::new();
            for import in &imports {
                if let Some(resolved) =
                    resolve_import_path(&import.module_specifier, from_file, &canonical_root)
                {
                    if let Some(&idx) = canonical_to_idx.get(&resolved) {
                        matched_indices.insert(idx);
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
/// Returns None if the file does not exist or is outside scan_root.
pub fn resolve_import_path(
    module_specifier: &str,
    from_file: &Path,
    scan_root: &Path,
) -> Option<String> {
    // Canonicalize base_dir: use the parent directory of from_file.
    // If the parent directory exists (even if from_file itself doesn't), canonicalize it.
    // Otherwise fall back to the non-canonical parent for path arithmetic.
    let base_dir_raw = from_file.parent()?;
    let base_dir = base_dir_raw
        .canonicalize()
        .unwrap_or_else(|_| base_dir_raw.to_path_buf());
    let raw_path = base_dir.join(module_specifier);
    // If the specifier already has a known TS/JS extension, try it directly.
    // Otherwise, probe by appending each known extension. We must APPEND (not replace) because
    // dotted module names like "user.service" have "service" as their apparent extension but
    // are not actually extension-bearing: the real file is "user.service.ts".
    const TS_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx"];
    let has_known_ext = raw_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| TS_EXTENSIONS.contains(&e));
    let candidates = if has_known_ext {
        vec![raw_path.clone()]
    } else {
        // Append extension to preserve dotted names (e.g. user.service → user.service.ts)
        let base = raw_path.as_os_str().to_string_lossy();
        TS_EXTENSIONS
            .iter()
            .map(|ext| std::path::PathBuf::from(format!("{base}.{ext}")))
            .collect::<Vec<_>>()
    };

    let canonical_root = scan_root.canonicalize().ok()?;

    for candidate in candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.starts_with(&canonical_root) {
                return Some(canonical.to_string_lossy().into_owned());
            }
        }
    }
    None
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
}
