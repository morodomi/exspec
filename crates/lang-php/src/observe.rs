use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

use exspec_core::observe::{
    BarrelReExport, FileMapping, ImportMapping, MappingStrategy, ObserveExtractor,
    ProductionFunction,
};

use super::PhpExtractor;

// ---------------------------------------------------------------------------
// Route struct (Laravel route extraction)
// ---------------------------------------------------------------------------

/// A route extracted from a Laravel routes/*.php file.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub http_method: String,
    pub path: String,
    pub handler_name: String,
    pub class_name: String,
    pub file: String,
    pub line: usize,
}

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const IMPORT_MAPPING_QUERY: &str = include_str!("../queries/import_mapping.scm");
static IMPORT_MAPPING_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const EXTENDS_CLASS_QUERY: &str = include_str!("../queries/extends_class.scm");
static EXTENDS_CLASS_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

fn php_language() -> tree_sitter::Language {
    tree_sitter_php::LANGUAGE_PHP.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&php_language(), source).expect("invalid query"))
}

// ---------------------------------------------------------------------------
// Stem helpers
// ---------------------------------------------------------------------------

/// Extract stem from a PHP test file path.
/// `tests/UserTest.php` -> `Some("User")`   (Test suffix, PHPUnit)
/// `tests/user_test.php` -> `Some("user")`  (_test suffix, Pest)
/// `tests/Unit/OrderServiceTest.php` -> `Some("OrderService")`
/// `src/User.php` -> `None`
pub fn test_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    // Must end with .php
    let stem = file_name.strip_suffix(".php")?;

    // *Test.php (PHPUnit convention)
    if let Some(rest) = stem.strip_suffix("Test") {
        if !rest.is_empty() {
            return Some(rest);
        }
    }

    // *_test.php (Pest convention)
    if let Some(rest) = stem.strip_suffix("_test") {
        if !rest.is_empty() {
            return Some(rest);
        }
    }

    None
}

/// Extract stem from a PHP production file path.
/// `src/User.php` -> `Some("User")`
/// `src/Models/User.php` -> `Some("User")`
/// `tests/UserTest.php` -> `None`
pub fn production_stem(path: &str) -> Option<&str> {
    // Test files are not production files
    if test_stem(path).is_some() {
        return None;
    }

    let file_name = Path::new(path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".php")?;

    if stem.is_empty() {
        return None;
    }

    Some(stem)
}

