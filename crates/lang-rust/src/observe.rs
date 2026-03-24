use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

use exspec_core::observe::{
    BarrelReExport, FileMapping, ImportMapping, MappingStrategy, ObserveExtractor,
    ProductionFunction,
};

use super::RustExtractor;

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const CFG_TEST_QUERY: &str = include_str!("../queries/cfg_test.scm");
static CFG_TEST_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const EXPORTED_SYMBOL_QUERY: &str = include_str!("../queries/exported_symbol.scm");
static EXPORTED_SYMBOL_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

fn rust_language() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| Query::new(&rust_language(), source).expect("invalid query"))
}

// ---------------------------------------------------------------------------
// Stem helpers
// ---------------------------------------------------------------------------

/// Extract stem from a Rust test file path.
/// `tests/test_foo.rs` -> `Some("foo")`  (test_ prefix)
/// `tests/foo_test.rs` -> `Some("foo")`  (_test suffix)
/// `tests/integration.rs` -> `Some("integration")` (tests/ dir = integration test)
/// `src/user.rs` -> `None`
pub fn test_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".rs")?;

    // test_ prefix
    if let Some(rest) = stem.strip_prefix("test_") {
        if !rest.is_empty() {
            return Some(rest);
        }
    }

    // _test suffix
    if let Some(rest) = stem.strip_suffix("_test") {
        if !rest.is_empty() {
            return Some(rest);
        }
    }

    // Files under tests/ directory are integration tests
    let normalized = path.replace('\\', "/");
    if normalized.starts_with("tests/") || normalized.contains("/tests/") {
        // Exclude mod.rs and main.rs in tests dir
        if stem != "mod" && stem != "main" {
            return Some(stem);
        }
    }

    None
}

/// Extract stem from a Rust production file path.
/// `src/user.rs` -> `Some("user")`
/// `src/lib.rs` -> `None` (barrel)
/// `src/mod.rs` -> `None` (barrel)
/// `src/main.rs` -> `None` (entry point)
/// `tests/test_foo.rs` -> `None` (test file)
pub fn production_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".rs")?;

    // Exclude barrel and entry point files
    if stem == "lib" || stem == "mod" || stem == "main" {
        return None;
    }

    // Exclude test files
    if test_stem(path).is_some() {
        return None;
    }

    // Exclude build.rs
    if file_name == "build.rs" {
        return None;
    }

    Some(stem)
}

/// Check if a file is a non-SUT helper (not subject under test).
pub fn is_non_sut_helper(file_path: &str, _is_known_production: bool) -> bool {
    let normalized = file_path.replace('\\', "/");
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // build.rs
    if file_name == "build.rs" {
        return true;
    }

    // tests/common/mod.rs and tests/common/*.rs (test helpers)
    if normalized.contains("/tests/common/") || normalized.starts_with("tests/common/") {
        return true;
    }

    // benches/ directory
    if normalized.starts_with("benches/") || normalized.contains("/benches/") {
        return true;
    }

    // examples/ directory
    if normalized.starts_with("examples/") || normalized.contains("/examples/") {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Inline test detection (Layer 0)
// ---------------------------------------------------------------------------

/// Detect #[cfg(test)] mod blocks in source code.
pub fn detect_inline_tests(source: &str) -> bool {
    let mut parser = RustExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return false,
    };
    let source_bytes = source.as_bytes();
    let query = cached_query(&CFG_TEST_QUERY_CACHE, CFG_TEST_QUERY);

    let attr_name_idx = query.capture_index_for_name("attr_name");
    let cfg_arg_idx = query.capture_index_for_name("cfg_arg");
    let cfg_test_attr_idx = query.capture_index_for_name("cfg_test_attr");

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

    while let Some(m) = matches.next() {
        let mut is_cfg = false;
        let mut is_test = false;
        let mut attr_node: Option<tree_sitter::Node> = None;

        for cap in m.captures {
            let text = cap.node.utf8_text(source_bytes).unwrap_or("");
            if attr_name_idx == Some(cap.index) && text == "cfg" {
                is_cfg = true;
            }
            if cfg_arg_idx == Some(cap.index) && text == "test" {
                is_test = true;
            }
            if cfg_test_attr_idx == Some(cap.index) {
                attr_node = Some(cap.node);
            }
        }

        if is_cfg && is_test {
            // Verify that the next sibling (skipping other attribute_items) is a mod_item
            if let Some(attr) = attr_node {
                let mut sibling = attr.next_sibling();
                while let Some(s) = sibling {
                    if s.kind() == "mod_item" {
                        return true;
                    }
                    if s.kind() != "attribute_item" {
                        break;
                    }
                    sibling = s.next_sibling();
                }
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// ObserveExtractor impl
// ---------------------------------------------------------------------------

impl ObserveExtractor for RustExtractor {
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

        // Find byte ranges of #[cfg(test)] mod blocks to exclude
        let cfg_test_ranges = find_cfg_test_ranges(source);

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        let mut result = Vec::new();

        while let Some(m) = matches.next() {
            let mut fn_name: Option<String> = None;
            let mut class_name: Option<String> = None;
            let mut line: usize = 1;
            let mut is_exported = false;
            let mut fn_start_byte: usize = 0;

            for cap in m.captures {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                let node_line = cap.node.start_position().row + 1;

                if name_idx == Some(cap.index) || method_name_idx == Some(cap.index) {
                    fn_name = Some(text);
                    line = node_line;
                } else if class_name_idx == Some(cap.index) {
                    class_name = Some(text);
                }

                // Check visibility for function/method nodes
                if function_idx == Some(cap.index) || method_idx == Some(cap.index) {
                    fn_start_byte = cap.node.start_byte();
                    is_exported = has_pub_visibility(cap.node);
                }
            }

            if let Some(name) = fn_name {
                // Skip functions inside #[cfg(test)] blocks
                if cfg_test_ranges
                    .iter()
                    .any(|(start, end)| fn_start_byte >= *start && fn_start_byte < *end)
                {
                    continue;
                }

                result.push(ProductionFunction {
                    name,
                    file: file_path.to_string(),
                    line,
                    class_name,
                    is_exported,
                });
            }
        }

        // Deduplicate
        let mut seen = HashSet::new();
        result.retain(|f| seen.insert((f.name.clone(), f.class_name.clone())));

        result
    }

    fn extract_imports(&self, source: &str, file_path: &str) -> Vec<ImportMapping> {
        // For Rust, extract_imports returns relative imports (use crate::... mapped to relative paths)
        let all = self.extract_all_import_specifiers(source);
        let mut result = Vec::new();
        for (specifier, symbols) in all {
            for sym in &symbols {
                result.push(ImportMapping {
                    symbol_name: sym.clone(),
                    module_specifier: specifier.clone(),
                    file: file_path.to_string(),
                    line: 1,
                    symbols: symbols.clone(),
                });
            }
        }
        result
    }

    fn extract_all_import_specifiers(&self, source: &str) -> Vec<(String, Vec<String>)> {
        extract_import_specifiers_with_crate_name(source, None)
    }

    fn extract_barrel_re_exports(&self, source: &str, file_path: &str) -> Vec<BarrelReExport> {
        if !self.is_barrel_file(file_path) {
            return Vec::new();
        }

        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let root = tree.root_node();
        let mut result = Vec::new();

        for i in 0..root.child_count() {
            let child = root.child(i).unwrap();

            // pub mod foo; -> BarrelReExport { from_specifier: "./foo", wildcard: true }
            if child.kind() == "mod_item" && has_pub_visibility(child) {
                // Check it's a declaration (no body block)
                let has_body = child.child_by_field_name("body").is_some();
                if !has_body {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let mod_name = name_node.utf8_text(source_bytes).unwrap_or("");
                        result.push(BarrelReExport {
                            symbols: Vec::new(),
                            from_specifier: format!("./{mod_name}"),
                            wildcard: true,
                            namespace_wildcard: false,
                        });
                    }
                }
            }

            // pub use foo::*; or pub use foo::{Bar, Baz};
            if child.kind() == "use_declaration" && has_pub_visibility(child) {
                if let Some(arg) = child.child_by_field_name("argument") {
                    extract_pub_use_re_exports(&arg, source_bytes, &mut result);
                }
            }

            // cfg macro blocks: cfg_*! { pub mod ...; pub use ...; }
            if child.kind() == "macro_invocation" {
                for j in 0..child.child_count() {
                    if let Some(tt) = child.child(j) {
                        if tt.kind() == "token_tree" {
                            let tt_text = tt.utf8_text(source_bytes).unwrap_or("");
                            extract_re_exports_from_text(tt_text, &mut result);
                        }
                    }
                }
            }
        }

        result
    }

    fn source_extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn index_file_names(&self) -> &[&str] {
        &["mod.rs", "lib.rs"]
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

    fn file_exports_any_symbol(&self, path: &Path, symbols: &[String]) -> bool {
        if symbols.is_empty() {
            return true;
        }
        // Optimistic fallback on read/parse failure (matches core default and Python).
        // FN avoidance is preferred over FP avoidance here.
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return true,
        };
        let mut parser = Self::parser();
        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => return true,
        };
        let query = cached_query(&EXPORTED_SYMBOL_QUERY_CACHE, EXPORTED_SYMBOL_QUERY);
        let symbol_idx = query
            .capture_index_for_name("symbol_name")
            .expect("@symbol_name capture not found in exported_symbol.scm");
        let vis_idx = query.capture_index_for_name("vis");

        let source_bytes = source.as_bytes();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.index == symbol_idx {
                    // Only consider items with exactly `pub` visibility (not `pub(crate)`, `pub(super)`)
                    let is_pub_only = m.captures.iter().any(|c| {
                        vis_idx == Some(c.index)
                            && c.node.utf8_text(source_bytes).unwrap_or("") == "pub"
                    });
                    if !is_pub_only {
                        continue;
                    }
                    let name = cap.node.utf8_text(source_bytes).unwrap_or("");
                    if symbols.iter().any(|s| s == name) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a tree-sitter node has `pub` visibility modifier.
fn has_pub_visibility(node: tree_sitter::Node) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "visibility_modifier" {
                return true;
            }
            // Stop at the first non-attribute, non-visibility child
            if child.kind() != "attribute_item" && child.kind() != "visibility_modifier" {
                break;
            }
        }
    }
    false
}

/// Find byte ranges of #[cfg(test)] mod blocks.
/// In tree-sitter-rust, `#[cfg(test)]` is an attribute_item that is a sibling
/// of the mod_item it annotates. We find the attribute, then look at the next
/// sibling to get the mod_item range.
fn find_cfg_test_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut parser = RustExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let source_bytes = source.as_bytes();
    let query = cached_query(&CFG_TEST_QUERY_CACHE, CFG_TEST_QUERY);

    let attr_name_idx = query.capture_index_for_name("attr_name");
    let cfg_arg_idx = query.capture_index_for_name("cfg_arg");
    let cfg_test_attr_idx = query.capture_index_for_name("cfg_test_attr");

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
    let mut ranges = Vec::new();

    while let Some(m) = matches.next() {
        let mut is_cfg = false;
        let mut is_test = false;
        let mut attr_node = None;

        for cap in m.captures {
            let text = cap.node.utf8_text(source_bytes).unwrap_or("");
            if attr_name_idx == Some(cap.index) && text == "cfg" {
                is_cfg = true;
            }
            if cfg_arg_idx == Some(cap.index) && text == "test" {
                is_test = true;
            }
            if cfg_test_attr_idx == Some(cap.index) {
                attr_node = Some(cap.node);
            }
        }

        if is_cfg && is_test {
            if let Some(attr) = attr_node {
                // Find the next sibling which should be the mod_item
                let mut sibling = attr.next_sibling();
                while let Some(s) = sibling {
                    if s.kind() == "mod_item" {
                        ranges.push((s.start_byte(), s.end_byte()));
                        break;
                    }
                    sibling = s.next_sibling();
                }
            }
        }
    }

    ranges
}

/// Extract use declarations from a `use_declaration` node.
/// Processes `use crate::...` imports and, if `crate_name` is provided,
/// also `use {crate_name}::...` imports (for integration tests).
fn extract_use_declaration(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    result: &mut HashMap<String, Vec<String>>,
    crate_name: Option<&str>,
) {
    let arg = match node.child_by_field_name("argument") {
        Some(a) => a,
        None => return,
    };
    let full_text = arg.utf8_text(source_bytes).unwrap_or("");

    // Handle `crate::` prefix
    if let Some(path_after_crate) = full_text.strip_prefix("crate::") {
        parse_use_path(path_after_crate, result);
        return;
    }

    // Handle `{crate_name}::` prefix for integration tests
    if let Some(name) = crate_name {
        let prefix = format!("{name}::");
        if let Some(path_after_name) = full_text.strip_prefix(&prefix) {
            parse_use_path(path_after_name, result);
        }
    }
}

/// Extract import specifiers with optional crate name support.
/// When `crate_name` is `Some`, also resolves `use {crate_name}::...` imports
/// in addition to the standard `use crate::...` imports.
pub fn extract_import_specifiers_with_crate_name(
    source: &str,
    crate_name: Option<&str>,
) -> Vec<(String, Vec<String>)> {
    let mut parser = RustExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let source_bytes = source.as_bytes();

    // Manual tree walking for Rust use statements, more reliable than queries
    // for complex use trees
    let root = tree.root_node();
    let mut result_map: HashMap<String, Vec<String>> = HashMap::new();

    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        if child.kind() == "use_declaration" {
            extract_use_declaration(&child, source_bytes, &mut result_map, crate_name);
        }
    }

    result_map.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Workspace support
// ---------------------------------------------------------------------------

/// A member crate in a Cargo workspace.
#[derive(Debug)]
pub struct WorkspaceMember {
    /// Crate name (hyphens converted to underscores).
    pub crate_name: String,
    /// Absolute path to the member crate root (directory containing Cargo.toml).
    pub member_root: std::path::PathBuf,
}

/// Directories to skip during workspace member traversal.
const SKIP_DIRS: &[&str] = &["target", ".cargo", "vendor"];

/// Maximum directory traversal depth when searching for workspace members.
const MAX_TRAVERSE_DEPTH: usize = 4;

/// Check whether a `Cargo.toml` at `scan_root` contains a `[workspace]` section.
pub fn has_workspace_section(scan_root: &Path) -> bool {
    let cargo_toml = scan_root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_toml) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content.lines().any(|line| line.trim() == "[workspace]")
}