/// Check if a file is a non-SUT helper (not subject under test).
pub fn is_non_sut_helper(file_path: &str, is_known_production: bool) -> bool {
    // If the file is already known to be a production file, it's not a helper.
    if is_known_production {
        return false;
    }

    let normalized = file_path.replace('\\', "/");
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // TestCase.php (base test class)
    if file_name == "TestCase.php" {
        return true;
    }

    // *Factory.php in tests/ (Laravel factory)
    if file_name.ends_with("Factory.php") {
        let in_tests = normalized.starts_with("tests/") || normalized.contains("/tests/");
        if in_tests {
            return true;
        }
    }

    // Abstract*.php in tests/
    if file_name.starts_with("Abstract") && file_name.ends_with(".php") {
        let in_tests = normalized.starts_with("tests/") || normalized.contains("/tests/");
        if in_tests {
            return true;
        }
    }

    // Trait*.php or *Trait.php in tests/ (test traits)
    let in_tests = normalized.starts_with("tests/") || normalized.contains("/tests/");
    if in_tests
        && file_name.ends_with(".php")
        && (file_name.starts_with("Trait") || file_name.ends_with("Trait.php"))
    {
        return true;
    }

    // Files in tests/Traits/ directory
    if normalized.contains("/tests/Traits/") || normalized.starts_with("tests/Traits/") {
        return true;
    }

    // Fixtures and Stubs directories under tests/ are test infrastructure, not SUT
    let lower = normalized.to_lowercase();
    if (lower.contains("/tests/fixtures/") || lower.starts_with("tests/fixtures/"))
        || (lower.contains("/tests/stubs/") || lower.starts_with("tests/stubs/"))
    {
        return true;
    }

    // Kernel.php
    if file_name == "Kernel.php" {
        return true;
    }

    // bootstrap.php or bootstrap/*.php
    if file_name == "bootstrap.php" {
        return true;
    }
    if normalized.starts_with("bootstrap/") || normalized.contains("/bootstrap/") {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// PSR-4 prefix resolution
// ---------------------------------------------------------------------------

/// Load PSR-4 namespace prefix -> directory mappings from composer.json.
/// Returns a map of namespace prefix (trailing `\` stripped) -> directory (trailing `/` stripped).
/// Returns an empty map if composer.json is absent or unparseable.
pub fn load_psr4_prefixes(scan_root: &Path) -> HashMap<String, String> {
    let composer_path = scan_root.join("composer.json");
    let content = match std::fs::read_to_string(&composer_path) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    let mut result = HashMap::new();

    // Parse both autoload and autoload-dev psr-4 sections
    for section in &["autoload", "autoload-dev"] {
        if let Some(psr4) = value
            .get(section)
            .and_then(|a| a.get("psr-4"))
            .and_then(|p| p.as_object())
        {
            for (ns, dir) in psr4 {
                // Strip trailing backslash from namespace prefix
                let ns_key = ns.trim_end_matches('\\').to_string();
                // Strip trailing slash from directory
                let dir_val = dir.as_str().unwrap_or("").trim_end_matches('/').to_string();
                if !ns_key.is_empty() {
                    result.insert(ns_key, dir_val);
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// External package detection
// ---------------------------------------------------------------------------

/// Known external PHP package namespace prefixes to skip during import resolution.
const EXTERNAL_NAMESPACES: &[&str] = &[
    "PHPUnit",
    "Illuminate",
    "Symfony",
    "Doctrine",
    "Mockery",
    "Carbon",
    "Pest",
    "Laravel",
    "Monolog",
    "Psr",
    "GuzzleHttp",
    "League",
    "Ramsey",
    "Spatie",
    "Nette",
    "Webmozart",
    "PhpParser",
    "SebastianBergmann",
];

fn is_external_namespace(namespace: &str, scan_root: Option<&Path>) -> bool {
    let first_segment = namespace.split('/').next().unwrap_or("");
    let is_known_external = EXTERNAL_NAMESPACES
        .iter()
        .any(|&ext| first_segment.eq_ignore_ascii_case(ext));

    if !is_known_external {
        return false;
    }

    // If scan_root is provided, check if the namespace source exists locally.
    // If it does, this is a framework self-test scenario — treat as internal.
    if let Some(root) = scan_root {
        for prefix in &["src", "app", "lib", ""] {
            let candidate = if prefix.is_empty() {
                root.join(first_segment)
            } else {
                root.join(prefix).join(first_segment)
            };
            if candidate.is_dir() {
                return false;
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Route extraction helpers (Laravel)
// ---------------------------------------------------------------------------

/// Known Laravel route HTTP methods.
const LARAVEL_HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "any"];

/// A prefix group: (prefix_value, group_body_start_byte, group_body_end_byte)
type PrefixGroup = (String, usize, usize);

/// Walk the AST and collect all `Route::prefix('...')` groups.
/// Returns a list of (prefix, start_byte, end_byte) for the closure/arrow body.
fn collect_prefix_groups(node: tree_sitter::Node, src: &[u8]) -> Vec<PrefixGroup> {
    let mut groups = Vec::new();
    collect_prefix_groups_recursive(node, src, &mut groups);
    groups
}

fn collect_prefix_groups_recursive(
    node: tree_sitter::Node,
    src: &[u8],
    groups: &mut Vec<PrefixGroup>,
) {
    // Check if this node is a `Route::prefix(...)->group(...)` call
    if node.kind() == "member_call_expression" {
        if let Some((prefix, body_start, body_end)) = try_extract_prefix_group(node, src) {
            groups.push((prefix, body_start, body_end));
        }
    }
    // Recurse into children
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_prefix_groups_recursive(child, src, groups);
    }
}

/// Try to extract prefix + group body from a `member_call_expression` node.
/// Matches: `Route::prefix('admin')->group(function () { ... })`
/// and also chained: `Route::middleware('auth')->group(...)` -> returns None (no prefix).
fn try_extract_prefix_group(node: tree_sitter::Node, src: &[u8]) -> Option<(String, usize, usize)> {
    // node is member_call_expression
    // structure: object->name(arguments)
    // object = scoped_call_expression (Route::prefix('admin')) OR another member_call_expression
    // name = "group"
    let name_node = node.child_by_field_name("name")?;
    let method_name = name_node.utf8_text(src).ok()?;
    if method_name != "group" {
        return None;
    }

    // Get the object (should be scoped_call_expression or member_call_expression chain)
    let object_node = node.child_by_field_name("object")?;

    // Extract prefix from the chain
    let prefix = extract_prefix_from_chain(object_node, src)?;

    // Get group's closure/arrow body byte range
    let args_node = node.child_by_field_name("arguments")?;
    let body_range = find_closure_body_range(args_node, src)?;

    Some((prefix, body_range.0, body_range.1))
}

/// Recursively extract the prefix string from a method chain.
/// `Route::prefix('admin')` -> Some("admin")
/// `Route::middleware('auth')` -> None (not a prefix call)
/// `Route::prefix('api')->middleware('auth')` -> Some("api")  (still has prefix)
fn extract_prefix_from_chain(node: tree_sitter::Node, src: &[u8]) -> Option<String> {
    match node.kind() {
        "scoped_call_expression" => {
            // Route::prefix('admin') or Route::middleware('auth')
            let method_node = node.child_by_field_name("name")?;
            let method = method_node.utf8_text(src).ok()?;
            if method == "prefix" {
                let args = node.child_by_field_name("arguments")?;
                extract_first_string_arg(args, src)
            } else {
                None
            }
        }
        "member_call_expression" => {
            // Chain like Route::prefix('api')->middleware('auth')
            // Walk back to find if there's a prefix call in the chain
            let object_node = node.child_by_field_name("object")?;
            extract_prefix_from_chain(object_node, src)
        }
        _ => None,
    }
}

/// Find the byte range of the closure/arrow function body inside arguments.
/// PHP uses `anonymous_function` and `arrow_function` node kinds.
fn find_closure_body_range(args_node: tree_sitter::Node, _src: &[u8]) -> Option<(usize, usize)> {
    // Walk through argument nodes to find the closure/arrow function
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        // Direct closure/arrow
        if let Some(range) = closure_node_range(child) {
            return Some(range);
        }
        // Wrapped in argument node
        if child.kind() == "argument" {
            let mut ac = child.walk();
            for grandchild in child.named_children(&mut ac) {
                if let Some(range) = closure_node_range(grandchild) {
                    return Some(range);
                }
            }
        }
    }
    None
}

fn closure_node_range(node: tree_sitter::Node) -> Option<(usize, usize)> {
    match node.kind() {
        "anonymous_function" | "arrow_function" | "closure_expression" => {
            Some((node.start_byte(), node.end_byte()))
        }
        _ => None,
    }
}

/// Extract the first string literal argument from an arguments node.
fn extract_first_string_arg(args_node: tree_sitter::Node, src: &[u8]) -> Option<String> {
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        if child.kind() == "encapsed_string" || child.kind() == "string" {
            let raw = child.utf8_text(src).ok()?;
            return Some(strip_php_string_quotes(raw));
        }
        // Also check argument nodes that wrap the string
        if child.kind() == "argument" {
            let mut child_cursor = child.walk();
            for grandchild in child.named_children(&mut child_cursor) {
                if grandchild.kind() == "encapsed_string" || grandchild.kind() == "string" {
                    let raw = grandchild.utf8_text(src).ok()?;
                    return Some(strip_php_string_quotes(raw));
                }
            }
        }
    }
    None
}

/// Strip PHP string quotes: `'admin'` -> `admin`, `"admin"` -> `admin`
fn strip_php_string_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Walk AST and collect route calls (Route::get/post/etc.) into routes vec.
fn collect_routes(
    node: tree_sitter::Node,
    src: &[u8],
    file_path: &str,
    prefix_groups: &[PrefixGroup],
    routes: &mut Vec<Route>,
) {
    // Check if this is a scoped_call_expression: Route::<method>(...)
    if node.kind() == "scoped_call_expression" {
        if let Some(route) = try_extract_route(node, src, file_path, prefix_groups) {
            routes.push(route);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_routes(child, src, file_path, prefix_groups, routes);
    }
}

/// Try to extract a Route from a scoped_call_expression node.
fn try_extract_route(
    node: tree_sitter::Node,
    src: &[u8],
    file_path: &str,
    prefix_groups: &[PrefixGroup],
) -> Option<Route> {
    // Check scope == "Route"
    let scope_node = node.child_by_field_name("scope")?;
    let scope = scope_node.utf8_text(src).ok()?;
    if scope != "Route" {
        return None;
    }

    // Check method is a known HTTP method
    let method_node = node.child_by_field_name("name")?;
    let method = method_node.utf8_text(src).ok()?;
    if !LARAVEL_HTTP_METHODS.contains(&method) {
        return None;
    }

    let http_method = method.to_uppercase();
    let line = node.start_position().row + 1;
    let byte_offset = node.start_byte();

    // Extract path and handler from arguments
    let args_node = node.child_by_field_name("arguments")?;
    let (path_raw, handler_name, class_name) = extract_route_args(args_node, src)?;

    // Resolve prefix from groups containing this byte offset
    let prefix = resolve_prefix(byte_offset, prefix_groups);

    // Combine prefix + path
    let path = if prefix.is_empty() {
        path_raw
    } else {
        // Remove leading slash from path_raw when combining with prefix
        let path_part = path_raw.trim_start_matches('/');
        format!("{prefix}/{path_part}")
    };

    Some(Route {
        http_method,
        path,
        handler_name,
        class_name,
        file: file_path.to_string(),
        line,
    })
}

/// Extract (path, handler_name, class_name) from a route call's arguments.
fn extract_route_args(
    args_node: tree_sitter::Node,
    src: &[u8],
) -> Option<(String, String, String)> {
    let args: Vec<tree_sitter::Node> = {
        let mut cursor = args_node.walk();
        args_node
            .named_children(&mut cursor)
            .filter(|n| n.kind() == "argument" || is_value_node(n.kind()))
            .collect()
    };

    // Normalize: unwrap argument nodes
    let values: Vec<tree_sitter::Node> = args
        .iter()
        .flat_map(|n| {
            if n.kind() == "argument" {
                let mut c = n.walk();
                n.named_children(&mut c).collect::<Vec<_>>()
            } else {
                vec![*n]
            }
        })
        .collect();

    if values.is_empty() {
        return None;
    }

    // First arg: path (string)
    let path_node = values.first()?;
    let path_raw = path_node.utf8_text(src).ok()?;
    let path = strip_php_string_quotes(path_raw);

    // Second arg: handler (array or closure/arrow)
    let handler_name;
    let class_name;

    if let Some(handler_node) = values.get(1) {
        match handler_node.kind() {
            "array_creation_expression" => {
                // [ControllerClass::class, 'method']
                let (cls, method) = extract_controller_array(*handler_node, src);
                class_name = cls;
                handler_name = method;
            }
            "closure_expression" | "arrow_function" | "anonymous_class" => {
                class_name = String::new();
                handler_name = String::new();
            }
            _ => {
                class_name = String::new();
                handler_name = String::new();
            }
        }
    } else {
        class_name = String::new();
        handler_name = String::new();
    }

    Some((path, handler_name, class_name))
}

fn is_value_node(kind: &str) -> bool {
    matches!(
        kind,
        "encapsed_string"
            | "string"
            | "array_creation_expression"
            | "closure_expression"
            | "arrow_function"
            | "anonymous_class"
            | "name"
    )
}

/// Extract (class_name, method_name) from `[ControllerClass::class, 'method']`.
fn extract_controller_array(array_node: tree_sitter::Node, src: &[u8]) -> (String, String) {
    let mut cursor = array_node.walk();
    let elements: Vec<tree_sitter::Node> = array_node
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "array_element_initializer")
        .collect();

    let mut class_name = String::new();
    let mut method_name = String::new();

    // First element: ControllerClass::class
    if let Some(elem0) = elements.first() {
        let mut ec = elem0.walk();
        for child in elem0.named_children(&mut ec) {
            if child.kind() == "class_constant_access_expression" {
                // Structure: scope::class
                if let Some(scope) = child.child_by_field_name("class") {
                    class_name = scope.utf8_text(src).unwrap_or("").to_string();
                    // Remove FQCN prefix: keep only the last segment
                    if let Some(last) = class_name.rsplit('\\').next() {
                        class_name = last.to_string();
                    }
                } else {
                    // Fallback: first named child
                    let mut cc = child.walk();
                    let first_child_text: Option<String> = child
                        .named_children(&mut cc)
                        .next()
                        .and_then(|n| n.utf8_text(src).ok())
                        .map(|s| s.to_string());
                    drop(cc);
                    if let Some(raw) = first_child_text {
                        if let Some(last) = raw.rsplit('\\').next() {
                            class_name = last.to_string();
                        }
                    }
                }
                break;
            }
        }
    }

    // Second element: 'method'
    if let Some(elem1) = elements.get(1) {
        let mut ec = elem1.walk();
        for child in elem1.named_children(&mut ec) {
            if child.kind() == "encapsed_string" || child.kind() == "string" {
                let raw = child.utf8_text(src).unwrap_or("");
                method_name = strip_php_string_quotes(raw);
                break;
            }
        }
    }

    (class_name, method_name)
}

/// Find the accumulated prefix for a given byte offset within prefix groups.
/// Groups are sorted by start byte; outermost first, innermost last.
fn resolve_prefix(byte_offset: usize, groups: &[PrefixGroup]) -> String {
    // Collect all groups that contain this byte offset
    let mut containing: Vec<&PrefixGroup> = groups
        .iter()
        .filter(|(_, start, end)| byte_offset > *start && byte_offset < *end)
        .collect();

    if containing.is_empty() {
        return String::new();
    }

    // Sort by start byte (outermost first)
    containing.sort_by_key(|(_, start, _)| *start);

    // Accumulate prefixes from outermost to innermost
    containing
        .iter()
        .map(|(p, _, _)| p.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

// ---------------------------------------------------------------------------
// ObserveExtractor impl
// ---------------------------------------------------------------------------

impl ObserveExtractor for PhpExtractor {
    fn extract_production_functions(
        &self,
        source: &str,
        file_path: &str,
    ) -> Vec<ProductionFunction> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&PRODUCTION_FUNCTION_QUERY_CACHE, PRODUCTION_FUNCTION_QUERY);

        let name_idx = query.capture_index_for_name("name");
        let class_name_idx = query.capture_index_for_name("class_name");
        let method_name_idx = query.capture_index_for_name("method_name");
        let function_idx = query.capture_index_for_name("function");
        let method_idx = query.capture_index_for_name("method");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        let mut result = Vec::new();

        while let Some(m) = matches.next() {
            let mut fn_name: Option<String> = None;
            let mut class_name: Option<String> = None;
            let mut line: usize = 1;
            let mut is_exported = true; // default: top-level functions are exported
            let mut method_node: Option<tree_sitter::Node> = None;

            for cap in m.captures {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                let node_line = cap.node.start_position().row + 1;

                if name_idx == Some(cap.index) {
                    fn_name = Some(text);
                    line = node_line;
                } else if class_name_idx == Some(cap.index) {
                    class_name = Some(text);
                } else if method_name_idx == Some(cap.index) {
                    fn_name = Some(text);
                    line = node_line;
                }

                // Capture method node for visibility check
                if method_idx == Some(cap.index) {
                    method_node = Some(cap.node);
                }

                // Top-level function: always exported
                if function_idx == Some(cap.index) {
                    is_exported = true;
                }
            }

            // Determine visibility from method node
            if let Some(method) = method_node {
                is_exported = has_public_visibility(method, source_bytes);
            }

            if let Some(name) = fn_name {
                result.push(ProductionFunction {
                    name,
                    file: file_path.to_string(),
                    line,
                    class_name,
                    is_exported,
                });
            }
        }

        result
    }

    fn extract_imports(&self, _source: &str, _file_path: &str) -> Vec<ImportMapping> {
        // PHP has no relative imports; Layer 2 uses PSR-4 namespace resolution
        Vec::new()
    }

    fn extract_all_import_specifiers(&self, source: &str) -> Vec<(String, Vec<String>)> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);

        let namespace_path_idx = query.capture_index_for_name("namespace_path");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut result_map: HashMap<String, Vec<String>> = HashMap::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if namespace_path_idx != Some(cap.index) {
                    continue;
                }
                let raw = cap.node.utf8_text(source_bytes).unwrap_or("");
                // Convert `App\Models\User` -> `App/Models/User`
                let fs_path = raw.replace('\\', "/");

                // Skip external packages (no scan_root — trait method, conservative filter)
                if is_external_namespace(&fs_path, None) {
                    continue;
                }

                // Split into module path and symbol
                // `App/Models/User` -> module=`App/Models`, symbol=`User`
                let parts: Vec<&str> = fs_path.splitn(2, '/').collect();
                if parts.len() < 2 {
                    // Single segment (no slash): use as both module and symbol
                    // e.g., `use User;` -> module="", symbol="User"
                    // Skip these edge cases
                    continue;
                }

                // Find the last '/' to split module from symbol
                if let Some(last_slash) = fs_path.rfind('/') {
                    let module_path = &fs_path[..last_slash];
                    let symbol = &fs_path[last_slash + 1..];
                    if !module_path.is_empty() && !symbol.is_empty() {
                        result_map
                            .entry(module_path.to_string())
                            .or_default()
                            .push(symbol.to_string());
                    }
                }
            }
        }

        result_map.into_iter().collect()
    }

    fn extract_barrel_re_exports(&self, _source: &str, _file_path: &str) -> Vec<BarrelReExport> {
        // PHP has no barrel export pattern
        Vec::new()
    }

    fn source_extensions(&self) -> &[&str] {
        &["php"]
    }

    fn index_file_names(&self) -> &[&str] {
        // PHP has no index files equivalent
        &[]
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
}

// ---------------------------------------------------------------------------
// Concrete methods (not in trait)
// ---------------------------------------------------------------------------

impl PhpExtractor {
    /// Extract Laravel routes from a routes/*.php file.
    pub fn extract_routes(&self, source: &str, file_path: &str) -> Vec<Route> {
        if source.is_empty() {
            return Vec::new();
        }

        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();

        // Collect prefix groups: (prefix_str, group_body_start_byte, group_body_end_byte)
        let prefix_groups = collect_prefix_groups(tree.root_node(), source_bytes);

        // Walk tree and collect routes
        let mut routes = Vec::new();
        collect_routes(
            tree.root_node(),
            source_bytes,
            file_path,
            &prefix_groups,
            &mut routes,
        );

        routes
    }

    /// Extract all import specifiers without external namespace filtering.
    /// Returns (module_path, [symbols]) pairs for all `use` statements.
    fn extract_raw_import_specifiers(source: &str) -> Vec<(String, Vec<String>)> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);

        let namespace_path_idx = query.capture_index_for_name("namespace_path");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut result_map: HashMap<String, Vec<String>> = HashMap::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                if namespace_path_idx != Some(cap.index) {
                    continue;
                }
                let raw = cap.node.utf8_text(source_bytes).unwrap_or("");
                let fs_path = raw.replace('\\', "/");

                let parts: Vec<&str> = fs_path.splitn(2, '/').collect();
                if parts.len() < 2 {
                    continue;
                }

                if let Some(last_slash) = fs_path.rfind('/') {
                    let module_path = &fs_path[..last_slash];
                    let symbol = &fs_path[last_slash + 1..];
                    if !module_path.is_empty() && !symbol.is_empty() {
                        result_map
                            .entry(module_path.to_string())
                            .or_default()
                            .push(symbol.to_string());
                    }
                }
            }
        }

        result_map.into_iter().collect()
    }

    /// Extract import specifiers from the parent class of a test file.
    /// Resolves the parent class to a file in the same directory, reads it,
    /// and returns its raw `use` statements (unfiltered).
    /// Only traces 1 level deep (direct parent only).
    pub fn extract_parent_class_imports(
        source: &str,
        test_dir: &str,
    ) -> Vec<(String, Vec<String>)> {
        // Step 1: parse source and find extends clause
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&EXTENDS_CLASS_QUERY_CACHE, EXTENDS_CLASS_QUERY);

        let parent_class_idx = query.capture_index_for_name("parent_class");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut parent_class_name: Option<String> = None;
        while let Some(m) = matches.next() {
            for cap in m.captures {
                if parent_class_idx == Some(cap.index) {
                    let name = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                    if !name.is_empty() {
                        parent_class_name = Some(name);
                        break;
                    }
                }
            }
            if parent_class_name.is_some() {
                break;
            }
        }

        let parent_name = match parent_class_name {
            Some(n) => n,
            None => return Vec::new(),
        };

        // Step 2: look for parent file in same directory
        let parent_file_name = format!("{parent_name}.php");
        let parent_path = Path::new(test_dir).join(&parent_file_name);

        // Read parent file
        let parent_source = match std::fs::read_to_string(&parent_path) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        // Step 3: extract raw import specifiers from parent
        Self::extract_raw_import_specifiers(&parent_source)
    }

    /// Layer 1 + Layer 2 (PSR-4): Map test files to production files.
    pub fn map_test_files_with_imports(
        &self,
        production_files: &[String],
        test_sources: &HashMap<String, String>,
        scan_root: &Path,
        l1_exclusive: bool,
    ) -> Vec<FileMapping> {
        let test_file_list: Vec<String> = test_sources.keys().cloned().collect();

        // Layer 1: filename convention (stem matching)
        let mut mappings =
            exspec_core::observe::map_test_files(self, production_files, &test_file_list);

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

        // Record Layer 1 matches per production file index
        let layer1_tests_per_prod: Vec<std::collections::HashSet<String>> = mappings
            .iter()
            .map(|m| m.test_files.iter().cloned().collect())
            .collect();

        // Collect set of test files matched by L1 for l1_exclusive mode
        let layer1_matched: std::collections::HashSet<String> = layer1_tests_per_prod
            .iter()
            .flat_map(|s| s.iter().cloned())
            .collect();

        // Load PSR-4 prefix mappings from composer.json (e.g., "MyApp" -> "custom_src")
        let psr4_prefixes = load_psr4_prefixes(scan_root);

        // Layer 2: PSR-4 convention import resolution
        // Use raw imports (unfiltered) and apply scan_root-aware external filtering
        for (test_file, source) in test_sources {
            if l1_exclusive && layer1_matched.contains(test_file) {
                continue;
            }
            let raw_specifiers = Self::extract_raw_import_specifiers(source);
            // Merge parent class imports (1-level, same directory only)
            let parent_dir = Path::new(test_file.as_str())
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parent_specifiers = Self::extract_parent_class_imports(source, &parent_dir);
            let combined: Vec<(String, Vec<String>)> = raw_specifiers
                .into_iter()
                .chain(parent_specifiers.into_iter())
                .collect();
            let specifiers: Vec<(String, Vec<String>)> = combined
                .into_iter()
                .filter(|(module_path, _)| !is_external_namespace(module_path, Some(scan_root)))
                .collect();
            let mut matched_indices = std::collections::HashSet::<usize>::new();

            for (module_path, _symbols) in &specifiers {
                // PSR-4 resolution:
                // `App/Models/User` -> try `src/Models/User.php`, `app/Models/User.php`, etc.
                //
                // Strategy: strip the first segment (PSR-4 prefix like "App")
                // and search for the remaining path under common directories.
                let parts: Vec<&str> = module_path.splitn(2, '/').collect();
                let first_segment = parts[0];
                let path_without_prefix = if parts.len() == 2 {
                    parts[1]
                } else {
                    module_path.as_str()
                };

                // Check if first segment matches a PSR-4 prefix from composer.json
                // e.g., "MyApp" -> "custom_src" means resolve under custom_src/
                let psr4_dir = psr4_prefixes.get(first_segment);

                // Derive the PHP file name from the last segment of module_path
                // e.g., `App/Models` -> last segment is `Models` -> file is `Models.php`
                // But module_path is actually the directory, not the file.
                // The symbol is in the symbols list, but we need to reconstruct the file path.
                // Actually, at this point module_path = `App/Models` and symbol could be `User`,
                // so the full file is `Models/User.php` (without prefix).

                // We need to get the symbols too
                for symbol in _symbols {
                    let file_name = format!("{symbol}.php");

                    // If composer.json defines a PSR-4 mapping for this namespace prefix,
                    // try the mapped directory first.
                    if let Some(psr4_base) = psr4_dir {
                        let candidate = canonical_root
                            .join(psr4_base)
                            .join(path_without_prefix)
                            .join(&file_name);
                        if let Ok(canonical_candidate) = candidate.canonicalize() {
                            let candidate_str = canonical_candidate.to_string_lossy().into_owned();
                            if let Some(&idx) = canonical_to_idx.get(&candidate_str) {
                                matched_indices.insert(idx);
                            }
                        }
                    }

                    // Try: <scan_root>/<common_prefix>/<path_without_prefix>/<symbol>.php
                    let common_prefixes = ["src", "app", "lib", ""];
                    for prefix in &common_prefixes {
                        let candidate = if prefix.is_empty() {
                            canonical_root.join(path_without_prefix).join(&file_name)
                        } else {
                            canonical_root
                                .join(prefix)
                                .join(path_without_prefix)
                                .join(&file_name)
                        };

                        if let Ok(canonical_candidate) = candidate.canonicalize() {
                            let candidate_str = canonical_candidate.to_string_lossy().into_owned();
                            if let Some(&idx) = canonical_to_idx.get(&candidate_str) {
                                matched_indices.insert(idx);
                            }
                        }
                    }

                    // Also try with the first segment kept (in case directory matches namespace 1:1)
                    // e.g., framework self-tests: `Illuminate/Http` -> `src/Illuminate/Http/Request.php`
                    for prefix in &common_prefixes {
                        let candidate = if prefix.is_empty() {
                            canonical_root.join(module_path).join(&file_name)
                        } else {
                            canonical_root
                                .join(prefix)
                                .join(module_path)
                                .join(&file_name)
                        };
                        if let Ok(canonical_candidate) = candidate.canonicalize() {
                            let candidate_str = canonical_candidate.to_string_lossy().into_owned();
                            if let Some(&idx) = canonical_to_idx.get(&candidate_str) {
                                matched_indices.insert(idx);
                            }
                        }
                    }
                }
            }

            for idx in matched_indices {
                if !mappings[idx].test_files.contains(test_file) {
                    mappings[idx].test_files.push(test_file.clone());
                }
            }
        }

        // Update strategy: if a production file had no Layer 1 matches but has Layer 2 matches,
        // set strategy to ImportTracing
        for (i, mapping) in mappings.iter_mut().enumerate() {
            let has_layer1 = !layer1_tests_per_prod[i].is_empty();
            if !has_layer1 && !mapping.test_files.is_empty() {
                mapping.strategy = MappingStrategy::ImportTracing;
            }
        }

        mappings
    }
}