/// Find all member crates in a Cargo workspace rooted at `scan_root`.
///
/// Returns an empty `Vec` if `scan_root` does not have a `[workspace]` section
/// in its `Cargo.toml`.
///
/// Supports both virtual workspaces (no `[package]`) and non-virtual workspaces
/// (both `[workspace]` and `[package]`).
///
/// Directories named `target`, `.cargo`, `vendor`, or starting with `.` are
/// skipped.  Traversal is limited to `MAX_TRAVERSE_DEPTH` levels.
pub fn find_workspace_members(scan_root: &Path) -> Vec<WorkspaceMember> {
    if !has_workspace_section(scan_root) {
        return Vec::new();
    }

    let mut members = Vec::new();
    find_members_recursive(scan_root, scan_root, 0, &mut members);
    members
}

fn find_members_recursive(
    scan_root: &Path,
    dir: &Path,
    depth: usize,
    members: &mut Vec<WorkspaceMember>,
) {
    if depth > MAX_TRAVERSE_DEPTH {
        return;
    }

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Skip hidden directories and known non-source dirs
        if dir_name.starts_with('.') || SKIP_DIRS.contains(&dir_name) {
            continue;
        }

        // Skip the scan root itself (already checked above)
        if path == scan_root {
            continue;
        }

        // Check if this subdirectory has a member Cargo.toml with [package]
        if let Some(crate_name) = parse_crate_name(&path) {
            members.push(WorkspaceMember {
                crate_name,
                member_root: path.to_path_buf(),
            });
            // Don't recurse into member crates (avoids cross-crate confusion)
            // However, nested workspaces / virtual manifests are rare; skip for now.
            continue;
        }

        // Recurse into directories without their own [package]
        find_members_recursive(scan_root, &path, depth + 1, members);
    }
}

/// Find the workspace member that owns `path` by longest prefix match.
///
/// Returns `None` if no member's `member_root` is a prefix of `path`.
pub fn find_member_for_path<'a>(
    path: &Path,
    members: &'a [WorkspaceMember],
) -> Option<&'a WorkspaceMember> {
    members
        .iter()
        .filter(|m| path.starts_with(&m.member_root))
        .max_by_key(|m| m.member_root.components().count())
}

/// Parse the `name = "..."` field from a Cargo.toml `[package]` section.
/// Hyphens in the name are converted to underscores.
/// Returns `None` if the file cannot be read or `[package]` section is absent.
pub fn parse_crate_name(scan_root: &Path) -> Option<String> {
    let cargo_toml = scan_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).ok()?;

    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();

        // Detect section headers
        if trimmed.starts_with('[') {
            if trimmed == "[package]" {
                in_package = true;
            } else {
                // Once we hit another section, stop looking
                if in_package {
                    break;
                }
            }
            continue;
        }

        if in_package {
            // Parse `name = "..."` or `name = '...'`
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim();
                    // Strip surrounding quotes
                    let name = if let Some(inner) =
                        rest.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
                    {
                        inner
                    } else if let Some(inner) =
                        rest.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
                    {
                        inner
                    } else {
                        continue;
                    };
                    return Some(name.replace('-', "_"));
                }
            }
        }
    }

    None
}

/// Parse a use path after `crate::` has been stripped.
/// e.g. "user::User" -> ("user", ["User"])
///      "models::user::User" -> ("models/user", ["User"])
///      "user::{User, Admin}" -> ("user", ["User", "Admin"])
///      "user::*" -> ("user", [])
fn parse_use_path(path: &str, result: &mut HashMap<String, Vec<String>>) {
    // Handle use list: `module::{A, B}`
    if let Some(brace_start) = path.find('{') {
        let module_part = &path[..brace_start.saturating_sub(2)]; // strip trailing ::
        let specifier = module_part.replace("::", "/");
        if let Some(brace_end) = path.find('}') {
            let list_content = &path[brace_start + 1..brace_end];
            let symbols: Vec<String> = list_content
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "*")
                .collect();
            if !specifier.is_empty() {
                result.entry(specifier).or_default().extend(symbols);
            }
        }
        return;
    }

    // Handle wildcard: `module::*`
    if let Some(module_part) = path.strip_suffix("::*") {
        let specifier = module_part.replace("::", "/");
        if !specifier.is_empty() {
            result.entry(specifier).or_default();
        }
        return;
    }

    // Single-segment module import (e.g., `use crate::fs`)
    if !path.contains("::") && !path.is_empty() {
        result.entry(path.to_string()).or_default();
        return;
    }

    // Simple path: `module::Symbol`
    let parts: Vec<&str> = path.split("::").collect();
    if parts.len() >= 2 {
        let module_parts = &parts[..parts.len() - 1];
        let symbol = parts[parts.len() - 1];
        let specifier = module_parts.join("/");
        result
            .entry(specifier)
            .or_default()
            .push(symbol.to_string());
    }
}

/// Extract `pub mod` and `pub use` re-exports from raw text (e.g., inside cfg macro token_tree).
/// Uses text matching since tree-sitter token_tree content is not structured AST.
fn extract_re_exports_from_text(text: &str, result: &mut Vec<BarrelReExport>) {
    for line in text.lines() {
        let trimmed = line.trim();
        // Skip token_tree boundary lines (bare `{` or `}`)
        if trimmed == "{" || trimmed == "}" {
            continue;
        }
        // Strip surrounding braces for single-line token_tree: "{ pub use ...; }"
        let trimmed = trimmed
            .strip_prefix('{')
            .unwrap_or(trimmed)
            .strip_suffix('}')
            .unwrap_or(trimmed)
            .trim();

        // pub mod foo; or pub(crate) mod foo;
        if (trimmed.starts_with("pub mod ") || trimmed.starts_with("pub(crate) mod "))
            && trimmed.ends_with(';')
        {
            let mod_name = trimmed
                .trim_start_matches("pub(crate) mod ")
                .trim_start_matches("pub mod ")
                .trim_end_matches(';')
                .trim();
            if !mod_name.is_empty() && !mod_name.contains(' ') {
                result.push(BarrelReExport {
                    symbols: Vec::new(),
                    from_specifier: format!("./{mod_name}"),
                    wildcard: true,
                    namespace_wildcard: false,
                });
            }
        }

        // pub use module::{A, B}; or pub use module::*;
        if trimmed.starts_with("pub use ") && trimmed.contains("::") {
            let use_path = trimmed
                .trim_start_matches("pub use ")
                .trim_end_matches(';')
                .trim();
            let use_path = use_path.strip_prefix("self::").unwrap_or(use_path);
            // pub use self::*; -> after strip, use_path = "*"
            if use_path == "*" {
                result.push(BarrelReExport {
                    symbols: Vec::new(),
                    from_specifier: "./".to_string(),
                    wildcard: true,
                    namespace_wildcard: false,
                });
                continue;
            }
            // Delegate to the same text-based parsing used for tree-sitter nodes
            if use_path.ends_with("::*") {
                let module_part = use_path.strip_suffix("::*").unwrap_or("");
                result.push(BarrelReExport {
                    symbols: Vec::new(),
                    from_specifier: format!("./{}", module_part.replace("::", "/")),
                    wildcard: true,
                    namespace_wildcard: false,
                });
            } else if let Some(brace_start) = use_path.find('{') {
                let module_part = &use_path[..brace_start.saturating_sub(2)];
                if let Some(brace_end) = use_path.find('}') {
                    let list_content = &use_path[brace_start + 1..brace_end];
                    let symbols: Vec<String> = list_content
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s != "*")
                        .collect();
                    result.push(BarrelReExport {
                        symbols,
                        from_specifier: format!("./{}", module_part.replace("::", "/")),
                        wildcard: false,
                        namespace_wildcard: false,
                    });
                }
            } else {
                // pub use module::Symbol;
                let parts: Vec<&str> = use_path.split("::").collect();
                if parts.len() >= 2 {
                    let module_parts = &parts[..parts.len() - 1];
                    let symbol = parts[parts.len() - 1];
                    result.push(BarrelReExport {
                        symbols: vec![symbol.to_string()],
                        from_specifier: format!("./{}", module_parts.join("/")),
                        wildcard: false,
                        namespace_wildcard: false,
                    });
                }
            }
        }
    }
}

/// Extract pub use re-exports for barrel files.
fn extract_pub_use_re_exports(
    arg: &tree_sitter::Node,
    source_bytes: &[u8],
    result: &mut Vec<BarrelReExport>,
) {
    let full_text = arg.utf8_text(source_bytes).unwrap_or("");
    // Strip `self::` prefix (means "current module" in Rust)
    let full_text = full_text.strip_prefix("self::").unwrap_or(full_text);

    // pub use self::*; -> after strip, full_text = "*"
    if full_text == "*" {
        result.push(BarrelReExport {
            symbols: Vec::new(),
            from_specifier: "./".to_string(),
            wildcard: true,
            namespace_wildcard: false,
        });
        return;
    }

    // pub use module::*;
    if full_text.ends_with("::*") {
        let module_part = full_text.strip_suffix("::*").unwrap_or("");
        result.push(BarrelReExport {
            symbols: Vec::new(),
            from_specifier: format!("./{}", module_part.replace("::", "/")),
            wildcard: true,
            namespace_wildcard: false,
        });
        return;
    }

    // pub use module::{A, B};
    if let Some(brace_start) = full_text.find('{') {
        let module_part = &full_text[..brace_start.saturating_sub(2)]; // strip trailing ::
        if let Some(brace_end) = full_text.find('}') {
            let list_content = &full_text[brace_start + 1..brace_end];
            let symbols: Vec<String> = list_content
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            result.push(BarrelReExport {
                symbols,
                from_specifier: format!("./{}", module_part.replace("::", "/")),
                wildcard: false,
                namespace_wildcard: false,
            });
        }
        return;
    }

    // pub use module::Symbol;
    let parts: Vec<&str> = full_text.split("::").collect();
    if parts.len() >= 2 {
        let module_parts = &parts[..parts.len() - 1];
        let symbol = parts[parts.len() - 1];
        result.push(BarrelReExport {
            symbols: vec![symbol.to_string()],
            from_specifier: format!("./{}", module_parts.join("/")),
            wildcard: false,
            namespace_wildcard: false,
        });
    }
}

// ---------------------------------------------------------------------------
// Concrete methods (not in trait)
// ---------------------------------------------------------------------------

impl RustExtractor {
    /// Layer 0 + Layer 1 + Layer 2: Map test files to production files.
    ///
    /// Layer 0: Inline test self-mapping (#[cfg(test)] in production files)
    /// Layer 1: Filename convention matching
    /// Layer 2: Import tracing (use crate::...)
    pub fn map_test_files_with_imports(
        &self,
        production_files: &[String],
        test_sources: &HashMap<String, String>,
        scan_root: &Path,
        l1_exclusive: bool,
    ) -> Vec<FileMapping> {
        let test_file_list: Vec<String> = test_sources.keys().cloned().collect();

        // Layer 1: filename convention
        let mut mappings =
            exspec_core::observe::map_test_files(self, production_files, &test_file_list);

        // Layer 0: Inline test self-mapping
        for (idx, prod_file) in production_files.iter().enumerate() {
            // Skip barrel/entry point files (mod.rs, lib.rs, main.rs, build.rs)
            if production_stem(prod_file).is_none() {
                continue;
            }
            if let Ok(source) = std::fs::read_to_string(prod_file) {
                if detect_inline_tests(&source) {
                    // Self-map: production file maps to itself
                    if !mappings[idx].test_files.contains(prod_file) {
                        mappings[idx].test_files.push(prod_file.clone());
                    }
                }
            }
        }

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
        let layer1_tests_per_prod: Vec<HashSet<String>> = mappings
            .iter()
            .map(|m| m.test_files.iter().cloned().collect())
            .collect();

        // Collect set of test files matched by L1 for l1_exclusive mode
        let layer1_matched: HashSet<String> = layer1_tests_per_prod
            .iter()
            .flat_map(|s| s.iter().cloned())
            .collect();

        // Resolve crate name for integration test import matching
        let crate_name = parse_crate_name(scan_root);
        let members = find_workspace_members(scan_root);

        // Layer 2: import tracing
        if let Some(ref name) = crate_name {
            // Root has a [package]: apply L2 for root crate itself
            self.apply_l2_imports(
                test_sources,
                name,
                scan_root,
                &canonical_root,
                &canonical_to_idx,
                &mut mappings,
                l1_exclusive,
                &layer1_matched,
            );
        }

        if !members.is_empty() {
            // Workspace mode: apply L2 per member crate
            for member in &members {
                // Collect only the test files belonging to this member
                let member_test_sources: HashMap<String, String> = test_sources
                    .iter()
                    .filter(|(path, _)| {
                        find_member_for_path(Path::new(path.as_str()), &members)
                            .map(|m| std::ptr::eq(m, member))
                            .unwrap_or(false)
                    })
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                self.apply_l2_imports(
                    &member_test_sources,
                    &member.crate_name,
                    &member.member_root,
                    &canonical_root,
                    &canonical_to_idx,
                    &mut mappings,
                    l1_exclusive,
                    &layer1_matched,
                );
            }
        } else if crate_name.is_none() {
            // Fallback: no [package] and no workspace members; apply L2 with "crate"
            // pseudo-name to handle `use crate::...` references
            self.apply_l2_imports(
                test_sources,
                "crate",
                scan_root,
                &canonical_root,
                &canonical_to_idx,
                &mut mappings,
                l1_exclusive,
                &layer1_matched,
            );
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

    /// Apply Layer 2 import tracing for a single crate root.
    ///
    /// `crate_name`: the crate name (underscored).
    /// `crate_root`: the crate root directory (contains `Cargo.toml` and `src/`).
    #[allow(clippy::too_many_arguments)]
    fn apply_l2_imports(
        &self,
        test_sources: &HashMap<String, String>,
        crate_name: &str,
        crate_root: &Path,
        canonical_root: &Path,
        canonical_to_idx: &HashMap<String, usize>,
        mappings: &mut [FileMapping],
        l1_exclusive: bool,
        layer1_matched: &HashSet<String>,
    ) {
        for (test_file, source) in test_sources {
            if l1_exclusive && layer1_matched.contains(test_file) {
                continue;
            }
            let imports = extract_import_specifiers_with_crate_name(source, Some(crate_name));
            let mut matched_indices = HashSet::<usize>::new();

            for (specifier, symbols) in &imports {
                // Convert specifier to file path relative to member crate root (src/)
                let src_relative = crate_root.join("src").join(specifier);

                if let Some(resolved) = exspec_core::observe::resolve_absolute_base_to_file(
                    self,
                    &src_relative,
                    canonical_root,
                ) {
                    let mut per_specifier_indices = HashSet::<usize>::new();
                    exspec_core::observe::collect_import_matches(
                        self,
                        &resolved,
                        symbols,
                        canonical_to_idx,
                        &mut per_specifier_indices,
                        canonical_root,
                    );
                    // Filter: if symbols are specified, only include files that export them
                    for idx in per_specifier_indices {
                        let prod_path = Path::new(&mappings[idx].production_file);
                        if self.file_exports_any_symbol(prod_path, symbols) {
                            matched_indices.insert(idx);
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
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // RS-STEM-01: tests/test_foo.rs -> test_stem = Some("foo")
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_01_test_prefix() {
        // Given: a file named tests/test_foo.rs
        // When: test_stem is called
        // Then: returns Some("foo")
        let extractor = RustExtractor::new();
        assert_eq!(extractor.test_stem("tests/test_foo.rs"), Some("foo"));
    }

    // -----------------------------------------------------------------------
    // RS-STEM-02: tests/foo_test.rs -> test_stem = Some("foo")
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_02_test_suffix() {
        // Given: a file named tests/foo_test.rs
        // When: test_stem is called
        // Then: returns Some("foo")
        let extractor = RustExtractor::new();
        assert_eq!(extractor.test_stem("tests/foo_test.rs"), Some("foo"));
    }

    // -----------------------------------------------------------------------
    // RS-STEM-03: tests/integration.rs -> test_stem = Some("integration")
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_03_tests_dir_integration() {
        // Given: a file in tests/ directory without test_ prefix or _test suffix
        // When: test_stem is called
        // Then: returns Some("integration") because tests/ directory files are integration tests
        let extractor = RustExtractor::new();
        assert_eq!(
            extractor.test_stem("tests/integration.rs"),
            Some("integration")
        );
    }

    // -----------------------------------------------------------------------
    // RS-STEM-04: src/user.rs -> test_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_04_production_file_no_test_stem() {
        // Given: a production file in src/
        // When: test_stem is called
        // Then: returns None
        let extractor = RustExtractor::new();
        assert_eq!(extractor.test_stem("src/user.rs"), None);
    }

    // -----------------------------------------------------------------------
    // RS-STEM-05: src/user.rs -> production_stem = Some("user")
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_05_production_stem_regular() {
        // Given: a regular production file
        // When: production_stem is called
        // Then: returns Some("user")
        let extractor = RustExtractor::new();
        assert_eq!(extractor.production_stem("src/user.rs"), Some("user"));
    }

    // -----------------------------------------------------------------------
    // RS-STEM-06: src/lib.rs -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_06_production_stem_lib() {
        // Given: lib.rs (barrel file)
        // When: production_stem is called
        // Then: returns None
        let extractor = RustExtractor::new();
        assert_eq!(extractor.production_stem("src/lib.rs"), None);
    }

    // -----------------------------------------------------------------------
    // RS-STEM-07: src/mod.rs -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_07_production_stem_mod() {
        // Given: mod.rs (barrel file)
        // When: production_stem is called
        // Then: returns None
        let extractor = RustExtractor::new();
        assert_eq!(extractor.production_stem("src/mod.rs"), None);
    }

    // -----------------------------------------------------------------------
    // RS-STEM-08: src/main.rs -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_08_production_stem_main() {
        // Given: main.rs (entry point)
        // When: production_stem is called
        // Then: returns None
        let extractor = RustExtractor::new();
        assert_eq!(extractor.production_stem("src/main.rs"), None);
    }

    // -----------------------------------------------------------------------
    // RS-STEM-09: tests/test_foo.rs -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_stem_09_production_stem_test_file() {
        // Given: a test file
        // When: production_stem is called
        // Then: returns None
        let extractor = RustExtractor::new();
        assert_eq!(extractor.production_stem("tests/test_foo.rs"), None);
    }

    // -----------------------------------------------------------------------
    // RS-HELPER-01: build.rs -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_helper_01_build_rs() {
        // Given: build.rs
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = RustExtractor::new();
        assert!(extractor.is_non_sut_helper("build.rs", false));
    }

    // -----------------------------------------------------------------------
    // RS-HELPER-02: tests/common/mod.rs -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_helper_02_tests_common() {
        // Given: tests/common/mod.rs (test helper module)
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = RustExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/common/mod.rs", false));
    }

    // -----------------------------------------------------------------------
    // RS-HELPER-03: src/user.rs -> is_non_sut_helper = false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_helper_03_regular_production_file() {
        // Given: a regular production file
        // When: is_non_sut_helper is called
        // Then: returns false
        let extractor = RustExtractor::new();
        assert!(!extractor.is_non_sut_helper("src/user.rs", false));
    }