// ---------------------------------------------------------------------------
// Visibility helper
// ---------------------------------------------------------------------------

/// Check if a PHP method_declaration node has `public` visibility.
/// Returns true for public, false for private/protected.
/// If no visibility_modifier child is found, defaults to true (public by convention in PHP).
fn has_public_visibility(node: tree_sitter::Node, source_bytes: &[u8]) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "visibility_modifier" {
                let text = child.utf8_text(source_bytes).unwrap_or("");
                return text == "public";
            }
        }
    }
    // No visibility modifier -> treat as public by default
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // PHP-STEM-01: tests/UserTest.php -> test_stem = Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_01_test_suffix() {
        // Given: a file named UserTest.php in tests/
        // When: test_stem is called
        // Then: returns Some("User")
        assert_eq!(test_stem("tests/UserTest.php"), Some("User"));
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-02: tests/user_test.php -> test_stem = Some("user")
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_02_pest_suffix() {
        // Given: a Pest-style file user_test.php
        // When: test_stem is called
        // Then: returns Some("user")
        assert_eq!(test_stem("tests/user_test.php"), Some("user"));
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-03: tests/Unit/OrderServiceTest.php -> test_stem = Some("OrderService")
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_03_nested() {
        // Given: a nested test file OrderServiceTest.php
        // When: test_stem is called
        // Then: returns Some("OrderService")
        assert_eq!(
            test_stem("tests/Unit/OrderServiceTest.php"),
            Some("OrderService")
        );
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-04: src/User.php -> test_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_04_non_test() {
        // Given: a production file src/User.php
        // When: test_stem is called
        // Then: returns None
        assert_eq!(test_stem("src/User.php"), None);
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-05: src/User.php -> production_stem = Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_05_prod_stem() {
        // Given: a production file src/User.php
        // When: production_stem is called
        // Then: returns Some("User")
        assert_eq!(production_stem("src/User.php"), Some("User"));
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-06: src/Models/User.php -> production_stem = Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_06_prod_nested() {
        // Given: a nested production file src/Models/User.php
        // When: production_stem is called
        // Then: returns Some("User")
        assert_eq!(production_stem("src/Models/User.php"), Some("User"));
    }

    // -----------------------------------------------------------------------
    // PHP-STEM-07: tests/UserTest.php -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn php_stem_07_test_not_prod() {
        // Given: a test file tests/UserTest.php
        // When: production_stem is called
        // Then: returns None (test files are not production files)
        assert_eq!(production_stem("tests/UserTest.php"), None);
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-01: tests/TestCase.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_01_test_case() {
        // Given: the base test class TestCase.php
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("tests/TestCase.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-02: tests/UserFactory.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_02_factory() {
        // Given: a Laravel factory file in tests/
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("tests/UserFactory.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-03: src/User.php -> is_non_sut_helper = false
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_03_production() {
        // Given: a regular production file
        // When: is_non_sut_helper is called
        // Then: returns false
        assert!(!is_non_sut_helper("src/User.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-04: tests/Traits/CreatesUsers.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_04_test_trait() {
        // Given: a test trait in tests/Traits/
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("tests/Traits/CreatesUsers.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-05: bootstrap/app.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_05_bootstrap() {
        // Given: a bootstrap file
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("bootstrap/app.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-FUNC-01: public function createUser() -> name="createUser", is_exported=true
    // -----------------------------------------------------------------------
    #[test]
    fn php_func_01_public_method() {
        // Given: a class with a public method
        // When: extract_production_functions is called
        // Then: name="createUser", is_exported=true
        let ext = PhpExtractor::new();
        let source = "<?php\nclass User {\n    public function createUser() {}\n}";
        let fns = ext.extract_production_functions(source, "src/User.php");
        let f = fns.iter().find(|f| f.name == "createUser").unwrap();
        assert!(f.is_exported);
    }

    // -----------------------------------------------------------------------
    // PHP-FUNC-02: private function helper() -> name="helper", is_exported=false
    // -----------------------------------------------------------------------
    #[test]
    fn php_func_02_private_method() {
        // Given: a class with a private method
        // When: extract_production_functions is called
        // Then: name="helper", is_exported=false
        let ext = PhpExtractor::new();
        let source = "<?php\nclass User {\n    private function helper() {}\n}";
        let fns = ext.extract_production_functions(source, "src/User.php");
        let f = fns.iter().find(|f| f.name == "helper").unwrap();
        assert!(!f.is_exported);
    }

    // -----------------------------------------------------------------------
    // PHP-FUNC-03: class User { public function save() } -> class_name=Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn php_func_03_class_method() {
        // Given: a class User with a public method save()
        // When: extract_production_functions is called
        // Then: name="save", class_name=Some("User")
        let ext = PhpExtractor::new();
        let source = "<?php\nclass User {\n    public function save() {}\n}";
        let fns = ext.extract_production_functions(source, "src/User.php");
        let f = fns.iter().find(|f| f.name == "save").unwrap();
        assert_eq!(f.class_name, Some("User".to_string()));
    }

    // -----------------------------------------------------------------------
    // PHP-FUNC-04: function global_helper() (top-level) -> exported
    // -----------------------------------------------------------------------
    #[test]
    fn php_func_04_top_level_function() {
        // Given: a top-level function global_helper()
        // When: extract_production_functions is called
        // Then: name="global_helper", is_exported=true
        let ext = PhpExtractor::new();
        let source = "<?php\nfunction global_helper() {\n    return 42;\n}";
        let fns = ext.extract_production_functions(source, "src/helpers.php");
        let f = fns.iter().find(|f| f.name == "global_helper").unwrap();
        assert!(f.is_exported);
        assert_eq!(f.class_name, None);
    }

    // -----------------------------------------------------------------------
    // PHP-IMP-01: use App\Models\User; -> ("App/Models", ["User"])
    // -----------------------------------------------------------------------
    #[test]
    fn php_imp_01_app_models() {
        // Given: a use statement for App\Models\User
        // When: extract_all_import_specifiers is called
        // Then: returns ("App/Models", ["User"])
        let ext = PhpExtractor::new();
        let source = "<?php\nuse App\\Models\\User;\n";
        let imports = ext.extract_all_import_specifiers(source);
        assert!(
            imports
                .iter()
                .any(|(m, s)| m == "App/Models" && s.contains(&"User".to_string())),
            "expected App/Models -> [User], got: {imports:?}"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-IMP-02: use App\Services\UserService; -> ("App/Services", ["UserService"])
    // -----------------------------------------------------------------------
    #[test]
    fn php_imp_02_app_services() {
        // Given: a use statement for App\Services\UserService
        // When: extract_all_import_specifiers is called
        // Then: returns ("App/Services", ["UserService"])
        let ext = PhpExtractor::new();
        let source = "<?php\nuse App\\Services\\UserService;\n";
        let imports = ext.extract_all_import_specifiers(source);
        assert!(
            imports
                .iter()
                .any(|(m, s)| m == "App/Services" && s.contains(&"UserService".to_string())),
            "expected App/Services -> [UserService], got: {imports:?}"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-IMP-03: use PHPUnit\Framework\TestCase; -> external package -> skipped
    // -----------------------------------------------------------------------
    #[test]
    fn php_imp_03_external_phpunit() {
        // Given: a use statement for external PHPUnit package
        // When: extract_all_import_specifiers is called
        // Then: returns empty (external packages are filtered)
        let ext = PhpExtractor::new();
        let source = "<?php\nuse PHPUnit\\Framework\\TestCase;\n";
        let imports = ext.extract_all_import_specifiers(source);
        assert!(
            imports.is_empty(),
            "external PHPUnit should be filtered, got: {imports:?}"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-IMP-04: use Illuminate\Http\Request; -> external package -> skipped
    // -----------------------------------------------------------------------
    #[test]
    fn php_imp_04_external_illuminate() {
        // Given: a use statement for external Illuminate (Laravel) package
        // When: extract_all_import_specifiers is called
        // Then: returns empty (external packages are filtered)
        let ext = PhpExtractor::new();
        let source = "<?php\nuse Illuminate\\Http\\Request;\n";
        let imports = ext.extract_all_import_specifiers(source);
        assert!(
            imports.is_empty(),
            "external Illuminate should be filtered, got: {imports:?}"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-E2E-01: User.php + UserTest.php in the same directory -> Layer 1 stem match
    // -----------------------------------------------------------------------
    #[test]
    fn php_e2e_01_stem_match() {
        // Given: production file User.php and test file UserTest.php in the same directory
        // (Layer 1 stem matching works when files share the same parent directory)
        // When: map_test_files_with_imports is called
        // Then: UserTest.php is matched to User.php via Layer 1 stem matching
        let dir = tempfile::tempdir().expect("failed to create tempdir");

        let prod_file = dir.path().join("User.php");
        std::fs::write(&prod_file, "<?php\nclass User {}").unwrap();

        let test_file = dir.path().join("UserTest.php");
        std::fs::write(&test_file, "<?php\nclass UserTest extends TestCase {}").unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            "<?php\nclass UserTest extends TestCase {}".to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        assert!(!mappings.is_empty(), "expected at least one mapping");
        let user_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("User.php"))
            .expect("expected User.php in mappings");
        assert!(
            !user_mapping.test_files.is_empty(),
            "expected UserTest.php to be mapped to User.php via Layer 1 stem match"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-E2E-02: tests/ServiceTest.php imports use App\Services\OrderService
    //             -> Layer 2 PSR-4 import match
    // -----------------------------------------------------------------------
    #[test]
    fn php_e2e_02_import_match() {
        // Given: production file app/Services/OrderService.php
        //        and test file tests/ServiceTest.php with `use App\Services\OrderService;`
        // When: map_test_files_with_imports is called
        // Then: ServiceTest.php is matched to OrderService.php via Layer 2 import tracing
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let services_dir = dir.path().join("app").join("Services");
        std::fs::create_dir_all(&services_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = services_dir.join("OrderService.php");
        std::fs::write(&prod_file, "<?php\nclass OrderService {}").unwrap();

        let test_file = test_dir.join("ServiceTest.php");
        let test_source =
            "<?php\nuse App\\Services\\OrderService;\nclass ServiceTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        let order_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("OrderService.php"))
            .expect("expected OrderService.php in mappings");
        assert!(
            !order_mapping.test_files.is_empty(),
            "expected ServiceTest.php to be mapped to OrderService.php via import tracing"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-E2E-03: tests/TestCase.php -> helper exclusion
    // -----------------------------------------------------------------------
    #[test]
    fn php_e2e_03_helper_exclusion() {
        // Given: a TestCase.php base class in tests/
        // When: map_test_files_with_imports is called
        // Then: TestCase.php is excluded (is_non_sut_helper = true)
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = src_dir.join("User.php");
        std::fs::write(&prod_file, "<?php\nclass User {}").unwrap();

        // TestCase.php should be treated as a helper, not a test file
        let test_case_file = test_dir.join("TestCase.php");
        std::fs::write(&test_case_file, "<?php\nabstract class TestCase {}").unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_case_file.to_string_lossy().into_owned(),
            "<?php\nabstract class TestCase {}".to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        // TestCase.php should not be matched to User.php
        let user_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("User.php"));
        if let Some(mapping) = user_mapping {
            assert!(
                mapping.test_files.is_empty()
                    || !mapping
                        .test_files
                        .iter()
                        .any(|t| t.contains("TestCase.php")),
                "TestCase.php should not be mapped as a test file for User.php"
            );
        }
    }

    // -----------------------------------------------------------------------
    // PHP-FW-01: laravel/framework layout -> Illuminate import resolves locally
    // -----------------------------------------------------------------------
    #[test]
    fn php_fw_01_laravel_framework_self_test() {
        // Given: laravel/framework layout with src/Illuminate/Http/Request.php
        //        and tests/Http/RequestTest.php importing `use Illuminate\Http\Request`
        // When: map_test_files_with_imports is called
        // Then: RequestTest.php is mapped to Request.php via Layer 2
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let src_dir = dir.path().join("src").join("Illuminate").join("Http");
        std::fs::create_dir_all(&src_dir).unwrap();
        let test_dir = dir.path().join("tests").join("Http");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = src_dir.join("Request.php");
        std::fs::write(
            &prod_file,
            "<?php\nnamespace Illuminate\\Http;\nclass Request {}",
        )
        .unwrap();

        let test_file = test_dir.join("RequestTest.php");
        let test_source =
            "<?php\nuse Illuminate\\Http\\Request;\nclass RequestTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        let request_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("Request.php"))
            .expect("expected Request.php in mappings");
        assert!(
            request_mapping
                .test_files
                .iter()
                .any(|t| t.contains("RequestTest.php")),
            "expected RequestTest.php to be mapped to Request.php via Layer 2, got: {:?}",
            request_mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PHP-FW-02: normal app -> Illuminate import still filtered (no local source)
    // -----------------------------------------------------------------------
    #[test]
    fn php_fw_02_normal_app_illuminate_filtered() {
        // Given: normal app layout with app/Models/User.php
        //        and tests/UserTest.php importing `use Illuminate\Http\Request`
        //        (no local Illuminate directory)
        // When: map_test_files_with_imports is called
        // Then: Illuminate import is NOT resolved (no mapping via import)
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let app_dir = dir.path().join("app").join("Models");
        std::fs::create_dir_all(&app_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = app_dir.join("User.php");
        std::fs::write(&prod_file, "<?php\nclass User {}").unwrap();

        // This test imports Illuminate but there's no local Illuminate source
        let test_file = test_dir.join("OrderTest.php");
        let test_source =
            "<?php\nuse Illuminate\\Http\\Request;\nclass OrderTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        // User.php should not have OrderTest.php mapped (no stem match, no import match)
        let user_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("User.php"))
            .expect("expected User.php in mappings");
        assert!(
            !user_mapping
                .test_files
                .iter()
                .any(|t| t.contains("OrderTest.php")),
            "Illuminate import should be filtered when no local source exists"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-FW-03: PHPUnit import still filtered via integration test (regression)
    // -----------------------------------------------------------------------
    #[test]
    fn php_fw_03_phpunit_still_external() {
        // Given: app with src/Calculator.php and tests/CalculatorTest.php
        //        importing only `use PHPUnit\Framework\TestCase` (no local PHPUnit source)
        // When: map_test_files_with_imports is called
        // Then: PHPUnit import does not create a false mapping
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = src_dir.join("Calculator.php");
        std::fs::write(&prod_file, "<?php\nclass Calculator {}").unwrap();

        // Test imports only PHPUnit (external) — no import-based mapping should occur
        let test_file = test_dir.join("OtherTest.php");
        let test_source =
            "<?php\nuse PHPUnit\\Framework\\TestCase;\nclass OtherTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        let calc_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("Calculator.php"))
            .expect("expected Calculator.php in mappings");
        assert!(
            !calc_mapping
                .test_files
                .iter()
                .any(|t| t.contains("OtherTest.php")),
            "PHPUnit import should not create a mapping to Calculator.php"
        );
    }

    // -----------------------------------------------------------------------
    // PHP-FW-04: symfony/symfony layout -> Symfony import resolves locally
    // -----------------------------------------------------------------------
    #[test]
    fn php_fw_04_symfony_self_test() {
        // Given: symfony layout with src/Symfony/Component/HttpFoundation/Request.php
        //        and tests/HttpFoundation/RequestTest.php importing
        //        `use Symfony\Component\HttpFoundation\Request`
        // When: map_test_files_with_imports is called
        // Then: RequestTest.php is mapped to Request.php via Layer 2
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let src_dir = dir
            .path()
            .join("src")
            .join("Symfony")
            .join("Component")
            .join("HttpFoundation");
        std::fs::create_dir_all(&src_dir).unwrap();
        let test_dir = dir.path().join("tests").join("HttpFoundation");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = src_dir.join("Request.php");
        std::fs::write(
            &prod_file,
            "<?php\nnamespace Symfony\\Component\\HttpFoundation;\nclass Request {}",
        )
        .unwrap();

        let test_file = test_dir.join("RequestTest.php");
        let test_source = "<?php\nuse Symfony\\Component\\HttpFoundation\\Request;\nclass RequestTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        let request_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("Request.php"))
            .expect("expected Request.php in mappings");
        assert!(
            request_mapping
                .test_files
                .iter()
                .any(|t| t.contains("RequestTest.php")),
            "expected RequestTest.php to be mapped to Request.php via Layer 2, got: {:?}",
            request_mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-06: tests/Fixtures/SomeHelper.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_06_fixtures_dir() {
        // Given: a file in tests/Fixtures/
        // When: is_non_sut_helper is called
        // Then: returns true (Fixtures are test infrastructure, not SUT)
        assert!(is_non_sut_helper("tests/Fixtures/SomeHelper.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-07: tests/Fixtures/nested/Stub.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_07_fixtures_nested() {
        // Given: a file in tests/Fixtures/nested/
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("tests/Fixtures/nested/Stub.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-08: tests/Stubs/UserStub.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_08_stubs_dir() {
        // Given: a file in tests/Stubs/
        // When: is_non_sut_helper is called
        // Then: returns true (Stubs are test infrastructure, not SUT)
        assert!(is_non_sut_helper("tests/Stubs/UserStub.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-09: tests/Stubs/nested/FakeRepo.php -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_09_stubs_nested() {
        // Given: a file in tests/Stubs/nested/
        // When: is_non_sut_helper is called
        // Then: returns true
        assert!(is_non_sut_helper("tests/Stubs/nested/FakeRepo.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-HELPER-10: app/Stubs/Template.php -> is_non_sut_helper = false (guard test)
    // -----------------------------------------------------------------------
    #[test]
    fn php_helper_10_non_test_stubs() {
        // Given: a file in app/Stubs/ (not under tests/)
        // When: is_non_sut_helper is called
        // Then: returns false (only tests/ subdirs are filtered)
        assert!(!is_non_sut_helper("app/Stubs/Template.php", false));
    }

    // -----------------------------------------------------------------------
    // PHP-PSR4-01: custom_src/ prefix via composer.json -> resolution success
    // -----------------------------------------------------------------------
    #[test]
    fn php_psr4_01_composer_json_resolution() {
        // Given: a project with composer.json defining PSR-4 autoload:
        //   {"autoload": {"psr-4": {"MyApp\\": "custom_src/"}}}
        //   production file: custom_src/Models/Order.php
        //   test file: tests/OrderTest.php with `use MyApp\Models\Order;`
        // When: map_test_files_with_imports is called
        // Then: OrderTest.php is matched to Order.php via PSR-4 resolution
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let custom_src_dir = dir.path().join("custom_src").join("Models");
        std::fs::create_dir_all(&custom_src_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Write composer.json with custom PSR-4 prefix
        let composer_json = r#"{"autoload": {"psr-4": {"MyApp\\": "custom_src/"}}}"#;
        std::fs::write(dir.path().join("composer.json"), composer_json).unwrap();

        let prod_file = custom_src_dir.join("Order.php");
        std::fs::write(
            &prod_file,
            "<?php\nnamespace MyApp\\Models;\nclass Order {}",
        )
        .unwrap();

        let test_file = test_dir.join("OrderTest.php");
        let test_source = "<?php\nuse MyApp\\Models\\Order;\nclass OrderTest extends TestCase {}";
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        let order_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("Order.php"))
            .expect("expected Order.php in mappings");
        assert!(
            order_mapping
                .test_files
                .iter()
                .any(|t| t.contains("OrderTest.php")),
            "expected OrderTest.php to be mapped to Order.php via PSR-4 composer.json resolution, got: {:?}",
            order_mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PHP-CLI-01: observe --lang php . -> CLI dispatch verification
    // -----------------------------------------------------------------------
    #[test]
    fn php_cli_01_dispatch() {
        // Given: a tempdir with a PHP file
        // When: PhpExtractor::map_test_files_with_imports is called on an empty project
        // Then: returns an empty (or valid) mapping without panicking
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let ext = PhpExtractor::new();
        let production_files: Vec<String> = vec![];
        let test_sources: HashMap<String, String> = HashMap::new();
        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);
        assert!(mappings.is_empty());
    }
}

// ---------------------------------------------------------------------------
// PHP-PARENT tests: parent class import propagation (TC-01 to TC-04)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod parent_class_tests {
    use super::*;
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // TC-01: Given test extends ParentClass in same dir, When parent has
    //        `use Illuminate\Foo`, Then Foo is in test's import list
    // -----------------------------------------------------------------------
    #[test]
    fn tc01_parent_imports_propagated_to_child() {
        // Given: parent class with Illuminate imports in same directory as child
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let parent_source = r#"<?php
namespace App\Tests;
use Illuminate\View\Compilers\BladeCompiler;
use Illuminate\Container\Container;
use PHPUnit\Framework\TestCase;
abstract class AbstractBaseTest extends TestCase {}"#;

        let parent_file = test_dir.join("AbstractBaseTest.php");
        std::fs::write(&parent_file, parent_source).unwrap();

        let child_source = r#"<?php
namespace App\Tests;
class ChildTest extends AbstractBaseTest {
    public function testSomething() { $this->assertTrue(true); }
}"#;

        // When: extract_parent_class_imports is called on the child
        let parent_imports = PhpExtractor::extract_parent_class_imports(
            child_source,
            &parent_file.parent().unwrap().to_string_lossy(),
        );

        // Then: Illuminate imports from parent are returned
        // (Illuminate is normally external, but parent_class_imports returns raw specifiers
        //  from parent file — filtering for L2 is done at the call site)
        assert!(
            !parent_imports.is_empty(),
            "expected parent Illuminate imports to be propagated, got: {parent_imports:?}"
        );
        let has_blade = parent_imports
            .iter()
            .any(|(m, _)| m.contains("BladeCompiler") || m.contains("Compilers"));
        let has_container = parent_imports.iter().any(|(m, _)| m.contains("Container"));
        assert!(
            has_blade || has_container,
            "expected BladeCompiler or Container in parent imports, got: {parent_imports:?}"
        );
    }

    // -----------------------------------------------------------------------
    // TC-02: Given test extends ParentClass, When parent has no production
    //        imports, Then no additional imports are added (guard test)
    // -----------------------------------------------------------------------
    #[test]
    fn tc02_parent_with_no_production_imports_adds_nothing() {
        // Given: a parent class that only imports PHPUnit (external, no production imports)
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();
        let app_dir = dir.path().join("app").join("Models");
        std::fs::create_dir_all(&app_dir).unwrap();

        let parent_source = r#"<?php
namespace App\Tests;
use PHPUnit\Framework\TestCase;
abstract class MinimalBaseTest extends TestCase {}"#;

        let parent_file = test_dir.join("MinimalBaseTest.php");
        std::fs::write(&parent_file, parent_source).unwrap();

        let child_source = r#"<?php
namespace App\Tests;
use App\Models\Order;
class OrderTest extends MinimalBaseTest {
    public function testOrder() { $this->assertTrue(true); }
}"#;

        let child_file = test_dir.join("OrderTest.php");
        std::fs::write(&child_file, child_source).unwrap();

        let prod_file = app_dir.join("Order.php");
        std::fs::write(&prod_file, "<?php\nnamespace App\\Models;\nclass Order {}").unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            child_file.to_string_lossy().into_owned(),
            child_source.to_string(),
        );

        // When: map_test_files_with_imports is called (parent has no production imports)
        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        // Then: OrderTest.php is still matched to Order.php via its own import (L2)
        // and parent's lack of production imports does not break anything
        let order_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("Order.php"))
            .expect("expected Order.php in mappings");
        assert!(
            !order_mapping.test_files.is_empty(),
            "expected OrderTest.php to be mapped to Order.php (child's own import), got empty"
        );
    }

    // -----------------------------------------------------------------------
    // TC-03: Given test extends external class (PHPUnit\TestCase),
    //        When resolve parent, Then skip (external namespace guard)
    // -----------------------------------------------------------------------
    #[test]
    fn tc03_external_parent_class_skipped() {
        // Given: a test that extends PHPUnit\Framework\TestCase directly
        //        (no local parent file to trace)
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let app_dir = dir.path().join("app").join("Services");
        std::fs::create_dir_all(&app_dir).unwrap();
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let prod_file = app_dir.join("PaymentService.php");
        std::fs::write(
            &prod_file,
            "<?php\nnamespace App\\Services;\nclass PaymentService {}",
        )
        .unwrap();

        // This test directly extends TestCase (external), not a local abstract class
        let test_source = r#"<?php
use PHPUnit\Framework\TestCase;
use App\Services\PaymentService;
class PaymentServiceTest extends TestCase {
    public function testPay() { $this->assertTrue(true); }
}"#;
        let test_file = test_dir.join("PaymentServiceTest.php");
        std::fs::write(&test_file, test_source).unwrap();

        let ext = PhpExtractor::new();
        let production_files = vec![prod_file.to_string_lossy().into_owned()];
        let mut test_sources = HashMap::new();
        test_sources.insert(
            test_file.to_string_lossy().into_owned(),
            test_source.to_string(),
        );

        // When: map_test_files_with_imports is called
        let mappings =
            ext.map_test_files_with_imports(&production_files, &test_sources, dir.path(), false);

        // Then: PaymentServiceTest.php is matched to PaymentService.php
        //       (its own import works; external parent does not cause errors)
        let payment_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("PaymentService.php"))
            .expect("expected PaymentService.php in mappings");
        assert!(
            payment_mapping
                .test_files
                .iter()
                .any(|t| t.contains("PaymentServiceTest.php")),
            "expected PaymentServiceTest.php mapped via own import; got: {:?}",
            payment_mapping.test_files
        );
        // No panic, no infinite loop = external parent was skipped silently
    }

    // -----------------------------------------------------------------------
    // TC-04: Given circular inheritance (A extends B, B extends A),
    //        When extract_parent_class_imports is called, Then no infinite loop
    // -----------------------------------------------------------------------
    #[test]
    fn tc04_circular_inheritance_no_infinite_loop() {
        // Given: two files that mutually extend each other (pathological case)
        let dir = tempfile::tempdir().expect("failed to create tempdir");
        let test_dir = dir.path().join("tests");
        std::fs::create_dir_all(&test_dir).unwrap();

        let a_source = r#"<?php
namespace App\Tests;
use App\Models\Foo;
class ATest extends BTest {}"#;

        let b_source = r#"<?php
namespace App\Tests;
use App\Models\Bar;
class BTest extends ATest {}"#;

        let a_file = test_dir.join("ATest.php");
        let b_file = test_dir.join("BTest.php");
        std::fs::write(&a_file, a_source).unwrap();
        std::fs::write(&b_file, b_source).unwrap();

        // When: extract_parent_class_imports is called on A (which extends B, which extends A)
        // Then: returns without infinite loop (function must complete in finite time)
        let result =
            PhpExtractor::extract_parent_class_imports(a_source, &test_dir.to_string_lossy());

        // The result may be empty or contain Bar; crucially it must NOT hang.
        // Just asserting this line is reached proves no infinite loop.
        let _ = result;
    }

    // -----------------------------------------------------------------------
    // TC-05: Given Laravel observe after fix, When measure recall, Then R > 90%
    // -----------------------------------------------------------------------
    #[test]
    #[ignore = "integration: requires local Laravel ground truth at /tmp/laravel"]
    fn tc05_laravel_recall_above_90_percent() {
        // Given: Laravel source tree at /tmp/laravel
        // When: observe --lang php is run
        // Then: Recall > 90% (parent class import propagation resolves AbstractBladeTestCase FN)
        // NOTE: This is a placeholder. Actual measurement is done manually via:
        //   cargo run -- observe --lang php --format json /tmp/laravel
        // and compared against the ground truth in docs/dogfooding-results.md.
        unimplemented!(
            "Integration test: run `cargo run -- observe --lang php --format json /tmp/laravel`"
        );
    }

    // -----------------------------------------------------------------------
    // TC-06: Given Laravel observe after fix, When check precision, Then no new FP
    // -----------------------------------------------------------------------
    #[test]
    #[ignore = "integration: requires local Laravel ground truth at /tmp/laravel"]
    fn tc06_laravel_no_new_false_positives() {
        // Given: Laravel source tree at /tmp/laravel
        // When: observe --lang php is run after parent class propagation is implemented
        // Then: Precision >= 96% (no new false positives introduced by parent import merging)
        // NOTE: This is a placeholder. Actual measurement is done manually.
        unimplemented!(
            "Integration test: run `cargo run -- observe --lang php --format json /tmp/laravel`"
        );
    }

    // -----------------------------------------------------------------------
    // TC-01: Controller array syntax -> Route{ GET, /users, index, UserController }
    // -----------------------------------------------------------------------
    #[test]
    fn tc01_controller_array_syntax() {
        // Given: a route file with Route::get pointing to a controller array
        let source = r#"<?php
use Illuminate\Support\Facades\Route;
Route::get('/users', [UserController::class, 'index']);
"#;
        let extractor = PhpExtractor;

        // When: extract_routes is called
        let routes = extractor.extract_routes(source, "routes/web.php");

        // Then: one route with GET /users, handler=index, class=UserController
        assert_eq!(routes.len(), 1);
        let r = &routes[0];
        assert_eq!(r.http_method, "GET");
        assert_eq!(r.path, "/users");
        assert_eq!(r.handler_name, "index");
        assert_eq!(r.class_name, "UserController");
        assert_eq!(r.file, "routes/web.php");
    }

    // -----------------------------------------------------------------------
    // TC-02: Closure handler -> Route{ POST, /users, "", "" }
    // -----------------------------------------------------------------------
    #[test]
    fn tc02_closure_handler() {
        // Given: a route file with Route::post using a closure handler
        let source = r#"<?php
use Illuminate\Support\Facades\Route;
Route::post('/users', fn () => response()->json(['ok' => true]));
"#;
        let extractor = PhpExtractor;

        // When: extract_routes is called
        let routes = extractor.extract_routes(source, "routes/web.php");

        // Then: one route with POST /users, handler="" and class="" (closure, no named handler)
        assert_eq!(routes.len(), 1);
        let r = &routes[0];
        assert_eq!(r.http_method, "POST");
        assert_eq!(r.path, "/users");
        assert_eq!(r.handler_name, "");
        assert_eq!(r.class_name, "");
    }

    // -----------------------------------------------------------------------
    // TC-03: prefix group (depth 1) -> Route{ GET, admin/users, ... }
    // -----------------------------------------------------------------------
    #[test]
    fn tc03_prefix_group_depth1() {
        // Given: a route file with a single prefix->group wrapping an inner route
        let source = r#"<?php
use Illuminate\Support\Facades\Route;
Route::prefix('admin')->group(function () {
    Route::get('/users', [UserController::class, 'index']);
});
"#;
        let extractor = PhpExtractor;

        // When: extract_routes is called
        let routes = extractor.extract_routes(source, "routes/web.php");

        // Then: one route with resolved path admin/users
        assert_eq!(routes.len(), 1);
        let r = &routes[0];
        assert_eq!(r.http_method, "GET");
        assert_eq!(r.path, "admin/users");
    }

    // -----------------------------------------------------------------------
    // TC-04: nested prefix group (depth 2) -> Route{ GET, api/v1/users, ... }
    // -----------------------------------------------------------------------
    #[test]
    fn tc04_nested_prefix_group_depth2() {
        // Given: a route file with two nested prefix->group blocks
        let source = r#"<?php
use Illuminate\Support\Facades\Route;
Route::prefix('api')->group(fn() =>
    Route::prefix('v1')->group(fn() =>
        Route::get('/users', [UserController::class, 'index'])
    )
);
"#;
        let extractor = PhpExtractor;

        // When: extract_routes is called
        let routes = extractor.extract_routes(source, "routes/web.php");

        // Then: one route with resolved path api/v1/users
        assert_eq!(routes.len(), 1);
        let r = &routes[0];
        assert_eq!(r.http_method, "GET");
        assert_eq!(r.path, "api/v1/users");
    }

    // -----------------------------------------------------------------------
    // TC-05: middleware group (no prefix) -> path unaffected
    // -----------------------------------------------------------------------
    #[test]
    fn tc05_middleware_group_no_prefix_effect() {
        // Given: a route file where routes are wrapped in a middleware group only
        let source = r#"<?php
use Illuminate\Support\Facades\Route;
Route::middleware('auth')->group(function () {
    Route::get('/dashboard', [DashboardController::class, 'index']);
});
"#;
        let extractor = PhpExtractor;

        // When: extract_routes is called
        let routes = extractor.extract_routes(source, "routes/web.php");

        // Then: one route with path /dashboard (middleware does not alter path)
        assert_eq!(routes.len(), 1);
        let r = &routes[0];
        assert_eq!(r.http_method, "GET");
        assert_eq!(r.path, "/dashboard");
    }
}