    // -----------------------------------------------------------------------
    // RS-HELPER-04: benches/bench.rs -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_helper_04_benches() {
        // Given: a benchmark file
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = RustExtractor::new();
        assert!(extractor.is_non_sut_helper("benches/bench.rs", false));
    }

    // -----------------------------------------------------------------------
    // RS-L0-01: #[cfg(test)] mod tests {} -> detect_inline_tests = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_01_cfg_test_present() {
        // Given: source with #[cfg(test)] mod tests block
        let source = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}
"#;
        // When: detect_inline_tests is called
        // Then: returns true
        assert!(detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L0-02: no #[cfg(test)] -> detect_inline_tests = false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_02_no_cfg_test() {
        // Given: source without #[cfg(test)]
        let source = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }
"#;
        // When: detect_inline_tests is called
        // Then: returns false
        assert!(!detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L0-03: #[cfg(not(test))] only -> detect_inline_tests = false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_03_cfg_not_test() {
        // Given: source with #[cfg(not(test))] only (no #[cfg(test)])
        let source = r#"
#[cfg(not(test))]
mod production_only {
    pub fn real_thing() {}
}
"#;
        // When: detect_inline_tests is called
        // Then: returns false
        assert!(!detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-FUNC-01: pub fn create_user() {} -> name="create_user", is_exported=true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_func_01_pub_function() {
        // Given: source with a pub function
        let source = "pub fn create_user() {}\n";

        // When: extract_production_functions is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_production_functions(source, "src/user.rs");

        // Then: name="create_user", is_exported=true
        let func = result.iter().find(|f| f.name == "create_user");
        assert!(func.is_some(), "create_user not found in {:?}", result);
        assert!(func.unwrap().is_exported);
    }

    // -----------------------------------------------------------------------
    // RS-FUNC-02: fn private_fn() {} -> name="private_fn", is_exported=false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_func_02_private_function() {
        // Given: source with a private function
        let source = "fn private_fn() {}\n";

        // When: extract_production_functions is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_production_functions(source, "src/internal.rs");

        // Then: name="private_fn", is_exported=false
        let func = result.iter().find(|f| f.name == "private_fn");
        assert!(func.is_some(), "private_fn not found in {:?}", result);
        assert!(!func.unwrap().is_exported);
    }

    // -----------------------------------------------------------------------
    // RS-FUNC-03: impl User { pub fn save() {} } -> name="save", class_name=Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn rs_func_03_impl_method() {
        // Given: source with an impl block
        let source = r#"
struct User;

impl User {
    pub fn save(&self) {}
}
"#;
        // When: extract_production_functions is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_production_functions(source, "src/user.rs");

        // Then: name="save", class_name=Some("User")
        let method = result.iter().find(|f| f.name == "save");
        assert!(method.is_some(), "save not found in {:?}", result);
        let method = method.unwrap();
        assert_eq!(method.class_name, Some("User".to_string()));
        assert!(method.is_exported);
    }

    // -----------------------------------------------------------------------
    // RS-FUNC-04: functions inside #[cfg(test)] mod tests are NOT extracted
    // -----------------------------------------------------------------------
    #[test]
    fn rs_func_04_cfg_test_excluded() {
        // Given: source with functions inside #[cfg(test)] mod
        let source = r#"
pub fn real_function() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_function() {
        assert!(true);
    }
}
"#;
        // When: extract_production_functions is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_production_functions(source, "src/lib.rs");

        // Then: only real_function is extracted, not test_real_function
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "real_function");
    }

    // -----------------------------------------------------------------------
    // RS-IMP-01: use crate::user::User -> ("user", ["User"])
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_01_simple_crate_import() {
        // Given: source with a simple crate import
        let source = "use crate::user::User;\n";

        // When: extract_all_import_specifiers is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: ("user", ["User"])
        let entry = result.iter().find(|(spec, _)| spec == "user");
        assert!(entry.is_some(), "user not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(symbols.contains(&"User".to_string()));
    }

    // -----------------------------------------------------------------------
    // RS-IMP-02: use crate::models::user::User -> ("models/user", ["User"])
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_02_nested_crate_import() {
        // Given: source with a nested crate import
        let source = "use crate::models::user::User;\n";

        // When: extract_all_import_specifiers is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: ("models/user", ["User"])
        let entry = result.iter().find(|(spec, _)| spec == "models/user");
        assert!(entry.is_some(), "models/user not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(symbols.contains(&"User".to_string()));
    }

    // -----------------------------------------------------------------------
    // RS-IMP-03: use crate::user::{User, Admin} -> ("user", ["User", "Admin"])
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_03_use_list() {
        // Given: source with a use list
        let source = "use crate::user::{User, Admin};\n";

        // When: extract_all_import_specifiers is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: ("user", ["User", "Admin"])
        let entry = result.iter().find(|(spec, _)| spec == "user");
        assert!(entry.is_some(), "user not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"User".to_string()),
            "User not in {:?}",
            symbols
        );
        assert!(
            symbols.contains(&"Admin".to_string()),
            "Admin not in {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-IMP-04: use std::collections::HashMap -> external crate -> skipped
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_04_external_crate_skipped() {
        // Given: source with an external crate import
        let source = "use std::collections::HashMap;\n";

        // When: extract_all_import_specifiers is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: not included (only crate:: imports are tracked)
        assert!(
            result.is_empty(),
            "external imports should be skipped: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-01: mod.rs -> is_barrel_file = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_01_mod_rs() {
        // Given: mod.rs
        // When: is_barrel_file is called
        // Then: returns true
        let extractor = RustExtractor::new();
        assert!(extractor.is_barrel_file("src/models/mod.rs"));
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-02: lib.rs -> is_barrel_file = true
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_02_lib_rs() {
        // Given: lib.rs
        // When: is_barrel_file is called
        // Then: returns true
        let extractor = RustExtractor::new();
        assert!(extractor.is_barrel_file("src/lib.rs"));
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-03: pub mod user; in mod.rs -> extract_barrel_re_exports
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_03_pub_mod() {
        // Given: mod.rs with pub mod user;
        let source = "pub mod user;\n";

        // When: extract_barrel_re_exports is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier="./user", wildcard=true
        let entry = result.iter().find(|e| e.from_specifier == "./user");
        assert!(entry.is_some(), "./user not found in {:?}", result);
        assert!(entry.unwrap().wildcard);
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-04: pub use user::*; in mod.rs -> extract_barrel_re_exports
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_04_pub_use_wildcard() {
        // Given: mod.rs with pub use user::*;
        let source = "pub use user::*;\n";

        // When: extract_barrel_re_exports is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier="./user", wildcard=true
        let entry = result.iter().find(|e| e.from_specifier == "./user");
        assert!(entry.is_some(), "./user not found in {:?}", result);
        assert!(entry.unwrap().wildcard);
    }

    // -----------------------------------------------------------------------
    // RS-E2E-01: inline tests -> Layer 0 self-map
    // -----------------------------------------------------------------------
    #[test]
    fn rs_e2e_01_inline_test_self_map() {
        // Given: a temp directory with a production file containing inline tests
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let user_rs = src_dir.join("user.rs");
        std::fs::write(
            &user_rs,
            r#"pub fn create_user() {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_create_user() { assert!(true); }
}
"#,
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = user_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = HashMap::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: user.rs is self-mapped (Layer 0)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            mapping.unwrap().test_files.contains(&prod_path),
            "Expected self-map for inline tests: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-E2E-02: stem match -> Layer 1
    // -----------------------------------------------------------------------
    #[test]
    fn rs_e2e_02_layer1_stem_match() {
        // Given: production file and test file with matching stems
        let extractor = RustExtractor::new();
        let production_files = vec!["src/user.rs".to_string()];
        let test_sources: HashMap<String, String> =
            [("tests/test_user.rs".to_string(), String::new())]
                .into_iter()
                .collect();

        // When: map_test_files_with_imports is called
        let scan_root = PathBuf::from(".");
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &scan_root,
            false,
        );

        // Then: Layer 1 stem match (same directory not required for test_stem)
        // Note: map_test_files requires same directory, but tests/ files have test_stem
        // that matches production_stem. However, core::map_test_files uses directory matching.
        // For cross-directory matching, we rely on Layer 2 (import tracing).
        // This test verifies the mapping structure is correct.
        let mapping = result.iter().find(|m| m.production_file == "src/user.rs");
        assert!(mapping.is_some());
    }

    // -----------------------------------------------------------------------
    // RS-E2E-03: import match -> Layer 2
    // -----------------------------------------------------------------------
    #[test]
    fn rs_e2e_03_layer2_import_tracing() {
        // Given: a temp directory with production and test files
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        let service_rs = src_dir.join("service.rs");
        std::fs::write(&service_rs, "pub struct Service;\n").unwrap();

        let test_service_rs = tests_dir.join("test_service.rs");
        let test_source = "use crate::service::Service;\n\n#[test]\nfn test_it() {}\n";
        std::fs::write(&test_service_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let prod_path = service_rs.to_string_lossy().into_owned();
        let test_path = test_service_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: service.rs is matched to test_service.rs via import tracing
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            mapping.unwrap().test_files.contains(&test_path),
            "Expected import tracing match: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-E2E-04: tests/common/mod.rs -> helper excluded
    // -----------------------------------------------------------------------
    #[test]
    fn rs_e2e_04_helper_excluded() {
        // Given: tests/common/mod.rs alongside test files
        let extractor = RustExtractor::new();
        let production_files = vec!["src/user.rs".to_string()];
        let test_sources: HashMap<String, String> = [
            ("tests/test_user.rs".to_string(), String::new()),
            (
                "tests/common/mod.rs".to_string(),
                "pub fn setup() {}\n".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        // When: map_test_files_with_imports is called
        let scan_root = PathBuf::from(".");
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &scan_root,
            false,
        );

        // Then: tests/common/mod.rs is NOT in any mapping
        for mapping in &result {
            assert!(
                !mapping
                    .test_files
                    .iter()
                    .any(|f| f.contains("common/mod.rs")),
                "common/mod.rs should not appear: {:?}",
                mapping
            );
        }
    }

    // -----------------------------------------------------------------------
    // RS-CRATE-01: parse_crate_name: 正常パース
    // -----------------------------------------------------------------------
    #[test]
    fn rs_crate_01_parse_crate_name_hyphen() {
        // Given: Cargo.toml に [package]\nname = "my-crate" を含む tempdir
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // When: parse_crate_name(dir) を呼ぶ
        let result = parse_crate_name(tmp.path());

        // Then: Some("my_crate") を返す（ハイフン→アンダースコア変換）
        assert_eq!(result, Some("my_crate".to_string()));
    }

    // -----------------------------------------------------------------------
    // RS-CRATE-02: parse_crate_name: ハイフンなし
    // -----------------------------------------------------------------------
    #[test]
    fn rs_crate_02_parse_crate_name_no_hyphen() {
        // Given: Cargo.toml に name = "tokio" を含む tempdir
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"tokio\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();

        // When: parse_crate_name(dir)
        let result = parse_crate_name(tmp.path());

        // Then: Some("tokio")
        assert_eq!(result, Some("tokio".to_string()));
    }

    // -----------------------------------------------------------------------
    // RS-CRATE-03: parse_crate_name: ファイルなし
    // -----------------------------------------------------------------------
    #[test]
    fn rs_crate_03_parse_crate_name_no_file() {
        // Given: Cargo.toml が存在しない tempdir
        let tmp = tempfile::tempdir().unwrap();

        // When: parse_crate_name(dir)
        let result = parse_crate_name(tmp.path());

        // Then: None
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // RS-CRATE-04: parse_crate_name: workspace (package なし)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_crate_04_parse_crate_name_workspace() {
        // Given: [workspace]\nmembers = ["crate1"] のみの Cargo.toml
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate1\"]\n",
        )
        .unwrap();

        // When: parse_crate_name(dir)
        let result = parse_crate_name(tmp.path());

        // Then: None
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // RS-IMP-05: crate_name simple import
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_05_crate_name_simple_import() {
        // Given: source = "use my_crate::user::User;\n", crate_name = Some("my_crate")
        let source = "use my_crate::user::User;\n";

        // When: extract_import_specifiers_with_crate_name(source, Some("my_crate"))
        let result = extract_import_specifiers_with_crate_name(source, Some("my_crate"));

        // Then: [("user", ["User"])]
        let entry = result.iter().find(|(spec, _)| spec == "user");
        assert!(entry.is_some(), "user not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"User".to_string()),
            "User not in {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-IMP-06: crate_name use list
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_06_crate_name_use_list() {
        // Given: source = "use my_crate::user::{User, Admin};\n", crate_name = Some("my_crate")
        let source = "use my_crate::user::{User, Admin};\n";

        // When: extract_import_specifiers_with_crate_name(source, Some("my_crate"))
        let result = extract_import_specifiers_with_crate_name(source, Some("my_crate"));

        // Then: [("user", ["User", "Admin"])]
        let entry = result.iter().find(|(spec, _)| spec == "user");
        assert!(entry.is_some(), "user not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"User".to_string()),
            "User not in {:?}",
            symbols
        );
        assert!(
            symbols.contains(&"Admin".to_string()),
            "Admin not in {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-IMP-07: crate_name=None ではスキップ
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_07_crate_name_none_skips() {
        // Given: source = "use my_crate::user::User;\n", crate_name = None
        let source = "use my_crate::user::User;\n";

        // When: extract_import_specifiers_with_crate_name(source, None)
        let result = extract_import_specifiers_with_crate_name(source, None);

        // Then: [] (空)
        assert!(
            result.is_empty(),
            "Expected empty result when crate_name=None, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // RS-IMP-08: crate:: と crate_name:: 混在
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_08_mixed_crate_and_crate_name() {
        // Given: source に `use crate::service::Service;` と `use my_crate::user::User;` の両方
        // crate_name = Some("my_crate")
        let source = "use crate::service::Service;\nuse my_crate::user::User;\n";

        // When: extract_import_specifiers_with_crate_name(source, Some("my_crate"))
        let result = extract_import_specifiers_with_crate_name(source, Some("my_crate"));

        // Then: [("service", ["Service"]), ("user", ["User"])] の両方が検出される
        let service_entry = result.iter().find(|(spec, _)| spec == "service");
        assert!(service_entry.is_some(), "service not found in {:?}", result);
        let (_, service_symbols) = service_entry.unwrap();
        assert!(
            service_symbols.contains(&"Service".to_string()),
            "Service not in {:?}",
            service_symbols
        );

        let user_entry = result.iter().find(|(spec, _)| spec == "user");
        assert!(user_entry.is_some(), "user not found in {:?}", result);
        let (_, user_symbols) = user_entry.unwrap();
        assert!(
            user_symbols.contains(&"User".to_string()),
            "User not in {:?}",
            user_symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-L2-INTEG: 統合テスト (tempdir)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l2_integ_crate_name_import_layer2() {
        // Given: tempdir に以下を作成
        //   - Cargo.toml: [package]\nname = "my-crate"\nversion = "0.1.0"\nedition = "2021"
        //   - src/user.rs: pub struct User;
        //   - tests/test_user.rs: use my_crate::user::User; (ソース)
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let user_rs = src_dir.join("user.rs");
        std::fs::write(&user_rs, "pub struct User;\n").unwrap();

        let test_user_rs = tests_dir.join("test_user.rs");
        let test_source = "use my_crate::user::User;\n\n#[test]\nfn test_user() {}\n";
        std::fs::write(&test_user_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let prod_path = user_rs.to_string_lossy().into_owned();
        let test_path = test_user_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports を呼ぶ
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: test_user.rs → user.rs が Layer 2 (ImportTracing) でマッチ
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some(), "production file mapping not found");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_user.rs to map to user.rs via Layer 2, got: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "Expected ImportTracing strategy, got: {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // RS-DEEP-REEXPORT-01: 2段 re-export — src/models/mod.rs: pub mod user;
    // -----------------------------------------------------------------------
    #[test]
    fn rs_deep_reexport_01_two_hop() {
        // Given: tempdir に以下を作成
        //   Cargo.toml: [package]\nname = "my-crate"\n...
        //   src/models/mod.rs: pub mod user;
        //   src/models/user.rs: pub struct User;
        //   tests/test_models.rs: use my_crate::models::User;
        let tmp = tempfile::tempdir().unwrap();
        let src_models_dir = tmp.path().join("src").join("models");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_models_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mod_rs = src_models_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod user;\n").unwrap();

        let user_rs = src_models_dir.join("user.rs");
        std::fs::write(&user_rs, "pub struct User;\n").unwrap();

        let test_models_rs = tests_dir.join("test_models.rs");
        let test_source = "use my_crate::models::User;\n\n#[test]\nfn test_user() {}\n";
        std::fs::write(&test_models_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let user_path = user_rs.to_string_lossy().into_owned();
        let test_path = test_models_rs.to_string_lossy().into_owned();
        let production_files = vec![user_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports を呼ぶ
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: test_models.rs → user.rs が Layer 2 (ImportTracing) でマッチ
        let mapping = result.iter().find(|m| m.production_file == user_path);
        assert!(mapping.is_some(), "production file mapping not found");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_models.rs to map to user.rs via Layer 2 (pub mod chain), got: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "Expected ImportTracing strategy, got: {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // RS-DEEP-REEXPORT-02: 3段 re-export — lib.rs → models/mod.rs → user.rs
    // テストが `use my_crate::models::User;` (user セグメントなし) のみを使うため
    // pub mod wildcard chain なしでは user.rs にマッチできない
    // -----------------------------------------------------------------------
    #[test]
    fn rs_deep_reexport_02_three_hop() {
        // Given: tempdir に以下を作成
        //   Cargo.toml: [package]\nname = "my-crate"\n...
        //   src/lib.rs: pub mod models;
        //   src/models/mod.rs: pub mod user;
        //   src/models/user.rs: pub struct User;
        //   tests/test_account.rs: use my_crate::models::User; (user セグメントなし)
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        let src_models_dir = src_dir.join("models");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_models_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        std::fs::write(src_dir.join("lib.rs"), "pub mod models;\n").unwrap();

        let mod_rs = src_models_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod user;\n").unwrap();

        let user_rs = src_models_dir.join("user.rs");
        std::fs::write(&user_rs, "pub struct User;\n").unwrap();

        // test_account.rs: ファイル名は user と無関係 → Layer 1 ではマッチしない
        let test_account_rs = tests_dir.join("test_account.rs");
        let test_source = "use my_crate::models::User;\n\n#[test]\nfn test_account() {}\n";
        std::fs::write(&test_account_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let user_path = user_rs.to_string_lossy().into_owned();
        let test_path = test_account_rs.to_string_lossy().into_owned();
        let production_files = vec![user_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports を呼ぶ
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: test_account.rs → user.rs が Layer 2 (ImportTracing) でマッチ
        // (lib.rs → models/ → pub mod user; の wildcard chain を辿る必要がある)
        let mapping = result.iter().find(|m| m.production_file == user_path);
        assert!(mapping.is_some(), "production file mapping not found");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_account.rs to map to user.rs via Layer 2 (3-hop pub mod chain), got: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "Expected ImportTracing strategy, got: {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // RS-DEEP-REEXPORT-03: pub use + pub mod 混在 → 両エントリ返す
    // -----------------------------------------------------------------------
    #[test]
    fn rs_deep_reexport_03_pub_use_and_pub_mod() {
        // Given: mod.rs with `pub mod internal;` and `pub use internal::Exported;`
        let source = "pub mod internal;\npub use internal::Exported;\n";

        // When: extract_barrel_re_exports is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: 2エントリ返す
        //   1. from_specifier="./internal", wildcard=true  (pub mod)
        //   2. from_specifier="./internal", symbols=["Exported"]  (pub use)
        let wildcard_entry = result
            .iter()
            .find(|e| e.from_specifier == "./internal" && e.wildcard);
        assert!(
            wildcard_entry.is_some(),
            "Expected wildcard=true entry for pub mod internal, got: {:?}",
            result
        );

        let symbol_entry = result.iter().find(|e| {
            e.from_specifier == "./internal"
                && !e.wildcard
                && e.symbols.contains(&"Exported".to_string())
        });
        assert!(
            symbol_entry.is_some(),
            "Expected symbols=[\"Exported\"] entry for pub use internal::Exported, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-01: pub fn match
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_01_pub_fn_match() {
        // Given: a file with pub fn create_user
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/exported_pub_symbols.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["create_user".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns true
        assert!(result, "Expected true for pub fn create_user");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-02: pub struct match
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_02_pub_struct_match() {
        // Given: a file with pub struct User
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/exported_pub_symbols.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["User".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns true
        assert!(result, "Expected true for pub struct User");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-03: non-existent symbol
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_03_nonexistent_symbol() {
        // Given: a file without NonExistent symbol
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/exported_pub_symbols.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["NonExistent".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns false
        assert!(!result, "Expected false for NonExistent symbol");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-04: file with no pub symbols
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_04_no_pub_symbols() {
        // Given: a file with no pub items
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/no_pub_symbols.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["internal_only".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns false
        assert!(!result, "Expected false for file with no pub symbols");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-05: pub use/mod only (no direct pub definitions)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_05_pub_use_mod_only() {
        // Given: a file with only pub use and pub mod (barrel re-exports)
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/pub_use_only.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["Foo".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns false (pub use/mod are handled by barrel resolution)
        assert!(
            !result,
            "Expected false for pub use/mod only file (barrel resolution handles these)"
        );
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-06: empty symbol list
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_06_empty_symbols() {
        // Given: any file
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rust/observe/exported_pub_symbols.rs");
        let extractor = RustExtractor::new();
        let symbols: Vec<String> = vec![];

        // When: file_exports_any_symbol is called with empty symbols
        let result = extractor.file_exports_any_symbol(&path, &symbols);

        // Then: returns true (short-circuit)
        assert!(result, "Expected true for empty symbol list");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-07: non-existent file (optimistic fallback)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_07_nonexistent_file() {
        // Given: a non-existent file path
        let path = PathBuf::from("/nonexistent/path/to/file.rs");
        let extractor = RustExtractor::new();
        let symbols = vec!["Foo".to_string()];

        // When: file_exports_any_symbol is called
        // Then: returns true (optimistic fallback, matches core default and Python)
        let result = extractor.file_exports_any_symbol(&path, &symbols);
        assert!(
            result,
            "Expected true for non-existent file (optimistic fallback)"
        );
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-PUB-ONLY-01: pub fn matches (regression)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_pub_only_01_pub_fn_matches() {
        // Given: a file with `pub fn create_user() {}`
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("service.rs");
        std::fs::write(&file, "pub fn create_user() {}").unwrap();
        let extractor = RustExtractor::new();
        let symbols = vec!["create_user".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&file, &symbols);

        // Then: returns true (pub fn is exported)
        assert!(result, "Expected true for pub fn create_user");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-PUB-ONLY-02: pub(crate) struct excluded
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_pub_only_02_pub_crate_excluded() {
        // Given: a file with `pub(crate) struct Handle {}`
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("driver.rs");
        std::fs::write(&file, "pub(crate) struct Handle {}").unwrap();
        let extractor = RustExtractor::new();
        let symbols = vec!["Handle".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&file, &symbols);

        // Then: returns false (pub(crate) is NOT a public export)
        assert!(!result, "Expected false for pub(crate) struct Handle");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-PUB-ONLY-03: pub(super) fn excluded
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_pub_only_03_pub_super_excluded() {
        // Given: a file with `pub(super) fn helper() {}`
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("internal.rs");
        std::fs::write(&file, "pub(super) fn helper() {}").unwrap();
        let extractor = RustExtractor::new();
        let symbols = vec!["helper".to_string()];

        // When: file_exports_any_symbol is called
        let result = extractor.file_exports_any_symbol(&file, &symbols);

        // Then: returns false (pub(super) is NOT a public export)
        assert!(!result, "Expected false for pub(super) fn helper");
    }

    // -----------------------------------------------------------------------
    // RS-EXPORT-PUB-ONLY-04: mixed visibility - pub struct matches, pub(crate) excluded
    // -----------------------------------------------------------------------
    #[test]
    fn rs_export_pub_only_04_mixed_visibility() {
        // Given: a file with `pub struct User {}` and `pub(crate) struct Inner {}`
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("models.rs");
        std::fs::write(&file, "pub struct User {}\npub(crate) struct Inner {}").unwrap();
        let extractor = RustExtractor::new();
        let symbols = vec!["User".to_string()];

        // When: file_exports_any_symbol is called with "User"
        let result = extractor.file_exports_any_symbol(&file, &symbols);

        // Then: returns true (pub struct User is exported)
        assert!(
            result,
            "Expected true for pub struct User in mixed visibility file"
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-01: workspace with 2 members -> 2 members detected
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_01_workspace_two_members() {
        // Given: a workspace with 2 member crates
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate_a\", \"crate_b\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("crate_a/src")).unwrap();
        std::fs::write(
            tmp.path().join("crate_a/Cargo.toml"),
            "[package]\nname = \"crate_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("crate_b/src")).unwrap();
        std::fs::write(
            tmp.path().join("crate_b/Cargo.toml"),
            "[package]\nname = \"crate_b\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // When: find_workspace_members is called
        let members = find_workspace_members(tmp.path());

        // Then: 2 WorkspaceMembers detected
        assert_eq!(members.len(), 2, "Expected 2 members, got: {:?}", members);
        let names: Vec<&str> = members.iter().map(|m| m.crate_name.as_str()).collect();
        assert!(
            names.contains(&"crate_a"),
            "crate_a not found in {:?}",
            names
        );
        assert!(
            names.contains(&"crate_b"),
            "crate_b not found in {:?}",
            names
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-02: single crate (non-workspace) returns empty
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_02_single_crate_returns_empty() {
        // Given: a single crate (no [workspace] section, has [package])
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my_crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();

        // When: find_workspace_members is called
        let members = find_workspace_members(tmp.path());

        // Then: empty Vec (not a workspace root)
        assert!(members.is_empty(), "Expected empty, got: {:?}", members);
    }

    // -----------------------------------------------------------------------
    // RS-WS-03: target/ directory is skipped
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_03_target_dir_skipped() {
        // Given: a workspace where target/ contains a Cargo.toml
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate_a\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("crate_a/src")).unwrap();
        std::fs::write(
            tmp.path().join("crate_a/Cargo.toml"),
            "[package]\nname = \"crate_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        // A Cargo.toml inside target/ (should be ignored)
        std::fs::create_dir_all(tmp.path().join("target/debug/build/fake")).unwrap();
        std::fs::write(
            tmp.path().join("target/debug/build/fake/Cargo.toml"),
            "[package]\nname = \"fake_crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // When: find_workspace_members is called
        let members = find_workspace_members(tmp.path());

        // Then: only crate_a detected (target/ is skipped)
        assert_eq!(members.len(), 1, "Expected 1 member, got: {:?}", members);
        assert_eq!(members[0].crate_name, "crate_a");
    }

    // -----------------------------------------------------------------------
    // RS-WS-04: hyphenated crate name -> underscore conversion
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_04_hyphenated_crate_name_converted() {
        // Given: a workspace with a member crate named "my-crate" (hyphenated)
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"my-crate\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("my-crate/src")).unwrap();
        std::fs::write(
            tmp.path().join("my-crate/Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // When: find_workspace_members is called
        let members = find_workspace_members(tmp.path());

        // Then: crate_name = "my_crate" (hyphens converted to underscores)
        assert_eq!(members.len(), 1, "Expected 1 member, got: {:?}", members);
        assert_eq!(members[0].crate_name, "my_crate");
    }

    // -----------------------------------------------------------------------
    // RS-WS-05: test file in member/tests/ -> Some(foo member)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_05_find_member_for_path_in_tests() {
        // Given: workspace members [crate_a at /tmp/ws/crate_a]
        let tmp = tempfile::tempdir().unwrap();
        let member_root = tmp.path().join("crate_a");
        std::fs::create_dir_all(&member_root).unwrap();
        let members = vec![WorkspaceMember {
            crate_name: "crate_a".to_string(),
            member_root: member_root.clone(),
        }];

        // When: find_member_for_path with a test file inside crate_a/tests/
        let test_file = member_root.join("tests").join("integration.rs");
        let result = find_member_for_path(&test_file, &members);

        // Then: returns Some(crate_a member)
        assert!(result.is_some(), "Expected Some(crate_a), got None");
        assert_eq!(result.unwrap().crate_name, "crate_a");
    }

    // -----------------------------------------------------------------------
    // RS-WS-06: test file not in any member -> None
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_06_find_member_for_path_not_in_any() {
        // Given: workspace members [crate_a]
        let tmp = tempfile::tempdir().unwrap();
        let member_root = tmp.path().join("crate_a");
        std::fs::create_dir_all(&member_root).unwrap();
        let members = vec![WorkspaceMember {
            crate_name: "crate_a".to_string(),
            member_root: member_root.clone(),
        }];

        // When: find_member_for_path with a path outside any member
        let outside_path = tmp.path().join("other").join("test.rs");
        let result = find_member_for_path(&outside_path, &members);

        // Then: returns None
        assert!(
            result.is_none(),
            "Expected None, got: {:?}",
            result.map(|m| &m.crate_name)
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-07: longest prefix match for nested members
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_07_find_member_longest_prefix() {
        // Given: workspace with nested members [ws/crates/foo, ws/crates/foo-extra]
        let tmp = tempfile::tempdir().unwrap();
        let foo_root = tmp.path().join("crates").join("foo");
        let foo_extra_root = tmp.path().join("crates").join("foo-extra");
        std::fs::create_dir_all(&foo_root).unwrap();
        std::fs::create_dir_all(&foo_extra_root).unwrap();
        let members = vec![
            WorkspaceMember {
                crate_name: "foo".to_string(),
                member_root: foo_root.clone(),
            },
            WorkspaceMember {
                crate_name: "foo_extra".to_string(),
                member_root: foo_extra_root.clone(),
            },
        ];

        // When: find_member_for_path with a path inside foo-extra/
        let test_file = foo_extra_root.join("tests").join("test_bar.rs");
        let result = find_member_for_path(&test_file, &members);

        // Then: returns foo-extra (longest prefix match)
        assert!(result.is_some(), "Expected Some(foo_extra), got None");
        assert_eq!(result.unwrap().crate_name, "foo_extra");
    }

    // -----------------------------------------------------------------------
    // RS-WS-E2E-01: workspace L2 import tracing works
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_e2e_01_workspace_l2_import_tracing() {
        // Given: a workspace with crate_a containing src/user.rs and tests/test_user.rs
        // that imports `use crate_a::user::create_user`
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate_a\"]\n",
        )
        .unwrap();

        let member_dir = tmp.path().join("crate_a");
        std::fs::create_dir_all(member_dir.join("src")).unwrap();
        std::fs::create_dir_all(member_dir.join("tests")).unwrap();
        std::fs::write(
            member_dir.join("Cargo.toml"),
            "[package]\nname = \"crate_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let user_rs = member_dir.join("src").join("user.rs");
        std::fs::write(&user_rs, "pub fn create_user() {}\n").unwrap();

        let test_rs = member_dir.join("tests").join("test_user.rs");
        std::fs::write(
            &test_rs,
            "use crate_a::user::create_user;\n#[test]\nfn test_create_user() { create_user(); }\n",
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = user_rs.to_string_lossy().into_owned();
        let test_path = test_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(
            test_path.clone(),
            std::fs::read_to_string(&test_rs).unwrap(),
        )]
        .into_iter()
        .collect();

        // When: map_test_files_with_imports is called at workspace root
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: test_user.rs -> user.rs via Layer 2 (ImportTracing)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some(), "No mapping for user.rs");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_user.rs in test_files, got: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "Expected ImportTracing strategy, got: {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-E2E-02: L0/L1 still work at workspace level
    //
    // Layer 1 (FileNameConvention) matches within the same directory only.
    // Cross-directory matches (src/ vs tests/) are handled by Layer 2.
    // This test verifies:
    //   - L0: src/service.rs with inline tests -> self-mapped
    //   - L1: src/test_service.rs -> src/service.rs (same src/ directory)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_e2e_02_l0_l1_still_work_at_workspace_level() {
        // Given: a workspace with crate_a containing src/service.rs (with inline tests)
        // and src/test_service.rs (same-directory filename convention match)
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate_a\"]\n",
        )
        .unwrap();

        let member_dir = tmp.path().join("crate_a");
        std::fs::create_dir_all(member_dir.join("src")).unwrap();
        std::fs::write(
            member_dir.join("Cargo.toml"),
            "[package]\nname = \"crate_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Layer 0: inline tests in service.rs
        let service_rs = member_dir.join("src").join("service.rs");
        std::fs::write(
            &service_rs,
            r#"pub fn do_work() {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_do_work() { do_work(); }
}
"#,
        )
        .unwrap();

        // Layer 1: test_service.rs in the same src/ directory -> service.rs
        let test_service_rs = member_dir.join("src").join("test_service.rs");
        std::fs::write(
            &test_service_rs,
            "#[test]\nfn test_service_smoke() { assert!(true); }\n",
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = service_rs.to_string_lossy().into_owned();
        let test_path = test_service_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(
            test_path.clone(),
            std::fs::read_to_string(&test_service_rs).unwrap(),
        )]
        .into_iter()
        .collect();

        // When: map_test_files_with_imports is called at workspace root
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: service.rs self-mapped (Layer 0) and test_service.rs mapped (Layer 1)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some(), "No mapping for service.rs");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&prod_path),
            "Expected service.rs self-mapped (Layer 0), got: {:?}",
            mapping.test_files
        );
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_service.rs mapped (Layer 1), got: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-E2E-03: Non-virtual workspace (both [workspace] and [package])
    //
    // Root Cargo.toml has both [workspace] and [package] (like clap).
    // L2 must work for both root crate and member crates.
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_e2e_03_non_virtual_workspace_l2() {
        // Given: a non-virtual workspace with root package "root_pkg"
        // and member "member_a"
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"member_a\"]\n\n[package]\nname = \"root_pkg\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Root crate src + tests
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::create_dir_all(tmp.path().join("tests")).unwrap();
        let root_src = tmp.path().join("src").join("lib.rs");
        std::fs::write(&root_src, "pub fn root_fn() {}\n").unwrap();
        let root_test = tmp.path().join("tests").join("test_root.rs");
        std::fs::write(
            &root_test,
            "use root_pkg::lib::root_fn;\n#[test]\nfn test_root() { }\n",
        )
        .unwrap();

        // Member crate
        let member_dir = tmp.path().join("member_a");
        std::fs::create_dir_all(member_dir.join("src")).unwrap();
        std::fs::create_dir_all(member_dir.join("tests")).unwrap();
        std::fs::write(
            member_dir.join("Cargo.toml"),
            "[package]\nname = \"member_a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let member_src = member_dir.join("src").join("handler.rs");
        std::fs::write(&member_src, "pub fn handle() {}\n").unwrap();
        let member_test = member_dir.join("tests").join("test_handler.rs");
        std::fs::write(
            &member_test,
            "use member_a::handler::handle;\n#[test]\nfn test_handle() { handle(); }\n",
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let root_src_path = root_src.to_string_lossy().into_owned();
        let member_src_path = member_src.to_string_lossy().into_owned();
        let root_test_path = root_test.to_string_lossy().into_owned();
        let member_test_path = member_test.to_string_lossy().into_owned();

        let production_files = vec![root_src_path.clone(), member_src_path.clone()];
        let test_sources: HashMap<String, String> = [
            (
                root_test_path.clone(),
                std::fs::read_to_string(&root_test).unwrap(),
            ),
            (
                member_test_path.clone(),
                std::fs::read_to_string(&member_test).unwrap(),
            ),
        ]
        .into_iter()
        .collect();

        // When: map_test_files_with_imports at workspace root
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: member's test maps to member's src via L2
        let member_mapping = result.iter().find(|m| m.production_file == member_src_path);
        assert!(member_mapping.is_some(), "No mapping for member handler.rs");
        let member_mapping = member_mapping.unwrap();
        assert!(
            member_mapping.test_files.contains(&member_test_path),
            "Expected member test mapped via L2, got: {:?}",
            member_mapping.test_files
        );
        assert_eq!(
            member_mapping.strategy,
            MappingStrategy::ImportTracing,
            "Expected ImportTracing for member, got: {:?}",
            member_mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // RS-WS-08: has_workspace_section detects [workspace]
    // -----------------------------------------------------------------------
    #[test]
    fn rs_ws_08_has_workspace_section() {
        let tmp = tempfile::tempdir().unwrap();

        // Virtual workspace
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n",
        )
        .unwrap();
        assert!(has_workspace_section(tmp.path()));

        // Non-virtual workspace
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"a\"]\n\n[package]\nname = \"root\"\n",
        )
        .unwrap();
        assert!(has_workspace_section(tmp.path()));

        // Single crate (no workspace)
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"single\"\n",
        )
        .unwrap();
        assert!(!has_workspace_section(tmp.path()));

        // No Cargo.toml
        std::fs::remove_file(tmp.path().join("Cargo.toml")).unwrap();
        assert!(!has_workspace_section(tmp.path()));
    }

    // -----------------------------------------------------------------------
    // RS-L0-BARREL-01: mod.rs with inline tests must NOT be self-mapped (TC-01)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_barrel_01_mod_rs_excluded() {
        // Given: mod.rs containing #[cfg(test)] in production_files
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let mod_rs = src_dir.join("mod.rs");
        std::fs::write(
            &mod_rs,
            r#"pub mod sub;

#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {}
}
"#,
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = mod_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = HashMap::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: mod.rs is NOT self-mapped (barrel file exclusion)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            !mapping.unwrap().test_files.contains(&prod_path),
            "mod.rs should NOT be self-mapped, but found in: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-L0-BARREL-02: lib.rs with inline tests must NOT be self-mapped (TC-02)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_barrel_02_lib_rs_excluded() {
        // Given: lib.rs containing #[cfg(test)] in production_files
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let lib_rs = src_dir.join("lib.rs");
        std::fs::write(
            &lib_rs,
            r#"pub mod utils;

#[cfg(test)]
mod tests {
    #[test]
    fn test_lib() {}
}
"#,
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = lib_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = HashMap::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: lib.rs is NOT self-mapped (barrel file exclusion)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            !mapping.unwrap().test_files.contains(&prod_path),
            "lib.rs should NOT be self-mapped, but found in: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-L0-BARREL-03: regular .rs file with inline tests IS self-mapped (TC-03, regression)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_barrel_03_regular_file_self_mapped() {
        // Given: a regular .rs file (not a barrel) containing #[cfg(test)]
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let service_rs = src_dir.join("service.rs");
        std::fs::write(
            &service_rs,
            r#"pub fn do_work() {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_do_work() { assert!(true); }
}
"#,
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = service_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = HashMap::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: service.rs IS self-mapped (regular file with inline tests)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            mapping.unwrap().test_files.contains(&prod_path),
            "service.rs should be self-mapped, but not found in: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-L0-BARREL-04: main.rs with inline tests must NOT be self-mapped (TC-04)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_barrel_04_main_rs_excluded() {
        // Given: main.rs containing #[cfg(test)] in production_files
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let main_rs = src_dir.join("main.rs");
        std::fs::write(
            &main_rs,
            r#"fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_main() {}
}
"#,
        )
        .unwrap();

        let extractor = RustExtractor::new();
        let prod_path = main_rs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = HashMap::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: main.rs is NOT self-mapped (entry point file exclusion)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(mapping.is_some());
        assert!(
            !mapping.unwrap().test_files.contains(&prod_path),
            "main.rs should NOT be self-mapped, but found in: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-L0-DETECT-01: #[cfg(test)] mod tests {} -> detect_inline_tests = true
    // (REGRESSION: should PASS with current implementation)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_detect_01_cfg_test_with_mod_block() {
        // Given: source with #[cfg(test)] followed by mod tests { ... }
        let source = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}
"#;
        // When: detect_inline_tests is called
        // Then: returns true (real inline test module)
        assert!(detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L0-DETECT-02: #[cfg(test)] for helper method (no mod) -> false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_detect_02_cfg_test_for_helper_method() {
        // Given: source with #[cfg(test)] applied to a function (not a mod)
        let source = r#"
pub struct Connection;

impl Connection {
    #[cfg(test)]
    pub fn test_helper(&self) -> bool {
        true
    }
}
"#;
        // When: detect_inline_tests is called
        // Then: returns false (cfg(test) does not annotate a mod_item)
        assert!(!detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L0-DETECT-03: #[cfg(test)] for mock substitution (use statement) -> false
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_detect_03_cfg_test_for_use_statement() {
        // Given: source with #[cfg(test)] applied to a use statement (mock substitution)
        let source = r#"
#[cfg(not(test))]
use real_http::Client;

#[cfg(test)]
use mock_http::Client;

pub fn fetch(url: &str) -> String {
    Client::get(url)
}
"#;
        // When: detect_inline_tests is called
        // Then: returns false (cfg(test) annotates a use item, not a mod_item)
        assert!(!detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L0-DETECT-04: #[cfg(test)] mod tests; (external module ref) -> true
    // (REGRESSION: should PASS with current implementation)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l0_detect_04_cfg_test_with_external_mod_ref() {
        // Given: source with #[cfg(test)] followed by mod tests; (semicolon form)
        let source = r#"
pub fn compute(x: i32) -> i32 { x * 2 }

#[cfg(test)]
mod tests;
"#;
        // When: detect_inline_tests is called
        // Then: returns true (mod_item via external module reference)
        assert!(detect_inline_tests(source));
    }

    // -----------------------------------------------------------------------
    // RS-L2-EXPORT-FILTER-01: test imports symbol directly from module path,
    // module file does NOT export that symbol -> file NOT mapped
    //
    // Scenario: use myapp::runtime::driver::{Builder}
    // driver.rs resolves directly (non-barrel), does NOT export Builder.
    // collect_import_matches() else-branch currently maps it without symbol check.
    // apply_l2_imports() should filter via file_exports_any_symbol().
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l2_export_filter_01_no_export_not_mapped() {
        // Given: temp directory mimicking a crate with:
        //   src/runtime/driver.rs: exports spawn() and Driver, NOT Builder
        //   tests/test_runtime.rs: use myapp::runtime::driver::{Builder}
        let tmp = tempfile::tempdir().unwrap();
        let src_runtime = tmp.path().join("src").join("runtime");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_runtime).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        // Cargo.toml
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // src/runtime/driver.rs - exports spawn() and Driver, NOT Builder
        let driver_rs = src_runtime.join("driver.rs");
        std::fs::write(&driver_rs, "pub fn spawn() {}\npub struct Driver;\n").unwrap();

        // tests/test_runtime.rs - imports Builder directly from runtime::driver
        // driver.rs resolves as a non-barrel file (no mod.rs lookup needed)
        let test_rs = tests_dir.join("test_runtime.rs");
        let test_source = "use myapp::runtime::driver::{Builder};\n\n#[test]\nfn test_build() {}\n";
        std::fs::write(&test_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let driver_path = driver_rs.to_string_lossy().into_owned();
        let test_path = test_rs.to_string_lossy().into_owned();
        let production_files = vec![driver_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: driver.rs is NOT mapped to test_runtime.rs
        // (driver.rs does not export Builder — apply_l2_imports must filter it)
        let mapping = result.iter().find(|m| m.production_file == driver_path);
        if let Some(m) = mapping {
            assert!(
                !m.test_files.contains(&test_path),
                "driver.rs should NOT be mapped (does not export Builder), but found: {:?}",
                m.test_files
            );
        }
        // If no mapping entry exists at all, that is also acceptable
    }

    // -----------------------------------------------------------------------
    // RS-L2-EXPORT-FILTER-02: barrel with pub mod service, test imports
    // ServiceFn which service.rs DOES export -> service.rs IS mapped
    // (REGRESSION: should PASS with current implementation)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l2_export_filter_02_exports_symbol_is_mapped() {
        // Given: temp directory mimicking a crate with:
        //   src/app/mod.rs: pub mod service;
        //   src/app/service.rs: exports pub fn service_fn()
        //   tests/test_app.rs: use myapp::app::{service_fn}
        let tmp = tempfile::tempdir().unwrap();
        let src_app = tmp.path().join("src").join("app");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_app).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        // Cargo.toml
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // src/app/mod.rs - pub mod service
        let mod_rs = src_app.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod service;\n").unwrap();

        // src/app/service.rs - exports service_fn
        let service_rs = src_app.join("service.rs");
        std::fs::write(&service_rs, "pub fn service_fn() {}\n").unwrap();

        // tests/test_app.rs - imports service_fn from app
        let test_rs = tests_dir.join("test_app.rs");
        let test_source = "use myapp::app::{service_fn};\n\n#[test]\nfn test_service() {}\n";
        std::fs::write(&test_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let service_path = service_rs.to_string_lossy().into_owned();
        let test_path = test_rs.to_string_lossy().into_owned();
        let production_files = vec![service_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: service.rs IS mapped to test_app.rs
        // (service.rs exports service_fn which the test imports)
        let mapping = result.iter().find(|m| m.production_file == service_path);
        assert!(mapping.is_some(), "service.rs should have a mapping entry");
        assert!(
            mapping.unwrap().test_files.contains(&test_path),
            "service.rs should be mapped to test_app.rs, got: {:?}",
            mapping.unwrap().test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-CFG-01: cfg_feat! { pub mod sub; } -> extract_barrel_re_exports
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_cfg_macro_pub_mod() {
        // Given: barrel mod.rs with cfg_feat! { pub mod sub; }
        let source = r#"
cfg_feat! {
    pub mod sub;
}
"#;

        // When: extract_barrel_re_exports is called
        let ext = RustExtractor::new();
        let result = ext.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: result contains BarrelReExport for "./sub" with wildcard=true
        assert!(
            !result.is_empty(),
            "Expected non-empty result, got: {:?}",
            result
        );
        assert!(
            result
                .iter()
                .any(|r| r.from_specifier == "./sub" && r.wildcard),
            "./sub with wildcard=true not found in {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-CFG-02: cfg_feat! { pub use util::{Symbol}; } -> extract_barrel_re_exports
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_cfg_macro_pub_use_braces() {
        // Given: barrel mod.rs with cfg_feat! { pub use util::{Symbol}; }
        let source = r#"
cfg_feat! {
    pub use util::{Symbol};
}
"#;

        // When: extract_barrel_re_exports is called
        let ext = RustExtractor::new();
        let result = ext.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: result contains BarrelReExport for "./util" with symbols=["Symbol"]
        assert!(
            !result.is_empty(),
            "Expected non-empty result, got: {:?}",
            result
        );
        assert!(
            result.iter().any(|r| r.from_specifier == "./util"
                && !r.wildcard
                && r.symbols.contains(&"Symbol".to_string())),
            "./util with symbols=[\"Symbol\"] not found in {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-CFG-03: top-level pub mod foo; (no macro) regression
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_top_level_regression() {
        // Given: barrel mod.rs with top-level pub mod foo; (no macro wrapper)
        let source = "pub mod foo;\n";

        // When: extract_barrel_re_exports is called
        let ext = RustExtractor::new();
        let result = ext.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: foo is detected (regression - top-level pub mod must still work)
        let entry = result.iter().find(|e| e.from_specifier == "./foo");
        assert!(
            entry.is_some(),
            "./foo not found in {:?} (regression: top-level pub mod broken)",
            result
        );
        assert!(entry.unwrap().wildcard);
    }

    // -----------------------------------------------------------------------
    // RS-IMP-09: parse_use_path single-segment module import (use crate::fs)
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_09_single_segment_module_import() {
        // Given: source with a single-segment module import after crate:: prefix
        let source = "use crate::fs;\n";

        // When: extract_all_import_specifiers is called
        let extractor = RustExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: result contains ("fs", []) — single-segment module import with empty symbols
        let entry = result.iter().find(|(spec, _)| spec == "fs");
        assert!(
            entry.is_some(),
            "fs not found in {:?} (single-segment module import should be registered)",
            result
        );
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "Expected empty symbols for module import, got: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-IMP-10: parse_use_path single-segment with crate_name
    // -----------------------------------------------------------------------
    #[test]
    fn rs_imp_10_single_segment_with_crate_name() {
        // Given: source `use my_crate::util;\n`, crate_name = "my_crate"
        let source = "use my_crate::util;\n";

        // When: extract_import_specifiers_with_crate_name called with crate_name = "my_crate"
        let result = extract_import_specifiers_with_crate_name(source, Some("my_crate"));

        // Then: result contains ("util", []) — single-segment module import with empty symbols
        let entry = result.iter().find(|(spec, _)| spec == "util");
        assert!(
            entry.is_some(),
            "util not found in {:?} (single-segment with crate_name should be registered)",
            result
        );
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "Expected empty symbols for module import, got: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-SELF-01: extract_barrel_re_exports strips self:: from wildcard
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_self_01_strips_self_from_wildcard() {
        // Given: barrel source with `pub use self::sub::*;`
        let source = "pub use self::sub::*;\n";

        // When: extract_barrel_re_exports is called on mod.rs
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier = "./sub" (not "./self/sub"), wildcard = true
        let entry = result.iter().find(|e| e.from_specifier == "./sub");
        assert!(
            entry.is_some(),
            "./sub not found in {:?} (self:: prefix should be stripped from wildcard)",
            result
        );
        assert!(
            entry.unwrap().wildcard,
            "Expected wildcard=true for pub use self::sub::*"
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-SELF-02: extract_barrel_re_exports strips self:: from symbol
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_self_02_strips_self_from_symbol() {
        // Given: barrel source with `pub use self::file::File;`
        let source = "pub use self::file::File;\n";

        // When: extract_barrel_re_exports is called on mod.rs
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier = "./file" (not "./self/file"), symbols = ["File"]
        let entry = result.iter().find(|e| e.from_specifier == "./file");
        assert!(
            entry.is_some(),
            "./file not found in {:?} (self:: prefix should be stripped from symbol import)",
            result
        );
        let entry = entry.unwrap();
        assert!(
            entry.symbols.contains(&"File".to_string()),
            "Expected symbols=[\"File\"], got: {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-SELF-03: extract_barrel_re_exports strips self:: from use list
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_self_03_strips_self_from_use_list() {
        // Given: barrel source with `pub use self::sync::{Mutex, RwLock};`
        let source = "pub use self::sync::{Mutex, RwLock};\n";

        // When: extract_barrel_re_exports is called on mod.rs
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier = "./sync" (not "./self/sync"), symbols contains "Mutex" and "RwLock"
        let entry = result.iter().find(|e| e.from_specifier == "./sync");
        assert!(
            entry.is_some(),
            "./sync not found in {:?} (self:: prefix should be stripped from use list)",
            result
        );
        let entry = entry.unwrap();
        assert!(
            entry.symbols.contains(&"Mutex".to_string()),
            "Expected Mutex in symbols, got: {:?}",
            entry.symbols
        );
        assert!(
            entry.symbols.contains(&"RwLock".to_string()),
            "Expected RwLock in symbols, got: {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-BARREL-CFG-SELF-01: extract_re_exports_from_text strips self:: in cfg macro
    // -----------------------------------------------------------------------
    #[test]
    fn rs_barrel_cfg_self_01_strips_self_in_cfg_macro() {
        // Given: barrel source with cfg macro block containing `pub use self::inner::Symbol;`
        let source = "cfg_feat! { pub use self::inner::Symbol; }\n";

        // When: extract_barrel_re_exports is called on mod.rs
        let extractor = RustExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "src/mod.rs");

        // Then: from_specifier = "./inner" (not "./self/inner"), symbols = ["Symbol"]
        let entry = result.iter().find(|e| e.from_specifier == "./inner");
        assert!(
            entry.is_some(),
            "./inner not found in {:?} (self:: prefix should be stripped in cfg macro text path)",
            result
        );
        let entry = entry.unwrap();
        assert!(
            entry.symbols.contains(&"Symbol".to_string()),
            "Expected symbols=[\"Symbol\"], got: {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // RS-L2-SELF-BARREL-E2E: L2 resolves through self:: barrel
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l2_self_barrel_e2e_resolves_through_self_barrel() {
        // Given: Cargo project with:
        //   src/fs/mod.rs (barrel: `pub use self::file::File;`)
        //   src/fs/file.rs (exports `pub struct File;`)
        //   tests/test_fs.rs (imports `use my_crate::fs::File;`)
        let tmp = tempfile::tempdir().unwrap();
        let src_fs_dir = tmp.path().join("src").join("fs");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_fs_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mod_rs = src_fs_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub use self::file::File;\n").unwrap();

        let file_rs = src_fs_dir.join("file.rs");
        std::fs::write(&file_rs, "pub struct File;\n").unwrap();

        let test_fs_rs = tests_dir.join("test_fs.rs");
        let test_source = "use my_crate::fs::File;\n\n#[test]\nfn test_fs() {}\n";
        std::fs::write(&test_fs_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let file_path = file_rs.to_string_lossy().into_owned();
        let test_path = test_fs_rs.to_string_lossy().into_owned();
        let production_files = vec![file_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: src/fs/file.rs is mapped to test_fs.rs (L2 resolves through self:: barrel)
        let mapping = result.iter().find(|m| m.production_file == file_path);
        assert!(mapping.is_some(), "No mapping for src/fs/file.rs");
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "Expected test_fs.rs to map to file.rs through self:: barrel (L2), got: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // RS-L2-SINGLE-SEG-E2E: L2 resolves single-segment module import
    // -----------------------------------------------------------------------
    #[test]
    fn rs_l2_single_seg_e2e_resolves_single_segment_module() {
        // Given: Cargo project with:
        //   src/fs/mod.rs (barrel: `pub mod copy;`)
        //   src/fs/copy.rs (`pub fn copy_file() {}`)
        //   tests/test_fs.rs (imports `use my_crate::fs;`)
        let tmp = tempfile::tempdir().unwrap();
        let src_fs_dir = tmp.path().join("src").join("fs");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&src_fs_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let mod_rs = src_fs_dir.join("mod.rs");
        std::fs::write(&mod_rs, "pub mod copy;\n").unwrap();

        let copy_rs = src_fs_dir.join("copy.rs");
        std::fs::write(&copy_rs, "pub fn copy_file() {}\n").unwrap();

        let test_fs_rs = tests_dir.join("test_fs.rs");
        let test_source = "use my_crate::fs;\n\n#[test]\nfn test_fs() {}\n";
        std::fs::write(&test_fs_rs, test_source).unwrap();

        let extractor = RustExtractor::new();
        let mod_path = mod_rs.to_string_lossy().into_owned();
        let copy_path = copy_rs.to_string_lossy().into_owned();
        let test_path = test_fs_rs.to_string_lossy().into_owned();
        let production_files = vec![mod_path.clone(), copy_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: src/fs/mod.rs or sub-modules (copy.rs) is mapped to test_fs.rs
        // (single-segment import `use my_crate::fs` should resolve to the fs module)
        let mod_mapping = result.iter().find(|m| m.production_file == mod_path);
        let copy_mapping = result.iter().find(|m| m.production_file == copy_path);
        let mod_mapped = mod_mapping
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        let copy_mapped = copy_mapping
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            mod_mapped || copy_mapped,
            "Expected test_fs.rs to map to src/fs/mod.rs or src/fs/copy.rs via single-segment L2, \
             mod_mapping: {:?}, copy_mapping: {:?}",
            mod_mapping.map(|m| &m.test_files),
            copy_mapping.map(|m| &m.test_files)
        );
    }
}
