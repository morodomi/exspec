use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryCursor};

use exspec_core::observe::{
    BarrelReExport, FileMapping, ImportMapping, MappingStrategy, ObserveExtractor,
    ProductionFunction,
};

use super::PythonExtractor;

const PRODUCTION_FUNCTION_QUERY: &str = include_str!("../queries/production_function.scm");
static PRODUCTION_FUNCTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const IMPORT_MAPPING_QUERY: &str = include_str!("../queries/import_mapping.scm");
static IMPORT_MAPPING_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const RE_EXPORT_QUERY: &str = include_str!("../queries/re_export.scm");
static RE_EXPORT_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const EXPORTED_SYMBOL_QUERY: &str = include_str!("../queries/exported_symbol.scm");
static EXPORTED_SYMBOL_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const BARE_IMPORT_ATTRIBUTE_QUERY: &str = include_str!("../queries/bare_import_attribute.scm");
static BARE_IMPORT_ATTRIBUTE_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const ASSERTION_QUERY: &str = include_str!("../queries/assertion.scm");
static ASSERTION_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const ASSIGNMENT_MAPPING_QUERY: &str = include_str!("../queries/assignment_mapping.scm");
static ASSIGNMENT_MAPPING_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

fn cached_query<'a>(lock: &'a OnceLock<Query>, source: &str) -> &'a Query {
    lock.get_or_init(|| {
        Query::new(&tree_sitter_python::LANGUAGE.into(), source).expect("invalid query")
    })
}

// ---------------------------------------------------------------------------
// Stem helpers
// ---------------------------------------------------------------------------

/// Extract stem from a test file path.
/// `test_user.py` -> `Some("user")`
/// `user_test.py` -> `Some("user")`
/// Other files -> `None`
pub fn test_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    // Must end with .py
    let stem = file_name.strip_suffix(".py")?;
    // test_*.py
    if let Some(rest) = stem.strip_prefix("test_") {
        return Some(rest);
    }
    // *_test.py
    if let Some(rest) = stem.strip_suffix("_test") {
        return Some(rest);
    }
    // Django: tests.py → parent directory name as stem
    if stem == "tests" {
        let sep_pos = path.rfind('/')?;
        let before_sep = &path[..sep_pos];
        let parent_start = before_sep.rfind('/').map(|i| i + 1).unwrap_or(0);
        let parent_name = &path[parent_start..sep_pos];
        if parent_name.is_empty() {
            return None;
        }
        return Some(parent_name);
    }
    None
}

/// Extract stem from a production file path.
/// `user.py` -> `Some("user")`
/// `_decoders.py` -> `Some("decoders")` (leading `_` stripped)
/// `__init__.py` -> `None`
/// `test_user.py` -> `None`
pub fn production_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".py")?;
    // Exclude __init__.py
    if stem == "__init__" {
        return None;
    }
    // Exclude Django tests.py
    if stem == "tests" {
        return None;
    }
    // Exclude test files
    if stem.starts_with("test_") || stem.ends_with("_test") {
        return None;
    }
    let stem = stem.strip_prefix('_').unwrap_or(stem);
    let stem = stem.strip_suffix("__").unwrap_or(stem);
    Some(stem)
}

/// Determine if a file is a non-SUT helper (should be excluded from mapping).
pub fn is_non_sut_helper(file_path: &str, is_known_production: bool) -> bool {
    // Phase 20: Path-segment check BEFORE is_known_production bypass.
    // Files inside tests/ or test/ directories that are NOT test files
    // are always helpers, even if they appear in production_files list.
    // (Same pattern as TypeScript observe.)
    let in_test_dir = file_path
        .split('/')
        .any(|seg| seg == "tests" || seg == "test");

    if in_test_dir {
        return true;
    }

    // Phase 21: Metadata/fixture/type-only files are always non-SUT helpers,
    // even if they appear in production_files list.
    // These files are frequently re-exported via barrels and cause FP fan-out.
    let stem_only = Path::new(file_path)
        .file_stem()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // __version__.py: package metadata, not a SUT
    if stem_only == "__version__" {
        return true;
    }

    // _types.py / __types__.py: pure type-definition files
    {
        let normalized = stem_only.trim_matches('_');
        if normalized == "types" || normalized.ends_with("_types") {
            return true;
        }
    }

    // mock.py / mock_*.py: test fixture/infrastructure
    if stem_only == "mock" || stem_only.starts_with("mock_") {
        return true;
    }

    if is_known_production {
        return false;
    }

    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    // Known helper filenames
    if matches!(
        file_name,
        "conftest.py" | "constants.py" | "setup.py" | "__init__.py"
    ) {
        return true;
    }

    // __pycache__/ files are helpers
    let parent_is_pycache = Path::new(file_path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .map(|s| s == "__pycache__")
        .unwrap_or(false);

    if parent_is_pycache {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Standalone helpers
// ---------------------------------------------------------------------------

/// Extract attribute names accessed on a bare-imported module.
///
/// For `import httpx; httpx.Client(); httpx.get()`, returns `["Client", "get"]`.
/// Returns empty vec if no attribute accesses are found (fallback to full match).
fn extract_bare_import_attributes(
    source_bytes: &[u8],
    tree: &tree_sitter::Tree,
    module_name: &str,
) -> Vec<String> {
    let query = cached_query(
        &BARE_IMPORT_ATTRIBUTE_QUERY_CACHE,
        BARE_IMPORT_ATTRIBUTE_QUERY,
    );
    let module_name_idx = query.capture_index_for_name("module_name").unwrap();
    let attribute_name_idx = query.capture_index_for_name("attribute_name").unwrap();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

    let mut attrs: Vec<String> = Vec::new();
    while let Some(m) = matches.next() {
        let mut mod_text = "";
        let mut attr_text = "";
        for cap in m.captures {
            if cap.index == module_name_idx {
                mod_text = cap.node.utf8_text(source_bytes).unwrap_or("");
            } else if cap.index == attribute_name_idx {
                attr_text = cap.node.utf8_text(source_bytes).unwrap_or("");
            }
        }
        if mod_text == module_name && !attr_text.is_empty() {
            attrs.push(attr_text.to_string());
        }
    }
    attrs.sort();
    attrs.dedup();
    attrs
}

// ---------------------------------------------------------------------------
// ObserveExtractor impl
// ---------------------------------------------------------------------------

impl ObserveExtractor for PythonExtractor {
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

        // Capture indices
        let name_idx = query.capture_index_for_name("name");
        let class_name_idx = query.capture_index_for_name("class_name");
        let method_name_idx = query.capture_index_for_name("method_name");
        let decorated_name_idx = query.capture_index_for_name("decorated_name");
        let decorated_class_name_idx = query.capture_index_for_name("decorated_class_name");
        let decorated_method_name_idx = query.capture_index_for_name("decorated_method_name");

        // Indices that represent function names (any of these → fn_name)
        let fn_name_indices: [Option<u32>; 4] = [
            name_idx,
            method_name_idx,
            decorated_name_idx,
            decorated_method_name_idx,
        ];
        // Indices that represent class names (any of these → class_name)
        let class_name_indices: [Option<u32>; 2] = [class_name_idx, decorated_class_name_idx];

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);
        let mut result = Vec::new();

        while let Some(m) = matches.next() {
            // Determine which pattern matched based on captures present
            let mut fn_name: Option<String> = None;
            let mut class_name: Option<String> = None;
            let mut line: usize = 1;

            for cap in m.captures {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                let node_line = cap.node.start_position().row + 1;

                if fn_name_indices.contains(&Some(cap.index)) {
                    fn_name = Some(text);
                    line = node_line;
                } else if class_name_indices.contains(&Some(cap.index)) {
                    class_name = Some(text);
                }
            }

            if let Some(name) = fn_name {
                result.push(ProductionFunction {
                    name,
                    file: file_path.to_string(),
                    line,
                    class_name,
                    is_exported: true,
                });
            }
        }

        // Deduplicate: same name + class_name pair may appear from multiple patterns
        let mut seen = HashSet::new();
        result.retain(|f| seen.insert((f.name.clone(), f.class_name.clone())));

        result
    }

    fn extract_imports(&self, source: &str, file_path: &str) -> Vec<ImportMapping> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);

        let module_name_idx = query.capture_index_for_name("module_name");
        let symbol_name_idx = query.capture_index_for_name("symbol_name");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Collect raw (module_text, symbol_text) pairs
        let mut raw: Vec<(String, String, usize)> = Vec::new();

        while let Some(m) = matches.next() {
            let mut module_text: Option<String> = None;
            let mut symbol_text: Option<String> = None;
            let mut symbol_line: usize = 1;

            for cap in m.captures {
                if module_name_idx == Some(cap.index) {
                    module_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                } else if symbol_name_idx == Some(cap.index) {
                    symbol_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                    symbol_line = cap.node.start_position().row + 1;
                }
            }

            let (module_text, symbol_text) = match (module_text, symbol_text) {
                (Some(m), Some(s)) => (m, s),
                _ => continue,
            };

            // Convert Python module path to specifier:
            // Leading dots: `.` -> `./`, `..` -> `../`, etc.
            // `from .models import X`  -> module_text might be ".models" or "models" depending on parse
            // We need to handle tree-sitter-python's representation
            let specifier_base = python_module_to_relative_specifier(&module_text);

            // Only include relative imports in extract_imports
            if specifier_base.starts_with("./") || specifier_base.starts_with("../") {
                // `from . import views` case: specifier_base is "./" (no module part)
                // In this case the symbol_name IS the module name, so specifier = "./{symbol}"
                let specifier = if specifier_base == "./"
                    && !module_text.contains('/')
                    && module_text.chars().all(|c| c == '.')
                {
                    format!("./{symbol_text}")
                } else {
                    specifier_base
                };
                raw.push((specifier, symbol_text, symbol_line));
            }
        }

        // Group by specifier: collect all symbols per specifier
        let mut specifier_symbols: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (spec, sym, line) in &raw {
            specifier_symbols
                .entry(spec.clone())
                .or_default()
                .push((sym.clone(), *line));
        }

        // Build ImportMapping per symbol
        let mut result = Vec::new();
        for (specifier, sym_lines) in &specifier_symbols {
            let all_symbols: Vec<String> = sym_lines.iter().map(|(s, _)| s.clone()).collect();
            for (sym, line) in sym_lines {
                result.push(ImportMapping {
                    symbol_name: sym.clone(),
                    module_specifier: specifier.clone(),
                    file: file_path.to_string(),
                    line: *line,
                    symbols: all_symbols.clone(),
                });
            }
        }

        result
    }

    fn extract_all_import_specifiers(&self, source: &str) -> Vec<(String, Vec<String>)> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&IMPORT_MAPPING_QUERY_CACHE, IMPORT_MAPPING_QUERY);

        let module_name_idx = query.capture_index_for_name("module_name");
        let symbol_name_idx = query.capture_index_for_name("symbol_name");
        let import_name_idx = query.capture_index_for_name("import_name");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut specifier_symbols: HashMap<String, Vec<String>> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut module_text: Option<String> = None;
            let mut symbol_text: Option<String> = None;
            let mut import_name_parts: Vec<String> = Vec::new();

            for cap in m.captures {
                if module_name_idx == Some(cap.index) {
                    module_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                } else if symbol_name_idx == Some(cap.index) {
                    symbol_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                } else if import_name_idx == Some(cap.index) {
                    // Use the parent dotted_name node's text to reconstruct the full
                    // module name (e.g., `os.path` from individual `identifier` captures).
                    let dotted_text = cap
                        .node
                        .parent()
                        .and_then(|p| p.utf8_text(source_bytes).ok())
                        .unwrap_or_else(|| cap.node.utf8_text(source_bytes).unwrap_or(""))
                        .to_string();
                    import_name_parts.push(dotted_text);
                }
            }

            if !import_name_parts.is_empty() {
                // bare import: `import X` or `import os.path`
                // Dedup in case multiple identifier captures share the same dotted_name parent.
                import_name_parts.dedup();
                let specifier = python_module_to_absolute_specifier(&import_name_parts[0]);
                if !specifier.starts_with("./")
                    && !specifier.starts_with("../")
                    && !specifier.is_empty()
                {
                    let attrs =
                        extract_bare_import_attributes(source_bytes, &tree, &import_name_parts[0]);
                    specifier_symbols.entry(specifier).or_insert_with(|| attrs);
                }
                continue;
            }

            let (module_text, symbol_text) = match (module_text, symbol_text) {
                (Some(m), Some(s)) => (m, s),
                _ => continue,
            };

            // Convert dotted module path to file path: `myapp.models` -> `myapp/models`
            let specifier = python_module_to_absolute_specifier(&module_text);

            // Skip relative imports (handled by extract_imports)
            // Skip empty specifiers (relative-only, like `from . import X` with no module)
            if specifier.starts_with("./") || specifier.starts_with("../") || specifier.is_empty() {
                continue;
            }

            specifier_symbols
                .entry(specifier)
                .or_default()
                .push(symbol_text);
        }

        specifier_symbols.into_iter().collect()
    }

    fn extract_barrel_re_exports(&self, source: &str, _file_path: &str) -> Vec<BarrelReExport> {
        let mut parser = Self::parser();
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&RE_EXPORT_QUERY_CACHE, RE_EXPORT_QUERY);

        let from_specifier_idx = query
            .capture_index_for_name("from_specifier")
            .expect("@from_specifier capture not found in re_export.scm");
        let symbol_name_idx = query.capture_index_for_name("symbol_name");
        let wildcard_idx = query.capture_index_for_name("wildcard");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Group symbols by from_specifier, tracking wildcard flag separately
        struct ReExportEntry {
            symbols: Vec<String>,
            wildcard: bool,
        }
        let mut grouped: HashMap<String, ReExportEntry> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut from_spec: Option<String> = None;
            let mut sym: Option<String> = None;
            let mut is_wildcard = false;

            for cap in m.captures {
                if cap.index == from_specifier_idx {
                    let raw = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                    from_spec = Some(python_module_to_relative_specifier(&raw));
                } else if wildcard_idx == Some(cap.index) {
                    is_wildcard = true;
                } else if symbol_name_idx == Some(cap.index) {
                    sym = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                }
            }

            if let Some(spec) = from_spec {
                // Only include relative re-exports
                if spec.starts_with("./") || spec.starts_with("../") {
                    let entry = grouped.entry(spec).or_insert(ReExportEntry {
                        symbols: Vec::new(),
                        wildcard: false,
                    });
                    if is_wildcard {
                        entry.wildcard = true;
                    }
                    if let Some(symbol) = sym {
                        if !entry.symbols.contains(&symbol) {
                            entry.symbols.push(symbol);
                        }
                    }
                }
            }
        }

        grouped
            .into_iter()
            .map(|(from_specifier, entry)| BarrelReExport {
                symbols: entry.symbols,
                from_specifier,
                wildcard: entry.wildcard,
                namespace_wildcard: false,
            })
            .collect()
    }

    fn source_extensions(&self) -> &[&str] {
        &["py"]
    }

    fn index_file_names(&self) -> &[&str] {
        &["__init__.py"]
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
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => return true, // If we can't read the file, assume it exports everything
        };

        let mut parser = Self::parser();
        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => return true,
        };
        let source_bytes = source.as_bytes();
        let query = cached_query(&EXPORTED_SYMBOL_QUERY_CACHE, EXPORTED_SYMBOL_QUERY);

        let symbol_idx = query.capture_index_for_name("symbol");
        let all_decl_idx = query.capture_index_for_name("all_decl");
        let var_name_idx = query.capture_index_for_name("var_name");

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut all_symbols: Vec<String> = Vec::new();
        let mut found_all = false;

        while let Some(m) = matches.next() {
            for cap in m.captures {
                // Detect __all__ existence via @var_name (pattern 1) or @all_decl (pattern 2)
                if var_name_idx == Some(cap.index) || all_decl_idx == Some(cap.index) {
                    found_all = true;
                } else if symbol_idx == Some(cap.index) {
                    let raw = cap.node.utf8_text(source_bytes).unwrap_or("");
                    let stripped = raw.trim_matches(|c| c == '\'' || c == '"');
                    all_symbols.push(stripped.to_string());
                }
            }
        }

        if !found_all {
            // No __all__ defined: treat as exporting everything
            return true;
        }

        // __all__ defined (possibly empty): check if any requested symbol is exported
        symbols.iter().any(|s| all_symbols.contains(s))
    }
}

// ---------------------------------------------------------------------------
// Module path conversion helpers
// ---------------------------------------------------------------------------

/// Convert a Python module specifier (as tree-sitter captures it) to a relative path specifier.
///
/// Tree-sitter-python represents `from .models import X` with module_name capturing `.models`
/// and `from ..utils import Y` with `..utils`.
/// `from . import views` has module_name capturing `.` (just dots).
///
/// We convert:
/// - `.models`  -> `./models`
/// - `..utils`  -> `../utils`
/// - `.`        -> `.` (handled separately as `from . import X`)
/// - `..`       -> `..` (handled separately)
fn python_module_to_relative_specifier(module: &str) -> String {
    // Count leading dots
    let dot_count = module.chars().take_while(|&c| c == '.').count();
    if dot_count == 0 {
        // Not a relative import
        return module.to_string();
    }

    let rest = &module[dot_count..];

    if dot_count == 1 {
        if rest.is_empty() {
            // `from . import X` -> specifier will be derived from symbol name
            // Return "./" as placeholder; caller uses the symbol as the path segment
            "./".to_string()
        } else {
            format!("./{rest}")
        }
    } else {
        // dot_count >= 2: `..` = `../`, `...` = `../../`, etc.
        let prefix = "../".repeat(dot_count - 1);
        if rest.is_empty() {
            // `from .. import X`
            prefix
        } else {
            format!("{prefix}{rest}")
        }
    }
}

/// Convert a Python absolute module path to a file-system path specifier.
/// `myapp.models` -> `myapp/models`
/// `.models`      -> (relative, skip)
fn python_module_to_absolute_specifier(module: &str) -> String {
    if module.starts_with('.') {
        // Relative import - not handled here
        return python_module_to_relative_specifier(module);
    }
    module.replace('.', "/")
}

// ---------------------------------------------------------------------------
// Concrete methods (not in trait)
// ---------------------------------------------------------------------------

/// Search depth 1 and depth 2 subdirectories of `scan_root` for a `manage.py` file.
///
/// Returns the first subdirectory that contains `manage.py`, or `None` if
/// `manage.py` exists at `scan_root` itself (already covered by canonical_root)
/// or no subdirectory contains `manage.py`.
pub fn find_manage_py_root(scan_root: &Path) -> Option<PathBuf> {
    // scan_root itself has manage.py → already covered by canonical_root
    if scan_root.join("manage.py").exists() {
        return None;
    }
    // Depth 1
    for entry in scan_root.read_dir().ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("manage.py").exists() {
            return Some(path);
        }
    }
    // Depth 2
    for entry in scan_root.read_dir().ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            for inner in path.read_dir().into_iter().flatten().flatten() {
                let inner_path = inner.path();
                if inner_path.is_dir() && inner_path.join("manage.py").exists() {
                    return Some(inner_path);
                }
            }
        }
    }
    None
}

/// Extract the set of import symbol names that appear (directly or via
/// variable chain) inside assertion nodes.
///
/// Algorithm:
/// 1. Parse source and find all assertion byte ranges via `assertion.scm`.
/// 2. Walk the AST within each assertion range to collect all `identifier`
///    leaf nodes → `assertion_identifiers`.
/// 3. Parse assignment mappings via `assignment_mapping.scm`:
///    - `@var` → `@class`  (direct: `var = ClassName()`)
///    - `@var` → `@source` (chain: `var = obj.method()`)
/// 4. Chain-expand `assertion_identifiers` up to 2 hops, resolving var →
///    class via the assignment map.
/// 5. Return the union of all resolved symbols.
///
/// Returns an empty `HashSet` when no assertions are found (caller is
/// responsible for the safe fallback to `all_matched`).
pub fn extract_assertion_referenced_imports(source: &str) -> HashSet<String> {
    let mut parser = PythonExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return HashSet::new(),
    };
    let source_bytes = source.as_bytes();

    // ---- Step 1: collect assertion byte ranges ----
    let assertion_query = cached_query(&ASSERTION_QUERY_CACHE, ASSERTION_QUERY);
    let assertion_cap_idx = match assertion_query.capture_index_for_name("assertion") {
        Some(idx) => idx,
        None => return HashSet::new(),
    };

    let mut assertion_ranges: Vec<(usize, usize)> = Vec::new();
    {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(assertion_query, tree.root_node(), source_bytes);
        while let Some(m) = matches.next() {
            for cap in m.captures {
                if cap.index == assertion_cap_idx {
                    let r = cap.node.byte_range();
                    assertion_ranges.push((r.start, r.end));
                }
            }
        }
    }

    if assertion_ranges.is_empty() {
        return HashSet::new();
    }

    // ---- Step 2: collect identifiers within assertion ranges (AST walk) ----
    let mut assertion_identifiers: HashSet<String> = HashSet::new();
    {
        let root = tree.root_node();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let nr = node.byte_range();
            // Only descend into nodes that overlap with at least one assertion range
            let overlaps = assertion_ranges
                .iter()
                .any(|&(s, e)| nr.start < e && nr.end > s);
            if !overlaps {
                continue;
            }
            if node.kind() == "identifier" {
                // The identifier itself must be within an assertion range
                if assertion_ranges
                    .iter()
                    .any(|&(s, e)| nr.start >= s && nr.end <= e)
                {
                    if let Ok(text) = node.utf8_text(source_bytes) {
                        if !text.is_empty() {
                            assertion_identifiers.insert(text.to_string());
                        }
                    }
                }
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
    }

    // ---- Step 3: build assignment map ----
    // Maps var_name → set of resolved names (class or source object)
    let assign_query = cached_query(&ASSIGNMENT_MAPPING_QUERY_CACHE, ASSIGNMENT_MAPPING_QUERY);
    let var_idx = assign_query.capture_index_for_name("var");
    let class_idx = assign_query.capture_index_for_name("class");
    let source_idx = assign_query.capture_index_for_name("source");

    // var → Vec<target_symbol>
    let mut assignment_map: HashMap<String, Vec<String>> = HashMap::new();

    if let (Some(var_cap), Some(class_cap), Some(source_cap)) = (var_idx, class_idx, source_idx) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(assign_query, tree.root_node(), source_bytes);
        while let Some(m) = matches.next() {
            let mut var_text = String::new();
            let mut target_text = String::new();
            for cap in m.captures {
                if cap.index == var_cap {
                    var_text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                } else if cap.index == class_cap || cap.index == source_cap {
                    let t = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                    if !t.is_empty() {
                        target_text = t;
                    }
                }
            }
            if !var_text.is_empty() && !target_text.is_empty() && var_text != target_text {
                assignment_map
                    .entry(var_text)
                    .or_default()
                    .push(target_text);
            }
        }
    }

    // ---- Step 4: chain-expand up to 2 hops ----
    let mut resolved: HashSet<String> = assertion_identifiers.clone();
    for _ in 0..2 {
        let mut additions: HashSet<String> = HashSet::new();
        for sym in &resolved {
            if let Some(targets) = assignment_map.get(sym) {
                for t in targets {
                    additions.insert(t.clone());
                }
            }
        }
        let before = resolved.len();
        resolved.extend(additions);
        if resolved.len() == before {
            break;
        }
    }

    resolved
}

/// Track newly matched production-file indices and the symbols that caused
/// them.  Called after each `collect_import_matches` invocation to update
/// `idx_to_symbols` with the diff between `all_matched` before and after.
fn track_new_matches(
    all_matched: &HashSet<usize>,
    before: &HashSet<usize>,
    symbols: &[String],
    idx_to_symbols: &mut HashMap<usize, HashSet<String>>,
) {
    for &new_idx in all_matched.difference(before) {
        let entry = idx_to_symbols.entry(new_idx).or_default();
        for s in symbols {
            entry.insert(s.clone());
        }
    }
}

impl PythonExtractor {
    /// Layer 1 + Layer 2: Map test files to production files.
    pub fn map_test_files_with_imports(
        &self,
        production_files: &[String],
        test_sources: &HashMap<String, String>,
        scan_root: &Path,
        l1_exclusive: bool,
    ) -> Vec<FileMapping> {
        let test_file_list: Vec<String> = test_sources.keys().cloned().collect();

        // Phase 20: Filter out test-directory helper files from production_files before
        // passing to Layer 1. Files inside tests/ or test/ path segments (relative to
        // scan_root) are helpers (e.g. tests/helpers.py, tests/testserver/server.py)
        // even when discover_files classifies them as production files.
        // We strip the scan_root prefix to get the relative path for segment checking,
        // avoiding false positives when the absolute path itself contains "tests" segments
        // (e.g. /path/to/project/tests/fixtures/observe/e2e_pkg/views.py).
        let canonical_root_for_filter = scan_root.canonicalize().ok();
        let filtered_production_files: Vec<String> = production_files
            .iter()
            .filter(|p| {
                let check_path = if let Some(ref root) = canonical_root_for_filter {
                    if let Ok(canonical_p) = Path::new(p).canonicalize() {
                        if let Ok(rel) = canonical_p.strip_prefix(root) {
                            rel.to_string_lossy().into_owned()
                        } else {
                            p.to_string()
                        }
                    } else {
                        p.to_string()
                    }
                } else {
                    p.to_string()
                };
                !is_non_sut_helper(&check_path, false)
            })
            .cloned()
            .collect();

        // Layer 1: filename convention
        let mut mappings =
            exspec_core::observe::map_test_files(self, &filtered_production_files, &test_file_list);

        // Build canonical path -> production index lookup
        let canonical_root = match scan_root.canonicalize() {
            Ok(r) => r,
            Err(_) => return mappings,
        };
        let manage_py_root = find_manage_py_root(scan_root).and_then(|p| p.canonicalize().ok());
        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        for (idx, prod) in filtered_production_files.iter().enumerate() {
            if let Ok(canonical) = Path::new(prod).canonicalize() {
                canonical_to_idx.insert(canonical.to_string_lossy().into_owned(), idx);
            }
        }

        // Record Layer 1 core matches per production file index
        let layer1_tests_per_prod: Vec<HashSet<String>> = mappings
            .iter()
            .map(|m| m.test_files.iter().cloned().collect())
            .collect();

        // Layer 1 extension: stem-only fallback (cross-directory match)
        // For test files that L1 core did not match, attempt stem-only match against prod files.
        // Stem collision guard: if multiple prod files share the same stem, defer to L2 import tracing.
        {
            // Build stem -> list of production indices (stem stripped of leading `_`)
            let mut stem_to_prod_indices: HashMap<String, Vec<usize>> = HashMap::new();
            for (idx, prod) in filtered_production_files.iter().enumerate() {
                if let Some(pstem) = self.production_stem(prod) {
                    stem_to_prod_indices
                        .entry(pstem.to_owned())
                        .or_default()
                        .push(idx);
                }
            }

            // Collect set of test files already matched by L1 core (any prod)
            let l1_core_matched: HashSet<&str> = layer1_tests_per_prod
                .iter()
                .flat_map(|s| s.iter().map(|t| t.as_str()))
                .collect();

            for test_file in &test_file_list {
                // Skip if L1 core already matched this test file
                if l1_core_matched.contains(test_file.as_str()) {
                    continue;
                }
                if let Some(tstem) = self.test_stem(test_file) {
                    if let Some(prod_indices) = stem_to_prod_indices.get(tstem) {
                        if prod_indices.len() > 1 {
                            continue; // stem collision: defer to L2 import tracing
                        }
                        for &idx in prod_indices {
                            if !mappings[idx].test_files.contains(test_file) {
                                mappings[idx].test_files.push(test_file.clone());
                            }
                        }
                    }
                }
            }
        }

        // Snapshot L1 (core + stem-only fallback) matches per prod for strategy update
        let layer1_extended_tests_per_prod: Vec<HashSet<String>> = mappings
            .iter()
            .map(|m| m.test_files.iter().cloned().collect())
            .collect();

        // Collect set of test files matched by L1 (core + stem-only fallback) for barrel suppression
        let l1_matched_tests: HashSet<String> = mappings
            .iter()
            .flat_map(|m| m.test_files.iter().cloned())
            .collect();

        // Layer 2: import tracing
        // Track production file indices matched ONLY via manage_py_root fallback
        // (needed to upgrade strategy to ImportTracing for Django-layout projects)
        let mut manage_py_only_prods: HashSet<usize> = HashSet::new();
        for (test_file, source) in test_sources {
            if l1_exclusive && l1_matched_tests.contains(test_file.as_str()) {
                continue;
            }
            let imports = <Self as ObserveExtractor>::extract_imports(self, source, test_file);
            let from_file = Path::new(test_file);
            // all_matched: every idx matched by L2 (traditional behavior)
            let mut all_matched = HashSet::<usize>::new();
            // idx_to_symbols: tracks which import symbols caused each idx match
            let mut idx_to_symbols: HashMap<usize, HashSet<String>> = HashMap::new();
            // direct_import_indices: indices resolved via non-barrel L2 absolute import
            // These bypass the assertion filter because `from pkg._sub import X` is a strong signal
            let mut direct_import_indices: HashSet<usize> = HashSet::new();

            for import in &imports {
                // Handle bare relative imports: `from . import X` (specifier="./")
                // or `from .. import X` (specifier="../"), etc.
                // These need per-symbol resolution since the module part is the symbol name.
                let is_bare_relative = (import.module_specifier == "./"
                    || import.module_specifier.ends_with('/'))
                    && import
                        .module_specifier
                        .trim_end_matches('/')
                        .chars()
                        .all(|c| c == '.');

                let specifier = if is_bare_relative {
                    let prefix =
                        &import.module_specifier[..import.module_specifier.len().saturating_sub(1)];
                    for sym in &import.symbols {
                        let sym_specifier = format!("{prefix}/{sym}");
                        if let Some(resolved) = exspec_core::observe::resolve_import_path(
                            self,
                            &sym_specifier,
                            from_file,
                            &canonical_root,
                        ) {
                            // Barrel suppression: skip barrel-resolved imports for L1-matched tests
                            if self.is_barrel_file(&resolved)
                                && l1_matched_tests.contains(test_file.as_str())
                            {
                                continue;
                            }
                            let sym_slice = &[sym.clone()];
                            let before = all_matched.clone();
                            exspec_core::observe::collect_import_matches(
                                self,
                                &resolved,
                                sym_slice,
                                &canonical_to_idx,
                                &mut all_matched,
                                &canonical_root,
                            );
                            track_new_matches(
                                &all_matched,
                                &before,
                                sym_slice,
                                &mut idx_to_symbols,
                            );
                            // Direct (non-barrel) bare relative import → assertion filter bypass
                            // Note: barrel files are skipped above for L1-matched tests only.
                            // For non-L1-matched tests, barrel imports may reach here, but
                            // !is_barrel_file prevents them from being added to direct_import_indices.
                            if !self.is_barrel_file(&resolved) {
                                for &idx in all_matched.difference(&before) {
                                    direct_import_indices.insert(idx);
                                }
                            }
                        }
                    }
                    continue;
                } else {
                    import.module_specifier.clone()
                };

                if let Some(resolved) = exspec_core::observe::resolve_import_path(
                    self,
                    &specifier,
                    from_file,
                    &canonical_root,
                ) {
                    // Barrel suppression: skip barrel-resolved imports for L1-matched tests
                    if self.is_barrel_file(&resolved)
                        && l1_matched_tests.contains(test_file.as_str())
                    {
                        continue;
                    }
                    let before = all_matched.clone();
                    exspec_core::observe::collect_import_matches(
                        self,
                        &resolved,
                        &import.symbols,
                        &canonical_to_idx,
                        &mut all_matched,
                        &canonical_root,
                    );
                    track_new_matches(&all_matched, &before, &import.symbols, &mut idx_to_symbols);
                    // Direct (non-barrel) non-bare relative import → assertion filter bypass
                    let is_direct = !self.is_barrel_file(&resolved);
                    if is_direct {
                        for &idx in all_matched.difference(&before) {
                            direct_import_indices.insert(idx);
                        }
                    }
                }
            }

            // Layer 2 (absolute imports): resolve from scan_root
            let abs_specifiers = self.extract_all_import_specifiers(source);
            for (specifier, symbols) in &abs_specifiers {
                let base = canonical_root.join(specifier);
                let standard_resolved = exspec_core::observe::resolve_absolute_base_to_file(
                    self,
                    &base,
                    &canonical_root,
                )
                .or_else(|| {
                    let src_base = canonical_root.join("src").join(specifier);
                    exspec_core::observe::resolve_absolute_base_to_file(
                        self,
                        &src_base,
                        &canonical_root,
                    )
                });
                let via_manage_py = standard_resolved.is_none() && manage_py_root.is_some();
                let resolved = standard_resolved.or_else(|| {
                    if let Some(ref mpr) = manage_py_root {
                        let django_base = mpr.join(specifier);
                        exspec_core::observe::resolve_absolute_base_to_file(
                            self,
                            &django_base,
                            &canonical_root,
                        )
                    } else {
                        None
                    }
                });
                if let Some(resolved) = resolved {
                    // Barrel suppression: skip barrel-resolved imports for L1-matched tests
                    if self.is_barrel_file(&resolved)
                        && l1_matched_tests.contains(test_file.as_str())
                    {
                        continue;
                    }
                    // Direct (non-barrel) import: bypasses assertion filter (L1080)
                    // to avoid FN when test imports sub-module directly.
                    let is_direct = !self.is_barrel_file(&resolved);
                    let before = all_matched.clone();
                    exspec_core::observe::collect_import_matches(
                        self,
                        &resolved,
                        symbols,
                        &canonical_to_idx,
                        &mut all_matched,
                        &canonical_root,
                    );
                    track_new_matches(&all_matched, &before, symbols, &mut idx_to_symbols);
                    // Track direct (non-barrel) absolute import matches for assertion filter bypass
                    if is_direct {
                        for &idx in all_matched.difference(&before) {
                            direct_import_indices.insert(idx);
                        }
                    }
                    // Track manage_py_root-only matches separately
                    if via_manage_py && is_direct {
                        for &idx in all_matched.difference(&before) {
                            manage_py_only_prods.insert(idx);
                        }
                    }
                }
            }

            // Assertion-referenced import filter (safe fallback)
            let asserted_imports = extract_assertion_referenced_imports(source);
            let final_indices: HashSet<usize> = if asserted_imports.is_empty() {
                // No assertions found -> fallback: use all_matched (PY-AF-06a)
                all_matched.clone()
            } else {
                // Filter to indices whose symbols intersect with asserted_imports
                let asserted_matched: HashSet<usize> = all_matched
                    .iter()
                    .copied()
                    .filter(|idx| {
                        idx_to_symbols
                            .get(idx)
                            .map(|syms| syms.iter().any(|s| asserted_imports.contains(s)))
                            .unwrap_or(false)
                    })
                    .collect();
                if asserted_matched.is_empty() {
                    // Assertions exist but no import symbol intersects -> safe fallback (PY-AF-06b, PY-AF-09)
                    all_matched.clone()
                } else {
                    // Include direct import indices regardless of assertion filter
                    let mut final_set = asserted_matched;
                    final_set.extend(direct_import_indices.intersection(&all_matched).copied());
                    final_set
                }
            };

            for idx in final_indices {
                if !mappings[idx].test_files.contains(test_file) {
                    mappings[idx].test_files.push(test_file.clone());
                }
            }
        }

        // Update strategy:
        // - If a production file had no Layer 1 matches but has L2 matches → ImportTracing
        // - If a production file was matched via manage_py_root fallback (Django layout),
        //   upgrade to ImportTracing regardless of L1 stem-only match
        for (i, mapping) in mappings.iter_mut().enumerate() {
            let has_layer1 = !layer1_extended_tests_per_prod[i].is_empty();
            if manage_py_only_prods.contains(&i) {
                // Matched via Django manage.py root fallback → ImportTracing
                mapping.strategy = MappingStrategy::ImportTracing;
            } else if !has_layer1 && !mapping.test_files.is_empty() {
                mapping.strategy = MappingStrategy::ImportTracing;
            }
        }

        mappings
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
    // PY-STEM-01: test_user.py -> test_stem = Some("user")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_01_test_prefix() {
        // Given: a file named test_user.py
        // When: test_stem is called
        // Then: returns Some("user")
        let extractor = PythonExtractor::new();
        let result = extractor.test_stem("tests/test_user.py");
        assert_eq!(result, Some("user"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-02: user_test.py -> test_stem = Some("user")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_02_test_suffix() {
        // Given: a file named user_test.py
        // When: test_stem is called
        // Then: returns Some("user")
        let extractor = PythonExtractor::new();
        let result = extractor.test_stem("tests/user_test.py");
        assert_eq!(result, Some("user"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-03: test_user_service.py -> test_stem = Some("user_service")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_03_test_prefix_multi_segment() {
        // Given: a file named test_user_service.py
        // When: test_stem is called
        // Then: returns Some("user_service")
        let extractor = PythonExtractor::new();
        let result = extractor.test_stem("tests/test_user_service.py");
        assert_eq!(result, Some("user_service"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-04: user.py -> production_stem = Some("user")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_04_production_stem_regular() {
        // Given: a regular production file user.py
        // When: production_stem is called
        // Then: returns Some("user")
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("src/user.py");
        assert_eq!(result, Some("user"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-05: __init__.py -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_05_production_stem_init() {
        // Given: __init__.py (barrel file)
        // When: production_stem is called
        // Then: returns None
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("src/__init__.py");
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // PY-STEM-06: test_user.py -> production_stem = None
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_06_production_stem_test_file() {
        // Given: a test file test_user.py
        // When: production_stem is called
        // Then: returns None (test files are not production)
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("tests/test_user.py");
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-01: conftest.py -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_01_conftest() {
        // Given: conftest.py
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/conftest.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-02: constants.py -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_02_constants() {
        // Given: constants.py
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("src/constants.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-03: __init__.py -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_03_init() {
        // Given: __init__.py
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("src/__init__.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-04: tests/utils.py -> is_non_sut_helper = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_04_utils_under_tests_dir() {
        // Given: utils.py under tests/ directory (not a test file)
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/utils.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-05: models.py -> is_non_sut_helper = false
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_05_models_is_not_helper() {
        // Given: models.py (regular production file)
        // When: is_non_sut_helper is called
        // Then: returns false
        let extractor = PythonExtractor::new();
        assert!(!extractor.is_non_sut_helper("src/models.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-06: tests/common.py -> helper even when is_known_production=true
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_06_tests_common_helper_despite_known_production() {
        // Given: file is tests/common.py with is_known_production=true
        // When: is_non_sut_helper is called
        // Then: returns true (path segment check overrides is_known_production)
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/common.py", true));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-07: tests/testserver/server.py -> helper (subdirectory of tests/)
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_07_tests_subdirectory_helper() {
        // Given: file is tests/testserver/server.py (inside tests/ dir but not a test file)
        // When: is_non_sut_helper is called
        // Then: returns true (path segment check catches subdirectories)
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/testserver/server.py", true));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-08: tests/compat.py -> helper (is_known_production=false)
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_08_tests_compat_helper() {
        // Given: file is tests/compat.py (inside tests/ dir, not a test file)
        // When: is_non_sut_helper is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/compat.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-09: tests/fixtures/data.py -> helper (deep nesting inside tests/)
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_09_deep_nested_test_dir_helper() {
        // Given: file is tests/fixtures/data.py (deeply nested inside tests/)
        // When: is_non_sut_helper is called
        // Then: returns true (path segment check catches any depth under tests/)
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("tests/fixtures/data.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-10: src/tests.py -> NOT helper (filename not dir segment)
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_10_tests_in_filename_not_helper() {
        // Given: file is src/tests.py ("tests" is in filename, not a directory segment)
        // When: is_non_sut_helper is called
        // Then: returns false (path segment check must not match filename)
        let extractor = PythonExtractor::new();
        assert!(!extractor.is_non_sut_helper("src/tests.py", false));
    }

    // -----------------------------------------------------------------------
    // PY-HELPER-11: test/helpers.py -> helper (test/ singular directory)
    // -----------------------------------------------------------------------
    #[test]
    fn py_helper_11_test_singular_dir_helper() {
        // Given: file is test/helpers.py (singular "test" directory, not "tests")
        // When: is_non_sut_helper is called
        // Then: returns true (segment check matches both "tests" and "test")
        let extractor = PythonExtractor::new();
        assert!(extractor.is_non_sut_helper("test/helpers.py", true));
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-01: __init__.py -> is_barrel_file = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_01_init_is_barrel() {
        // Given: __init__.py
        // When: is_barrel_file is called
        // Then: returns true
        let extractor = PythonExtractor::new();
        assert!(extractor.is_barrel_file("src/mypackage/__init__.py"));
    }

    // -----------------------------------------------------------------------
    // PY-FUNC-01: def create_user() -> name="create_user", class_name=None
    // -----------------------------------------------------------------------
    #[test]
    fn py_func_01_top_level_function() {
        // Given: Python source with a top-level function
        let source = r#"
def create_user():
    pass
"#;
        // When: extract_production_functions is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_production_functions(source, "src/users.py");

        // Then: name="create_user", class_name=None
        let func = result.iter().find(|f| f.name == "create_user");
        assert!(func.is_some(), "create_user not found in {:?}", result);
        let func = func.unwrap();
        assert_eq!(func.class_name, None);
    }

    // -----------------------------------------------------------------------
    // PY-FUNC-02: class User: def save(self) -> name="save", class_name=Some("User")
    // -----------------------------------------------------------------------
    #[test]
    fn py_func_02_class_method() {
        // Given: Python source with a class containing a method
        let source = r#"
class User:
    def save(self):
        pass
"#;
        // When: extract_production_functions is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_production_functions(source, "src/models.py");

        // Then: name="save", class_name=Some("User")
        let method = result.iter().find(|f| f.name == "save");
        assert!(method.is_some(), "save not found in {:?}", result);
        let method = method.unwrap();
        assert_eq!(method.class_name, Some("User".to_string()));
    }

    // -----------------------------------------------------------------------
    // PY-FUNC-03: @decorator def endpoint() -> extracted
    // -----------------------------------------------------------------------
    #[test]
    fn py_func_03_decorated_function() {
        // Given: Python source with a decorated function
        let source = r#"
import functools

def my_decorator(func):
    @functools.wraps(func)
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper

@my_decorator
def endpoint():
    pass
"#;
        // When: extract_production_functions is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_production_functions(source, "src/views.py");

        // Then: endpoint is extracted
        let func = result.iter().find(|f| f.name == "endpoint");
        assert!(func.is_some(), "endpoint not found in {:?}", result);
    }

    // -----------------------------------------------------------------------
    // PY-IMP-01: from .models import User -> specifier="./models", symbols=["User"]
    // -----------------------------------------------------------------------
    #[test]
    fn py_imp_01_relative_import_from_dot() {
        // Given: source with relative import from .models
        let source = "from .models import User\n";

        // When: extract_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_imports(source, "tests/test_user.py");

        // Then: one entry with specifier="./models", symbols=["User"]
        let imp = result.iter().find(|i| i.module_specifier == "./models");
        assert!(
            imp.is_some(),
            "import from ./models not found in {:?}",
            result
        );
        let imp = imp.unwrap();
        assert!(
            imp.symbols.contains(&"User".to_string()),
            "User not in symbols: {:?}",
            imp.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMP-02: from ..utils import helper -> specifier="../utils", symbols=["helper"]
    // -----------------------------------------------------------------------
    #[test]
    fn py_imp_02_relative_import_two_dots() {
        // Given: source with two-dot relative import
        let source = "from ..utils import helper\n";

        // When: extract_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_imports(source, "tests/unit/test_something.py");

        // Then: one entry with specifier="../utils", symbols=["helper"]
        let imp = result.iter().find(|i| i.module_specifier == "../utils");
        assert!(
            imp.is_some(),
            "import from ../utils not found in {:?}",
            result
        );
        let imp = imp.unwrap();
        assert!(
            imp.symbols.contains(&"helper".to_string()),
            "helper not in symbols: {:?}",
            imp.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMP-03: from myapp.models import User -> ("myapp/models", ["User"])
    // -----------------------------------------------------------------------
    #[test]
    fn py_imp_03_absolute_import_dotted() {
        // Given: source with absolute import using dotted module path
        let source = "from myapp.models import User\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("myapp/models", ["User"])
        let entry = result.iter().find(|(spec, _)| spec == "myapp/models");
        assert!(entry.is_some(), "myapp/models not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"User".to_string()),
            "User not in symbols: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMP-04: import os -> not resolved (skipped)
    // -----------------------------------------------------------------------
    #[test]
    fn py_imp_04_plain_import_skipped() {
        // Given: source with a plain stdlib import
        let source = "import os\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: "os" is present with empty symbols (bare import produces no symbol constraints)
        let os_entry = result.iter().find(|(spec, _)| spec == "os");
        assert!(
            os_entry.is_some(),
            "plain 'import os' should be included as bare import, got {:?}",
            result
        );
        let (_, symbols) = os_entry.unwrap();
        assert!(
            symbols.is_empty(),
            "expected empty symbols for bare import, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMP-05: from . import views -> specifier="./views", symbols=["views"]
    // -----------------------------------------------------------------------
    #[test]
    fn py_imp_05_from_dot_import_name() {
        // Given: source with `from . import views`
        let source = "from . import views\n";

        // When: extract_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_imports(source, "tests/test_app.py");

        // Then: specifier="./views", symbols=["views"]
        let imp = result.iter().find(|i| i.module_specifier == "./views");
        assert!(imp.is_some(), "./views not found in {:?}", result);
        let imp = imp.unwrap();
        assert!(
            imp.symbols.contains(&"views".to_string()),
            "views not in symbols: {:?}",
            imp.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-01: `import httpx` -> specifier="httpx", symbols=[]
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_01_bare_import_simple() {
        // Given: source with a bare import of a third-party package
        let source = "import httpx\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", []) -- bare import produces empty symbols
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(
            entry.is_some(),
            "httpx not found in {:?}; bare import should be included",
            result
        );
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "expected empty symbols for bare import, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-02: `import os.path` -> specifier="os/path", symbols=[]
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_01b_bare_import_attribute_access_narrowing() {
        // Given: source with bare import + attribute access (simple, non-dotted)
        let source = "import httpx\nhttpx.Client()\nhttpx.get('/api')\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", ["Client", "get"]) -- attribute access narrows symbols
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"Client".to_string()),
            "expected Client in symbols, got {:?}",
            symbols
        );
        assert!(
            symbols.contains(&"get".to_string()),
            "expected get in symbols, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-02a: `import os.path; os.path.join(...)` -> specifier="os/path", symbols=[]
    //   Dotted bare import attribute-access fallback: @module_name captures single
    //   identifier "os" but import_name_parts[0] is "os.path", so no match → empty symbols.
    //   This is intentional: fallback to match-all is the safe side.
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_02a_dotted_bare_import_attribute_fallback() {
        // Given: source with dotted bare import + attribute access
        let source = "import os.path\nos.path.join('/a', 'b')\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: specifier="os/path", symbols=[] (fallback: tree-sitter @module_name captures
        //   "os" (single identifier) but import_name_parts[0] is "os.path", so mismatch → empty)
        let entry = result.iter().find(|(spec, _)| spec == "os/path");
        assert!(entry.is_some(), "os/path not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "expected empty symbols for dotted bare import (intentional fallback), got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-02: `import os.path` -> specifier="os/path", symbols=[]
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_02_bare_import_dotted() {
        // Given: source with a dotted bare import
        let source = "import os.path\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("os/path", []) -- dots converted to slashes
        let entry = result.iter().find(|(spec, _)| spec == "os/path");
        assert!(
            entry.is_some(),
            "os/path not found in {:?}; dotted bare import should be converted",
            result
        );
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "expected empty symbols for dotted bare import, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-03: `from httpx import Client` -> specifier="httpx", symbols=["Client"]
    //               (regression: from-import still works after bare-import change)
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_03_from_import_regression() {
        // Given: source with a from-import (existing behaviour must not regress)
        let source = "from httpx import Client\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", ["Client"])
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"Client".to_string()),
            "Client not in symbols: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-02: __init__.py with `from .module import Foo`
    //               -> extract_barrel_re_exports: symbols=["Foo"], from_specifier="./module"
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_02_re_export_named() {
        // Given: __init__.py content with a named re-export
        let source = "from .module import Foo\n";

        // When: extract_barrel_re_exports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "__init__.py");

        // Then: one entry with symbols=["Foo"], from_specifier="./module"
        let entry = result.iter().find(|e| e.from_specifier == "./module");
        assert!(entry.is_some(), "./module not found in {:?}", result);
        let entry = entry.unwrap();
        assert!(
            entry.symbols.contains(&"Foo".to_string()),
            "Foo not in symbols: {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-03: __all__ = ["Foo"] -> file_exports_any_symbol(["Foo"]) = true
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_03_all_exports_symbol_present() {
        // Given: a file with __all__ = ["Foo"]
        // (we use the fixture file)
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/barrel/__init__.py");

        // When: file_exports_any_symbol is called with ["Foo"]
        let extractor = PythonExtractor::new();
        let symbols = vec!["Foo".to_string()];
        let result = extractor.file_exports_any_symbol(&fixture_path, &symbols);

        // Then: returns true
        assert!(
            result,
            "expected file_exports_any_symbol to return true for Foo"
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-04: __all__ = ["Foo"] -> file_exports_any_symbol(["Bar"]) = false
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_04_all_exports_symbol_absent() {
        // Given: a file with __all__ = ["Foo"]
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/barrel/__init__.py");

        // When: file_exports_any_symbol is called with ["Bar"]
        let extractor = PythonExtractor::new();
        let symbols = vec!["Bar".to_string()];
        let result = extractor.file_exports_any_symbol(&fixture_path, &symbols);

        // Then: returns false
        assert!(
            !result,
            "expected file_exports_any_symbol to return false for Bar"
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-05: `from .module import *` extracts wildcard=true
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_05_re_export_wildcard() {
        // Given: __init__.py content with a wildcard re-export
        let source = "from .module import *\n";

        // When: extract_barrel_re_exports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "__init__.py");

        // Then: one entry with wildcard=true, from_specifier="./module", empty symbols
        let entry = result.iter().find(|e| e.from_specifier == "./module");
        assert!(entry.is_some(), "./module not found in {:?}", result);
        let entry = entry.unwrap();
        assert!(entry.wildcard, "expected wildcard=true, got {:?}", entry);
        assert!(
            entry.symbols.is_empty(),
            "expected empty symbols for wildcard, got {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-06: `from .module import Foo, Bar` extracts named (wildcard=false)
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_06_re_export_named_multi_symbol() {
        // Given: __init__.py content with multiple named re-exports
        let source = "from .module import Foo, Bar\n";

        // When: extract_barrel_re_exports is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_barrel_re_exports(source, "__init__.py");

        // Then: one entry with wildcard=false, symbols=["Foo", "Bar"]
        let entry = result.iter().find(|e| e.from_specifier == "./module");
        assert!(entry.is_some(), "./module not found in {:?}", result);
        let entry = entry.unwrap();
        assert!(
            !entry.wildcard,
            "expected wildcard=false for named re-export, got {:?}",
            entry
        );
        assert!(
            entry.symbols.contains(&"Foo".to_string()),
            "Foo not in symbols: {:?}",
            entry.symbols
        );
        assert!(
            entry.symbols.contains(&"Bar".to_string()),
            "Bar not in symbols: {:?}",
            entry.symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-07: e2e: wildcard barrel resolves imported symbol
    // test imports `from pkg import Foo`, pkg/__init__.py has `from .module import *`,
    // pkg/module.py defines Foo → mapped
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_07_e2e_wildcard_barrel_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        // pkg/__init__.py: wildcard re-export
        std::fs::write(pkg.join("__init__.py"), "from .module import *\n").unwrap();
        // pkg/module.py: defines Foo
        std::fs::write(pkg.join("module.py"), "class Foo:\n    pass\n").unwrap();
        // tests/test_foo.py: imports from pkg
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        std::fs::write(
            tests_dir.join("test_foo.py"),
            "from pkg import Foo\n\ndef test_foo():\n    assert Foo()\n",
        )
        .unwrap();

        let extractor = PythonExtractor::new();
        let module_path = pkg.join("module.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();
        let test_source = std::fs::read_to_string(&test_path).unwrap();

        let production_files = vec![module_path.clone()];
        let test_sources: HashMap<String, String> =
            [(test_path.clone(), test_source)].into_iter().collect();

        // When: map_test_files_with_imports
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: module.py is matched to test_foo.py via barrel chain
        let mapping = result.iter().find(|m| m.production_file == module_path);
        assert!(
            mapping.is_some(),
            "module.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_foo.py not matched to module.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-08: e2e: named barrel resolves imported symbol
    // test imports `from pkg import Foo`, pkg/__init__.py has `from .module import Foo`,
    // pkg/module.py defines Foo → mapped
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_08_e2e_named_barrel_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        // pkg/__init__.py: named re-export
        std::fs::write(pkg.join("__init__.py"), "from .module import Foo\n").unwrap();
        // pkg/module.py: defines Foo
        std::fs::write(pkg.join("module.py"), "class Foo:\n    pass\n").unwrap();
        // tests/test_foo.py: imports from pkg
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        std::fs::write(
            tests_dir.join("test_foo.py"),
            "from pkg import Foo\n\ndef test_foo():\n    assert Foo()\n",
        )
        .unwrap();

        let extractor = PythonExtractor::new();
        let module_path = pkg.join("module.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();
        let test_source = std::fs::read_to_string(&test_path).unwrap();

        let production_files = vec![module_path.clone()];
        let test_sources: HashMap<String, String> =
            [(test_path.clone(), test_source)].into_iter().collect();

        // When: map_test_files_with_imports
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: module.py is matched to test_foo.py via barrel chain
        let mapping = result.iter().find(|m| m.production_file == module_path);
        assert!(
            mapping.is_some(),
            "module.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_foo.py not matched to module.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-BARREL-09: e2e: wildcard barrel does NOT map non-exported symbol
    // test imports `from pkg import NonExistent`, pkg/__init__.py has `from .module import *`,
    // pkg/module.py has __all__ = ["Foo"] (does NOT export NonExistent) → NOT mapped
    // -----------------------------------------------------------------------
    #[test]
    fn py_barrel_09_e2e_wildcard_barrel_non_exported_not_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        // pkg/__init__.py: wildcard re-export
        std::fs::write(pkg.join("__init__.py"), "from .module import *\n").unwrap();
        // pkg/module.py: __all__ explicitly limits exports to Foo only
        std::fs::write(
            pkg.join("module.py"),
            "__all__ = [\"Foo\"]\n\nclass Foo:\n    pass\n\nclass NonExistent:\n    pass\n",
        )
        .unwrap();
        // tests/test_nonexistent.py: imports NonExistent from pkg
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        std::fs::write(
            tests_dir.join("test_nonexistent.py"),
            "from pkg import NonExistent\n\ndef test_ne():\n    assert NonExistent()\n",
        )
        .unwrap();

        let extractor = PythonExtractor::new();
        let module_path = pkg.join("module.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_nonexistent.py")
            .to_string_lossy()
            .into_owned();
        let test_source = std::fs::read_to_string(&test_path).unwrap();

        let production_files = vec![module_path.clone()];
        let test_sources: HashMap<String, String> =
            [(test_path.clone(), test_source)].into_iter().collect();

        // When: map_test_files_with_imports
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: module.py is NOT matched to test_nonexistent.py
        // (NonExistent is not exported by module.py)
        let mapping = result.iter().find(|m| m.production_file == module_path);
        if let Some(mapping) = mapping {
            assert!(
                !mapping.test_files.contains(&test_path),
                "test_nonexistent.py should NOT be matched to module.py: {:?}",
                mapping.test_files
            );
        }
        // If no mapping found for module.py at all, that's also correct
    }

    // -----------------------------------------------------------------------
    // PY-E2E-01: models.py + test_models.py (same dir) -> Layer 1 match
    // -----------------------------------------------------------------------
    #[test]
    fn py_e2e_01_layer1_stem_match() {
        // Given: production file models.py and test file test_models.py in the same directory
        let extractor = PythonExtractor::new();
        let production_files = vec!["e2e_pkg/models.py".to_string()];
        let test_sources: HashMap<String, String> =
            [("e2e_pkg/test_models.py".to_string(), "".to_string())]
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

        // Then: models.py is matched to test_models.py via Layer 1 (FileNameConvention)
        let mapping = result
            .iter()
            .find(|m| m.production_file == "e2e_pkg/models.py");
        assert!(
            mapping.is_some(),
            "models.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping
                .test_files
                .contains(&"e2e_pkg/test_models.py".to_string()),
            "test_models.py not in test_files: {:?}",
            mapping.test_files
        );
        assert_eq!(mapping.strategy, MappingStrategy::FileNameConvention);
    }

    // -----------------------------------------------------------------------
    // PY-E2E-02: views.py + test importing `from ..views import index` -> Layer 2 match
    // -----------------------------------------------------------------------
    #[test]
    fn py_e2e_02_layer2_import_tracing() {
        // Given: production file views.py and a test that imports from it
        let extractor = PythonExtractor::new();

        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/e2e_pkg");

        let views_path = fixture_root.join("views.py").to_string_lossy().into_owned();
        let test_views_path = fixture_root
            .join("tests/test_views.py")
            .to_string_lossy()
            .into_owned();

        let test_source =
            std::fs::read_to_string(fixture_root.join("tests/test_views.py")).unwrap_or_default();

        let production_files = vec![views_path.clone()];
        let test_sources: HashMap<String, String> = [(test_views_path.clone(), test_source)]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &fixture_root,
            false,
        );

        // Then: views.py is matched to test_views.py (Layer 2 or Layer 1)
        let mapping = result.iter().find(|m| m.production_file == views_path);
        assert!(
            mapping.is_some(),
            "views.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_views_path),
            "test_views.py not matched to views.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-E2E-03: conftest.py is excluded from mapping as helper
    // -----------------------------------------------------------------------
    #[test]
    fn py_e2e_03_conftest_excluded_as_helper() {
        // Given: conftest.py alongside test files
        let extractor = PythonExtractor::new();
        let production_files = vec!["e2e_pkg/models.py".to_string()];
        let test_sources: HashMap<String, String> = [
            ("e2e_pkg/tests/test_models.py".to_string(), "".to_string()),
            (
                "e2e_pkg/tests/conftest.py".to_string(),
                "import pytest\n".to_string(),
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

        // Then: conftest.py is NOT included in any test_files list
        for mapping in &result {
            assert!(
                !mapping.test_files.iter().any(|f| f.contains("conftest.py")),
                "conftest.py should not appear in mappings: {:?}",
                mapping
            );
        }
    }

    // -----------------------------------------------------------------------
    // Helper: setup tempdir with files and run map_test_files_with_imports
    // -----------------------------------------------------------------------

    struct ImportTestResult {
        mappings: Vec<FileMapping>,
        prod_path: String,
        test_path: String,
        _tmp: tempfile::TempDir,
    }

    /// Create a tempdir with one production file and one test file, then run
    /// `map_test_files_with_imports`. `extra_files` are written but not included
    /// in `production_files` or `test_sources` (e.g. `__init__.py`).
    fn run_import_test(
        prod_rel: &str,
        prod_content: &str,
        test_rel: &str,
        test_content: &str,
        extra_files: &[(&str, &str)],
    ) -> ImportTestResult {
        let tmp = tempfile::tempdir().unwrap();

        // Write extra files first (e.g. __init__.py)
        for (rel, content) in extra_files {
            let path = tmp.path().join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content).unwrap();
        }

        // Write production file
        let prod_abs = tmp.path().join(prod_rel);
        if let Some(parent) = prod_abs.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&prod_abs, prod_content).unwrap();

        // Write test file
        let test_abs = tmp.path().join(test_rel);
        if let Some(parent) = test_abs.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&test_abs, test_content).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = prod_abs.to_string_lossy().into_owned();
        let test_path = test_abs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        let mappings = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        ImportTestResult {
            mappings,
            prod_path,
            test_path,
            _tmp: tmp,
        }
    }

    // -----------------------------------------------------------------------
    // PY-ABS-01: `from models.cars import Car` -> mapped to models/cars.py via Layer 2
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_01_absolute_import_nested_module() {
        // Given: `from models.cars import Car` in tests/unit/test_car.py,
        //        models/cars.py exists at scan_root
        let r = run_import_test(
            "models/cars.py",
            "class Car:\n    pass\n",
            "tests/unit/test_car.py",
            "from models.cars import Car\n\ndef test_car():\n    pass\n",
            &[],
        );

        // Then: models/cars.py is mapped to test_car.py via Layer 2 (ImportTracing)
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "models/cars.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_car.py not in test_files for models/cars.py: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "expected ImportTracing strategy, got {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // PY-ABS-02: `from utils.publish_state import ...` -> mapped to utils/publish_state.py
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_02_absolute_import_utils_module() {
        // Given: `from utils.publish_state import PublishState` in tests/test_pub.py,
        //        utils/publish_state.py exists at scan_root
        let r = run_import_test(
            "utils/publish_state.py",
            "class PublishState:\n    pass\n",
            "tests/test_pub.py",
            "from utils.publish_state import PublishState\n\ndef test_pub():\n    pass\n",
            &[],
        );

        // Then: utils/publish_state.py is mapped to test_pub.py via Layer 2
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "utils/publish_state.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_pub.py not in test_files for utils/publish_state.py: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "expected ImportTracing strategy, got {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // PY-ABS-03: relative import `from .models import X` -> resolves from from_file parent
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_03_relative_import_still_resolves() {
        // Given: `from .models import X` in pkg/test_something.py,
        //        pkg/models.py exists relative to test file
        // Note: production file must NOT be inside tests/ dir (Phase 20: tests/ files are helpers)
        let r = run_import_test(
            "pkg/models.py",
            "class X:\n    pass\n",
            "pkg/test_something.py",
            "from .models import X\n\ndef test_x():\n    pass\n",
            &[],
        );

        // Then: models.py is mapped to test_something.py (relative import resolves from parent dir)
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "pkg/models.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_something.py not in test_files for pkg/models.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-STEM-07: _decoders.py -> production_stem strips single leading underscore
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_07_production_stem_single_underscore_prefix() {
        // Given: production file path "httpx/_decoders.py"
        // When: production_stem() is called
        // Then: returns Some("decoders") (single leading underscore stripped)
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("httpx/_decoders.py");
        assert_eq!(result, Some("decoders"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-08: __version__.py -> production_stem strips only one underscore
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_08_production_stem_double_underscore_strips_one() {
        // Given: production file path "httpx/__version__.py"
        // When: production_stem() is called
        // Then: returns Some("_version") (only one leading underscore stripped, not __init__ which returns None)
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("httpx/__version__.py");
        assert_eq!(result, Some("_version"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-09: decoders.py -> production_stem unchanged (regression)
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_09_production_stem_no_prefix_regression() {
        // Given: production file path "httpx/decoders.py" (no underscore prefix)
        // When: production_stem() is called
        // Then: returns Some("decoders") (unchanged, no regression)
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("httpx/decoders.py");
        assert_eq!(result, Some("decoders"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-10: ___triple.py -> production_stem strips one underscore
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_10_production_stem_triple_underscore() {
        // Given: production file path "pkg/___triple.py"
        // When: production_stem() is called
        // Then: returns Some("__triple") (one leading underscore stripped)
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("pkg/___triple.py");
        assert_eq!(result, Some("__triple"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-11: ___foo__.py -> strip_prefix + strip_suffix chained
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_11_production_stem_prefix_and_suffix_chained() {
        // Given: production file path "pkg/___foo__.py"
        // When: production_stem() is called
        // Then: returns Some("__foo") (strip_prefix('_') -> "__foo__", strip_suffix("__") -> "__foo")
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("pkg/___foo__.py");
        assert_eq!(result, Some("__foo"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-12: __foo__.py -> strip_prefix + strip_suffix (double underscore prefix)
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_12_production_stem_dunder_prefix_and_suffix() {
        // Given: production file path "pkg/__foo__.py"
        // When: production_stem() is called
        // Then: returns Some("_foo") (strip_prefix('_') -> "_foo__", strip_suffix("__") -> "_foo")
        let extractor = PythonExtractor::new();
        let result = extractor.production_stem("pkg/__foo__.py");
        assert_eq!(result, Some("_foo"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-13: test_stem("app/tests.py") -> Some("app")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_13_tests_file_with_parent_dir() {
        // Given: path = "app/tests.py"
        // When: test_stem(path)
        // Then: Some("app") (parent directory name used as stem)
        let result = test_stem("app/tests.py");
        assert_eq!(result, Some("app"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-14: test_stem("tests/aggregation/tests.py") -> Some("aggregation")
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_14_tests_file_with_nested_parent_dir() {
        // Given: path = "tests/aggregation/tests.py"
        // When: test_stem(path)
        // Then: Some("aggregation") (immediate parent directory name used as stem)
        let result = test_stem("tests/aggregation/tests.py");
        assert_eq!(result, Some("aggregation"));
    }

    // -----------------------------------------------------------------------
    // PY-STEM-15: test_stem("tests.py") -> None (no parent dir)
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_15_tests_file_no_parent_dir() {
        // Given: path = "tests.py" (no parent directory component)
        // When: test_stem(path)
        // Then: None (no parent dir to derive stem from, defer to Layer 2)
        let result = test_stem("tests.py");
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // PY-STEM-16: production_stem("app/tests.py") -> None
    // -----------------------------------------------------------------------
    #[test]
    fn py_stem_16_production_stem_excludes_tests_file() {
        // Given: path = "app/tests.py"
        // When: production_stem(path)
        // Then: None (tests.py must not appear in production_files)
        let result = production_stem("app/tests.py");
        assert_eq!(result, None);
    }

    // -----------------------------------------------------------------------
    // PY-SRCLAYOUT-01: src/ layout absolute import resolved
    // -----------------------------------------------------------------------
    #[test]
    fn py_srclayout_01_src_layout_absolute_import_resolved() {
        // Given: tempdir with "src/mypackage/__init__.py" + "src/mypackage/sessions.py"
        //        and test file "tests/test_sessions.py" containing "from mypackage.sessions import Session"
        let r = run_import_test(
            "src/mypackage/sessions.py",
            "class Session:\n    pass\n",
            "tests/test_sessions.py",
            "from mypackage.sessions import Session\n\ndef test_session():\n    pass\n",
            &[("src/mypackage/__init__.py", "")],
        );

        // Then: sessions.py is in test_files for test_sessions.py.
        // Layer 1 core does not match because prod dir (src/mypackage) != test dir (tests),
        // but stem-only fallback matches via stem "sessions" (cross-directory).
        // Strategy remains FileNameConvention (L1 fallback is still L1).
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "src/mypackage/sessions.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_sessions.py not in test_files for sessions.py (src/ layout): {:?}",
            mapping.test_files
        );
        assert_eq!(mapping.strategy, MappingStrategy::FileNameConvention);
    }

    // -----------------------------------------------------------------------
    // PY-SRCLAYOUT-02: non-src layout still works (regression)
    // -----------------------------------------------------------------------
    #[test]
    fn py_srclayout_02_non_src_layout_regression() {
        // Given: tempdir with "mypackage/sessions.py"
        //        and test file "tests/test_sessions.py" containing "from mypackage.sessions import Session"
        let r = run_import_test(
            "mypackage/sessions.py",
            "class Session:\n    pass\n",
            "tests/test_sessions.py",
            "from mypackage.sessions import Session\n\ndef test_session():\n    pass\n",
            &[],
        );

        // Then: sessions.py is in test_files for test_sessions.py (non-src layout still works).
        // Layer 1 core does not match because prod dir (mypackage) != test dir (tests),
        // but stem-only fallback matches via stem "sessions" (cross-directory).
        // Strategy remains FileNameConvention (L1 fallback is still L1).
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "mypackage/sessions.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_sessions.py not in test_files for sessions.py (non-src layout): {:?}",
            mapping.test_files
        );
        assert_eq!(mapping.strategy, MappingStrategy::FileNameConvention);
    }

    // -----------------------------------------------------------------------
    // PY-ABS-04: `from nonexistent.module import X` -> no mapping added (graceful skip)
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_04_nonexistent_absolute_import_skipped() {
        // Given: `from nonexistent.module import X` in test file,
        //        nonexistent/module.py does NOT exist at scan_root.
        //        models/real.py exists as production file but is NOT imported.
        let r = run_import_test(
            "models/real.py",
            "class Real:\n    pass\n",
            "tests/test_missing.py",
            "from nonexistent.module import X\n\ndef test_x():\n    pass\n",
            &[],
        );

        // Then: test_missing.py is NOT mapped to models/real.py (unresolvable import skipped)
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        if let Some(mapping) = mapping {
            assert!(
                !mapping.test_files.contains(&r.test_path),
                "test_missing.py should NOT be mapped to models/real.py: {:?}",
                mapping.test_files
            );
        }
        // passing if no mapping or test_path not in mapping
    }

    // -----------------------------------------------------------------------
    // PY-ABS-05: absolute import in test file maps to production file outside tests/
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_05_mixed_absolute_and_relative_imports() {
        // Given: a test file with `from models.cars import Car` (absolute),
        //        models/cars.py exists at scan_root,
        //        tests/helpers.py also exists but is a test helper (Phase 20: excluded)
        let tmp = tempfile::tempdir().unwrap();
        let models_dir = tmp.path().join("models");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        let cars_py = models_dir.join("cars.py");
        std::fs::write(&cars_py, "class Car:\n    pass\n").unwrap();

        let helpers_py = tests_dir.join("helpers.py");
        std::fs::write(&helpers_py, "def setup():\n    pass\n").unwrap();

        let test_py = tests_dir.join("test_mixed.py");
        let test_source =
            "from models.cars import Car\nfrom .helpers import setup\n\ndef test_mixed():\n    pass\n";
        std::fs::write(&test_py, test_source).unwrap();

        let extractor = PythonExtractor::new();
        let cars_prod = cars_py.to_string_lossy().into_owned();
        let helpers_prod = helpers_py.to_string_lossy().into_owned();
        let test_path = test_py.to_string_lossy().into_owned();

        let production_files = vec![cars_prod.clone(), helpers_prod.clone()];
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

        // Then: models/cars.py is mapped via absolute import (Layer 2)
        let cars_mapping = result.iter().find(|m| m.production_file == cars_prod);
        assert!(
            cars_mapping.is_some(),
            "models/cars.py not found in mappings: {:?}",
            result
        );
        let cars_m = cars_mapping.unwrap();
        assert!(
            cars_m.test_files.contains(&test_path),
            "test_mixed.py not mapped to models/cars.py via absolute import: {:?}",
            cars_m.test_files
        );

        // Then: tests/helpers.py should NOT appear in mappings (Phase 20: tests/ dir files are helpers)
        let helpers_mapping = result.iter().find(|m| m.production_file == helpers_prod);
        assert!(
            helpers_mapping.is_none(),
            "tests/helpers.py should be excluded as test helper (Phase 20), but found in mappings: {:?}",
            helpers_mapping
        );
    }

    // -----------------------------------------------------------------------
    // PY-REL-01: `from .. import X` bare two-dot relative import
    // -----------------------------------------------------------------------
    #[test]
    fn py_rel_01_bare_two_dot_relative_import() {
        // Given: `from .. import utils` in pkg/sub/test_thing.py,
        //        pkg/utils.py exists (parent of parent)
        let r = run_import_test(
            "pkg/utils.py",
            "def helper():\n    pass\n",
            "pkg/sub/test_thing.py",
            "from .. import utils\n\ndef test_thing():\n    pass\n",
            &[],
        );

        // Then: pkg/utils.py is mapped via bare relative import (is_bare_relative=true path)
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "pkg/utils.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_thing.py not in test_files for pkg/utils.py via bare two-dot import: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-L2-DJANGO-01: Django layout tests.py mapped via Layer 2
    // -----------------------------------------------------------------------
    #[test]
    fn py_l2_django_01_tests_file_mapped_via_import_tracing() {
        // Given: tempdir with src/models.py (production) and app/tests.py (test)
        //        app/tests.py contains `from src.models import Model`
        let r = run_import_test(
            "src/models.py",
            "class Model:\n    pass\n",
            "app/tests.py",
            "from src.models import Model\n\n\ndef test_model():\n    pass\n",
            &[],
        );

        // Then: src/models.py is mapped to app/tests.py via ImportTracing strategy
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "src/models.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "app/tests.py not in test_files for src/models.py: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "expected ImportTracing strategy, got {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // TC-01: Django project with project/manage.py and from app.models import X
    //        -> test maps to project/app/models.py via manage.py root fallback
    // -----------------------------------------------------------------------
    #[test]
    fn py_l2_django_managepy_root_tc01_subdirectory_layout() {
        // Given: Django project with project/manage.py, project/app/models.py
        //        and a test file with `from app.models import MyModel`
        let tmp = tempfile::tempdir().unwrap();

        let manage_py_path = tmp.path().join("project").join("manage.py");
        std::fs::create_dir_all(manage_py_path.parent().unwrap()).unwrap();
        std::fs::write(&manage_py_path, "#!/usr/bin/env python\n").unwrap();

        let prod_rel = "project/app/models.py";
        let prod_abs = tmp.path().join(prod_rel);
        std::fs::create_dir_all(prod_abs.parent().unwrap()).unwrap();
        std::fs::write(&prod_abs, "class MyModel:\n    pass\n").unwrap();

        let test_rel = "tests/test_models.py";
        let test_abs = tmp.path().join(test_rel);
        std::fs::create_dir_all(test_abs.parent().unwrap()).unwrap();
        let test_content = "from app.models import MyModel\n\ndef test_mymodel():\n    pass\n";
        std::fs::write(&test_abs, test_content).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = prod_abs.to_string_lossy().into_owned();
        let test_path = test_abs.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: observe runs
        let mappings = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            tmp.path(),
            false,
        );

        // Then: test maps to project/app/models.py via manage.py root fallback
        let mapping = mappings.iter().find(|m| m.production_file == prod_path);
        assert!(
            mapping.is_some(),
            "project/app/models.py not found in mappings: {:?}",
            mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "tests/test_models.py not in test_files for project/app/models.py: {:?}",
            mapping.test_files
        );
        assert_eq!(
            mapping.strategy,
            MappingStrategy::ImportTracing,
            "expected ImportTracing strategy, got {:?}",
            mapping.strategy
        );
    }

    // -----------------------------------------------------------------------
    // TC-03: find_manage_py_root returns None when manage.py is at scan_root itself
    // -----------------------------------------------------------------------
    #[test]
    fn py_l2_django_managepy_root_tc03_at_scan_root_returns_none() {
        // Given: manage.py at scan_root itself (not in a subdirectory)
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("manage.py"), "#!/usr/bin/env python\n").unwrap();

        // When: find_manage_py_root is called with scan_root
        let result = find_manage_py_root(tmp.path());

        // Then: returns None (manage.py at scan_root is already covered by canonical_root)
        assert!(
            result.is_none(),
            "expected None when manage.py is at scan_root, got {:?}",
            result
        );
    }
}

// ---------------------------------------------------------------------------
// Route extraction
// ---------------------------------------------------------------------------

const ROUTE_DECORATOR_QUERY: &str = include_str!("../queries/route_decorator.scm");
static ROUTE_DECORATOR_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

/// A route extracted from a FastAPI application.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub http_method: String,
    pub path: String,
    pub handler_name: String,
    pub file: String,
}

/// Extract `router = APIRouter(prefix="...")` assignments from source.
/// Returns a HashMap from variable name to prefix string.
fn collect_router_prefixes(
    source_bytes: &[u8],
    tree: &tree_sitter::Tree,
) -> HashMap<String, String> {
    let mut prefixes = HashMap::new();

    // Walk the tree to find: assignment where right side is APIRouter(prefix="...")
    let root = tree.root_node();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.kind() == "assignment" {
            let left = node.child_by_field_name("left");
            let right = node.child_by_field_name("right");

            if let (Some(left_node), Some(right_node)) = (left, right) {
                if left_node.kind() == "identifier" && right_node.kind() == "call" {
                    let var_name = left_node.utf8_text(source_bytes).unwrap_or("").to_string();

                    // Check if the call is APIRouter(...)
                    let fn_node = right_node.child_by_field_name("function");
                    let is_api_router = fn_node
                        .and_then(|f| f.utf8_text(source_bytes).ok())
                        .map(|name| name == "APIRouter")
                        .unwrap_or(false);

                    if is_api_router {
                        // Look for prefix keyword argument
                        let args_node = right_node.child_by_field_name("arguments");
                        if let Some(args) = args_node {
                            let mut args_cursor = args.walk();
                            for arg in args.named_children(&mut args_cursor) {
                                if arg.kind() == "keyword_argument" {
                                    let kw_name = arg
                                        .child_by_field_name("name")
                                        .and_then(|n| n.utf8_text(source_bytes).ok())
                                        .unwrap_or("");
                                    if kw_name == "prefix" {
                                        if let Some(val) = arg.child_by_field_name("value") {
                                            if val.kind() == "string" {
                                                let raw = val.utf8_text(source_bytes).unwrap_or("");
                                                let prefix = strip_string_quotes(raw);
                                                prefixes.insert(var_name.clone(), prefix);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // If no prefix found, insert empty string (APIRouter() without prefix)
                        prefixes.entry(var_name).or_default();
                    }
                }
            }
        }

        // Push children in reverse order so they are popped in source order (DFS)
        let mut w = node.walk();
        let children: Vec<_> = node.named_children(&mut w).collect();
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }

    prefixes
}

/// Strip surrounding quotes from a Python string literal.
/// `"'/users'"` → `"/users"`, `'"hello"'` → `"hello"`, triple-quoted too.
/// Also handles Python string prefixes: r"...", b"...", f"...", u"...", rb"...", etc.
///
/// Precondition: `raw` must be a tree-sitter `string` node text (always includes quotes after prefix).
fn strip_string_quotes(raw: &str) -> String {
    // Strip Python string prefix characters (r, b, f, u and combinations thereof).
    // Safe because tree-sitter string nodes always have surrounding quotes after the prefix.
    let raw = raw.trim_start_matches(|c: char| "rRbBfFuU".contains(c));
    // Try triple quotes first
    for q in &[r#"""""#, "'''"] {
        if let Some(inner) = raw.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    // Single quotes
    for q in &["\"", "'"] {
        if let Some(inner) = raw.strip_prefix(q).and_then(|s| s.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    raw.to_string()
}

/// Extract FastAPI routes from Python source code.
pub fn extract_routes(source: &str, file_path: &str) -> Vec<Route> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut parser = PythonExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let source_bytes = source.as_bytes();

    // Pass 1: collect APIRouter prefix assignments
    let router_prefixes = collect_router_prefixes(source_bytes, &tree);

    // Pass 2: run route_decorator query
    let query = cached_query(&ROUTE_DECORATOR_QUERY_CACHE, ROUTE_DECORATOR_QUERY);

    let obj_idx = query.capture_index_for_name("route.object");
    let method_idx = query.capture_index_for_name("route.method");
    let path_idx = query.capture_index_for_name("route.path");
    let handler_idx = query.capture_index_for_name("route.handler");

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

    let mut routes = Vec::new();
    let mut seen = HashSet::new();

    while let Some(m) = matches.next() {
        let mut obj: Option<String> = None;
        let mut method: Option<String> = None;
        let mut path_raw: Option<String> = None;
        let mut path_is_string = false;
        let mut handler: Option<String> = None;

        for cap in m.captures {
            let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
            if obj_idx == Some(cap.index) {
                obj = Some(text);
            } else if method_idx == Some(cap.index) {
                method = Some(text);
            } else if path_idx == Some(cap.index) {
                // Determine if it's a string literal or identifier
                path_is_string = cap.node.kind() == "string";
                path_raw = Some(text);
            } else if handler_idx == Some(cap.index) {
                handler = Some(text);
            }
        }

        let (obj, method, handler) = match (obj, method, handler) {
            (Some(o), Some(m), Some(h)) => (o, m, h),
            _ => continue,
        };

        // Filter: method must be a known HTTP method
        if !HTTP_METHODS.contains(&method.as_str()) {
            continue;
        }

        // Resolve path
        let sub_path = match path_raw {
            Some(ref raw) if path_is_string => strip_string_quotes(raw),
            Some(_) => "<dynamic>".to_string(),
            None => "<dynamic>".to_string(),
        };

        // Resolve prefix from router variable
        let prefix = router_prefixes.get(&obj).map(|s| s.as_str()).unwrap_or("");
        let full_path = if prefix.is_empty() {
            sub_path
        } else {
            format!("{prefix}{sub_path}")
        };

        // Deduplicate: same (method, path, handler)
        let key = (method.clone(), full_path.clone(), handler.clone());
        if !seen.insert(key) {
            continue;
        }

        routes.push(Route {
            http_method: method.to_uppercase(),
            path: full_path,
            handler_name: handler,
            file: file_path.to_string(),
        });
    }

    routes
}

// ---------------------------------------------------------------------------
// Route extraction tests (FA-RT-01 ~ FA-RT-10)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod route_tests {
    use super::*;

    // FA-RT-01: basic @app.get route
    #[test]
    fn fa_rt_01_basic_app_get_route() {
        // Given: source with `@app.get("/users") def read_users(): ...`
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/users")
def read_users():
    return []
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: [Route { method: "GET", path: "/users", handler: "read_users" }]
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].http_method, "GET");
        assert_eq!(routes[0].path, "/users");
        assert_eq!(routes[0].handler_name, "read_users");
    }

    // FA-RT-02: multiple HTTP methods
    #[test]
    fn fa_rt_02_multiple_http_methods() {
        // Given: source with @app.get, @app.post, @app.put, @app.delete on separate functions
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/items")
def list_items():
    return []

@app.post("/items")
def create_item():
    return {}

@app.put("/items/{item_id}")
def update_item(item_id: int):
    return {}

@app.delete("/items/{item_id}")
def delete_item(item_id: int):
    return {}
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: 4 routes with correct methods
        assert_eq!(routes.len(), 4, "expected 4 routes, got {:?}", routes);
        let methods: Vec<&str> = routes.iter().map(|r| r.http_method.as_str()).collect();
        assert!(methods.contains(&"GET"), "missing GET");
        assert!(methods.contains(&"POST"), "missing POST");
        assert!(methods.contains(&"PUT"), "missing PUT");
        assert!(methods.contains(&"DELETE"), "missing DELETE");
    }

    // FA-RT-03: path parameter
    #[test]
    fn fa_rt_03_path_parameter() {
        // Given: `@app.get("/items/{item_id}")`
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/items/{item_id}")
def read_item(item_id: int):
    return {}
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: path = "/items/{item_id}"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "/items/{item_id}");
    }

    // FA-RT-04: @router.get with APIRouter prefix
    #[test]
    fn fa_rt_04_router_get_with_prefix() {
        // Given: `router = APIRouter(prefix="/items")` + `@router.get("/{item_id}")`
        let source = r#"
from fastapi import APIRouter

router = APIRouter(prefix="/items")

@router.get("/{item_id}")
def read_item(item_id: int):
    return {}
"#;

        // When: extract_routes(source, "routes.py")
        let routes = extract_routes(source, "routes.py");

        // Then: path = "/items/{item_id}"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(
            routes[0].path, "/items/{item_id}",
            "expected prefix-resolved path"
        );
    }

    // FA-RT-05: @router.get without prefix
    #[test]
    fn fa_rt_05_router_get_without_prefix() {
        // Given: `router = APIRouter()` + `@router.get("/health")`
        let source = r#"
from fastapi import APIRouter

router = APIRouter()

@router.get("/health")
def health_check():
    return {"status": "ok"}
"#;

        // When: extract_routes(source, "routes.py")
        let routes = extract_routes(source, "routes.py");

        // Then: path = "/health"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "/health");
    }

    // FA-RT-06: non-route decorator ignored
    #[test]
    fn fa_rt_06_non_route_decorator_ignored() {
        // Given: `@pytest.fixture` or `@staticmethod` decorated function
        let source = r#"
import pytest

@pytest.fixture
def client():
    return None

class MyClass:
    @staticmethod
    def helper():
        pass
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: empty Vec
        assert!(
            routes.is_empty(),
            "expected no routes for non-route decorators, got {:?}",
            routes
        );
    }

    // FA-RT-07: dynamic path (non-literal)
    #[test]
    fn fa_rt_07_dynamic_path_non_literal() {
        // Given: `@app.get(some_variable)`
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

ROUTE_PATH = "/dynamic"

@app.get(ROUTE_PATH)
def dynamic_route():
    return {}
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: path = "<dynamic>"
        assert_eq!(
            routes.len(),
            1,
            "expected 1 route for dynamic path, got {:?}",
            routes
        );
        assert_eq!(
            routes[0].path, "<dynamic>",
            "expected <dynamic> for non-literal path argument"
        );
    }

    // FA-RT-08: empty source
    #[test]
    fn fa_rt_08_empty_source() {
        // Given: ""
        let source = "";

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: empty Vec
        assert!(routes.is_empty(), "expected empty Vec for empty source");
    }

    // FA-RT-09: async def handler
    #[test]
    fn fa_rt_09_async_def_handler() {
        // Given: `@app.get("/") async def root(): ...`
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/")
async def root():
    return {"message": "hello"}
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: handler = "root" (async は無視)
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(
            routes[0].handler_name, "root",
            "async def should produce handler_name = 'root'"
        );
    }

    // FA-RT-10: multiple decorators on same function
    #[test]
    fn fa_rt_10_multiple_decorators_on_same_function() {
        // Given: `@app.get("/") @require_auth def root(): ...`
        let source = r#"
from fastapi import FastAPI
app = FastAPI()

def require_auth(func):
    return func

@app.get("/")
@require_auth
def root():
    return {}
"#;

        // When: extract_routes(source, "main.py")
        let routes = extract_routes(source, "main.py");

        // Then: 1 route (non-route decorators ignored)
        assert_eq!(
            routes.len(),
            1,
            "expected exactly 1 route (non-route decorators ignored), got {:?}",
            routes
        );
        assert_eq!(routes[0].http_method, "GET");
        assert_eq!(routes[0].path, "/");
        assert_eq!(routes[0].handler_name, "root");
    }
}

// ---------------------------------------------------------------------------
// Django URL conf route extraction
// ---------------------------------------------------------------------------

const DJANGO_URL_PATTERN_QUERY: &str = include_str!("../queries/django_url_pattern.scm");
static DJANGO_URL_PATTERN_QUERY_CACHE: OnceLock<Query> = OnceLock::new();

static DJANGO_PATH_RE: OnceLock<regex::Regex> = OnceLock::new();
static DJANGO_RE_PATH_RE: OnceLock<regex::Regex> = OnceLock::new();

const HTTP_METHOD_ANY: &str = "ANY";

/// Normalize a Django `path()` URL pattern to Express-style `:param` notation.
/// `"users/<int:pk>/"` → `"users/:pk/"`
/// `"users/<pk>/"` → `"users/:pk/"`
pub fn normalize_django_path(path: &str) -> String {
    let re = DJANGO_PATH_RE
        .get_or_init(|| regex::Regex::new(r"<(?:\w+:)?(\w+)>").expect("invalid regex"));
    re.replace_all(path, ":$1").into_owned()
}

/// Normalize a Django `re_path()` URL pattern.
/// Strips leading `^` / trailing `$` anchors and converts `(?P<name>...)` to `:name`.
pub fn normalize_re_path(path: &str) -> String {
    // Strip leading ^ (only if the very first character is ^)
    let s = path.strip_prefix('^').unwrap_or(path);
    // Strip trailing $ (only if the very last character is $)
    let s = s.strip_suffix('$').unwrap_or(s);
    // Replace (?P<name>...) named groups with :name.
    // Note: `[^)]*` correctly handles typical Django patterns like `(?P<year>[0-9]{4})`.
    // Known limitation: nested parentheses inside a named group (e.g., `(?P<slug>(?:foo|bar))`)
    // will not match because `[^)]*` stops at the first `)`. Such patterns are extremely rare
    // in Django URL confs and are left as a known constraint.
    let re = DJANGO_RE_PATH_RE
        .get_or_init(|| regex::Regex::new(r"\(\?P<(\w+)>[^)]*\)").expect("invalid regex"));
    re.replace_all(s, ":$1").into_owned()
}

/// Extract Django URL conf routes from Python source code.
pub fn extract_django_routes(source: &str, file_path: &str) -> Vec<Route> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut parser = PythonExtractor::parser();
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let source_bytes = source.as_bytes();

    let query = cached_query(&DJANGO_URL_PATTERN_QUERY_CACHE, DJANGO_URL_PATTERN_QUERY);

    let func_idx = query.capture_index_for_name("django.func");
    let path_idx = query.capture_index_for_name("django.path");
    let handler_idx = query.capture_index_for_name("django.handler");

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

    let mut routes = Vec::new();
    let mut seen = HashSet::new();

    while let Some(m) = matches.next() {
        let mut func: Option<String> = None;
        let mut path_raw: Option<String> = None;
        let mut handler: Option<String> = None;

        for cap in m.captures {
            let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
            if func_idx == Some(cap.index) {
                func = Some(text);
            } else if path_idx == Some(cap.index) {
                path_raw = Some(text);
            } else if handler_idx == Some(cap.index) {
                handler = Some(text);
            }
        }

        let (func, path_raw, handler) = match (func, path_raw, handler) {
            (Some(f), Some(p), Some(h)) => (f, p, h),
            _ => continue,
        };

        let raw_path = strip_string_quotes(&path_raw);
        let normalized = match func.as_str() {
            "re_path" => normalize_re_path(&raw_path),
            _ => normalize_django_path(&raw_path),
        };

        // Deduplicate: same (method, path, handler)
        let key = (
            HTTP_METHOD_ANY.to_string(),
            normalized.clone(),
            handler.clone(),
        );
        if !seen.insert(key) {
            continue;
        }

        routes.push(Route {
            http_method: HTTP_METHOD_ANY.to_string(),
            path: normalized,
            handler_name: handler,
            file: file_path.to_string(),
        });
    }

    routes
}

// ---------------------------------------------------------------------------
// Django route extraction tests (DJ-NP-*, DJ-NR-*, DJ-RT-*, DJ-RT-E2E-*)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod django_route_tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Unit: normalize_django_path
    // -----------------------------------------------------------------------

    // DJ-NP-01: typed parameter
    #[test]
    fn dj_np_01_typed_parameter() {
        // Given: a Django path with a typed parameter "users/<int:pk>/"
        // When: normalize_django_path is called
        // Then: returns "users/:pk/"
        let result = normalize_django_path("users/<int:pk>/");
        assert_eq!(result, "users/:pk/");
    }

    // DJ-NP-02: untyped parameter
    #[test]
    fn dj_np_02_untyped_parameter() {
        // Given: a Django path with an untyped parameter "users/<pk>/"
        // When: normalize_django_path is called
        // Then: returns "users/:pk/"
        let result = normalize_django_path("users/<pk>/");
        assert_eq!(result, "users/:pk/");
    }

    // DJ-NP-03: multiple parameters
    #[test]
    fn dj_np_03_multiple_parameters() {
        // Given: a Django path with multiple parameters
        // When: normalize_django_path is called
        // Then: returns "posts/:slug/comments/:id/"
        let result = normalize_django_path("posts/<slug:slug>/comments/<int:id>/");
        assert_eq!(result, "posts/:slug/comments/:id/");
    }

    // DJ-NP-04: no parameters
    #[test]
    fn dj_np_04_no_parameters() {
        // Given: a Django path with no parameters "users/"
        // When: normalize_django_path is called
        // Then: returns "users/" unchanged
        let result = normalize_django_path("users/");
        assert_eq!(result, "users/");
    }

    // -----------------------------------------------------------------------
    // Unit: normalize_re_path
    // -----------------------------------------------------------------------

    // DJ-NR-01: single named group
    #[test]
    fn dj_nr_01_single_named_group() {
        // Given: a re_path pattern with one named group
        // When: normalize_re_path is called
        // Then: returns "articles/:year/"
        let result = normalize_re_path("^articles/(?P<year>[0-9]{4})/$");
        assert_eq!(result, "articles/:year/");
    }

    // DJ-NR-02: multiple named groups
    #[test]
    fn dj_nr_02_multiple_named_groups() {
        // Given: a re_path pattern with multiple named groups
        // When: normalize_re_path is called
        // Then: returns ":year/:month/"
        let result = normalize_re_path("^(?P<year>[0-9]{4})/(?P<month>[0-9]{2})/$");
        assert_eq!(result, ":year/:month/");
    }

    // DJ-NR-03: no named groups
    #[test]
    fn dj_nr_03_no_named_groups() {
        // Given: a re_path pattern with no named groups
        // When: normalize_re_path is called
        // Then: anchor stripped → "users/"
        let result = normalize_re_path("^users/$");
        assert_eq!(result, "users/");
    }

    // DJ-NR-04: ^ inside character class must not be stripped
    #[test]
    fn dj_nr_04_character_class_caret_preserved() {
        // Given: a re_path pattern with ^ inside a character class [^/]+
        // When: normalize_re_path is called
        // Then: the ^ inside [] is NOT treated as an anchor: "items/[^/]+/"
        let result = normalize_re_path("^items/[^/]+/$");
        assert_eq!(result, "items/[^/]+/");
    }

    // -----------------------------------------------------------------------
    // Unit: extract_django_routes
    // -----------------------------------------------------------------------

    // DJ-RT-01: basic path() with attribute handler (views.user_list)
    #[test]
    fn dj_rt_01_basic_path_attribute_handler() {
        // Given: urlpatterns with path("users/", views.user_list)
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("users/", views.user_list),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: 1 route, method="ANY", path="users/", handler="user_list"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].http_method, "ANY");
        assert_eq!(routes[0].path, "users/");
        assert_eq!(routes[0].handler_name, "user_list");
    }

    // DJ-RT-02: path() with direct import handler
    #[test]
    fn dj_rt_02_path_direct_import_handler() {
        // Given: urlpatterns with path("users/", user_list) — direct function import
        let source = r#"
from django.urls import path
from .views import user_list

urlpatterns = [
    path("users/", user_list),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: 1 route, method="ANY", path="users/", handler="user_list"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].http_method, "ANY");
        assert_eq!(routes[0].path, "users/");
        assert_eq!(routes[0].handler_name, "user_list");
    }

    // DJ-RT-03: path() with typed parameter
    #[test]
    fn dj_rt_03_path_typed_parameter() {
        // Given: path("users/<int:pk>/", views.user_detail)
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("users/<int:pk>/", views.user_detail),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: path = "users/:pk/"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "users/:pk/");
    }

    // DJ-RT-04: path() with untyped parameter
    #[test]
    fn dj_rt_04_path_untyped_parameter() {
        // Given: path("users/<pk>/", views.user_detail)
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("users/<pk>/", views.user_detail),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: path = "users/:pk/"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "users/:pk/");
    }

    // DJ-RT-05: re_path() with named group
    #[test]
    fn dj_rt_05_re_path_named_group() {
        // Given: re_path("^articles/(?P<year>[0-9]{4})/$", views.year_archive)
        let source = r#"
from django.urls import re_path
from . import views

urlpatterns = [
    re_path(r"^articles/(?P<year>[0-9]{4})/$", views.year_archive),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: path = "articles/:year/"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "articles/:year/");
    }

    // DJ-RT-06: multiple routes — all method "ANY"
    #[test]
    fn dj_rt_06_multiple_routes() {
        // Given: 3 path() entries in urlpatterns
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("users/", views.user_list),
    path("users/<int:pk>/", views.user_detail),
    path("about/", views.about),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: 3 routes, all method "ANY"
        assert_eq!(routes.len(), 3, "expected 3 routes, got {:?}", routes);
        for r in &routes {
            assert_eq!(r.http_method, "ANY", "expected method ANY for {:?}", r);
        }
    }

    // DJ-RT-07: path() with name kwarg — name kwarg ignored, handler captured
    #[test]
    fn dj_rt_07_path_with_name_kwarg() {
        // Given: path("login/", views.login_view, name="login")
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("login/", views.login_view, name="login"),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: 1 route, handler = "login_view" (name kwarg ignored)
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].handler_name, "login_view");
    }

    // DJ-RT-08: empty source
    #[test]
    fn dj_rt_08_empty_source() {
        // Given: ""
        // When: extract_django_routes is called
        let routes = extract_django_routes("", "urls.py");

        // Then: empty Vec
        assert!(routes.is_empty(), "expected empty Vec for empty source");
    }

    // DJ-RT-09: no path/re_path calls
    #[test]
    fn dj_rt_09_no_path_calls() {
        // Given: source with no path() or re_path() calls
        let source = r#"
from django.db import models

class User(models.Model):
    name = models.CharField(max_length=100)
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "models.py");

        // Then: empty Vec
        assert!(
            routes.is_empty(),
            "expected empty Vec for non-URL source, got {:?}",
            routes
        );
    }

    // DJ-RT-10: deduplication — same (path, handler) appears twice → 1 route
    #[test]
    fn dj_rt_10_deduplication() {
        // Given: two identical path() entries
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("users/", views.user_list),
    path("users/", views.user_list),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: 1 route (deduplicated)
        assert_eq!(
            routes.len(),
            1,
            "expected 1 route after dedup, got {:?}",
            routes
        );
    }

    // DJ-RT-11: include() is ignored
    #[test]
    fn dj_rt_11_include_is_ignored() {
        // Given: urlpatterns with include() only
        let source = r#"
from django.urls import path, include

urlpatterns = [
    path("api/", include("myapp.urls")),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: empty Vec (include() is not a handler)
        assert!(
            routes.is_empty(),
            "expected empty Vec for include()-only urlpatterns, got {:?}",
            routes
        );
    }

    // DJ-RT-12: multiple path parameters
    #[test]
    fn dj_rt_12_multiple_path_parameters() {
        // Given: path("posts/<slug:slug>/comments/<int:id>/", views.comment_detail)
        let source = r#"
from django.urls import path
from . import views

urlpatterns = [
    path("posts/<slug:slug>/comments/<int:id>/", views.comment_detail),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: path = "posts/:slug/comments/:id/"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, "posts/:slug/comments/:id/");
    }

    // DJ-RT-13: re_path with multiple named groups
    #[test]
    fn dj_rt_13_re_path_multiple_named_groups() {
        // Given: re_path("^(?P<year>[0-9]{4})/(?P<month>[0-9]{2})/$", views.archive)
        let source = r#"
from django.urls import re_path
from . import views

urlpatterns = [
    re_path(r"^(?P<year>[0-9]{4})/(?P<month>[0-9]{2})/$", views.archive),
]
"#;
        // When: extract_django_routes is called
        let routes = extract_django_routes(source, "urls.py");

        // Then: path = ":year/:month/"
        assert_eq!(routes.len(), 1, "expected 1 route, got {:?}", routes);
        assert_eq!(routes[0].path, ":year/:month/");
    }

    // -----------------------------------------------------------------------
    // Integration: CLI (DJ-RT-E2E-01)
    // -----------------------------------------------------------------------

    // DJ-RT-E2E-01: observe with Django routes — routes_total = 2
    #[test]
    fn dj_rt_e2e_01_observe_django_routes_coverage() {
        use tempfile::TempDir;

        // Given: tempdir with urls.py (2 routes) and test_urls.py
        let dir = TempDir::new().unwrap();
        let urls_py = dir.path().join("urls.py");
        let test_urls_py = dir.path().join("test_urls.py");

        std::fs::write(
            &urls_py,
            r#"from django.urls import path
from . import views

urlpatterns = [
    path("users/", views.user_list),
    path("users/<int:pk>/", views.user_detail),
]
"#,
        )
        .unwrap();

        std::fs::write(
            &test_urls_py,
            r#"def test_user_list():
    pass

def test_user_detail():
    pass
"#,
        )
        .unwrap();

        // When: extract_django_routes from urls.py
        let urls_source = std::fs::read_to_string(&urls_py).unwrap();
        let urls_path = urls_py.to_string_lossy().into_owned();

        let routes = extract_django_routes(&urls_source, &urls_path);

        // Then: routes_total = 2
        assert_eq!(
            routes.len(),
            2,
            "expected 2 routes extracted from urls.py, got {:?}",
            routes
        );

        // Verify both routes have method "ANY"
        for r in &routes {
            assert_eq!(r.http_method, "ANY", "expected method ANY, got {:?}", r);
        }
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-04: e2e: `import pkg`, pkg/__init__.py has `from .module import *`,
    //               pkg/module.py has Foo -> module.py mapped
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_04_e2e_bare_import_wildcard_barrel_mapped() {
        use tempfile::TempDir;

        // Given: tempdir with pkg/__init__.py (wildcard re-export) + pkg/module.py
        //        and test_foo.py that uses bare `import pkg`
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(pkg.join("__init__.py"), "from .module import *\n").unwrap();
        std::fs::write(pkg.join("module.py"), "class Foo:\n    pass\n").unwrap();

        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        let test_content = "import pkg\n\ndef test_foo():\n    assert pkg.Foo()\n";
        std::fs::write(tests_dir.join("test_foo.py"), test_content).unwrap();

        let module_path = pkg.join("module.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![module_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: module.py is matched via bare import + wildcard barrel chain
        let mapping = result.iter().find(|m| m.production_file == module_path);
        assert!(
            mapping.is_some(),
            "module.py not mapped; bare import + wildcard barrel should resolve. mappings={:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_foo.py not in test_files for module.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-IMPORT-05: e2e: `import pkg`, pkg/__init__.py has `from .module import Foo`
    //               (named), pkg/module.py has Foo -> module.py mapped
    // -----------------------------------------------------------------------
    #[test]
    fn py_import_05_e2e_bare_import_named_barrel_mapped() {
        use tempfile::TempDir;

        // Given: tempdir with pkg/__init__.py (named re-export) + pkg/module.py
        //        and test_foo.py that uses bare `import pkg`
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(pkg.join("__init__.py"), "from .module import Foo\n").unwrap();
        std::fs::write(pkg.join("module.py"), "class Foo:\n    pass\n").unwrap();

        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        let test_content = "import pkg\n\ndef test_foo():\n    assert pkg.Foo()\n";
        std::fs::write(tests_dir.join("test_foo.py"), test_content).unwrap();

        let module_path = pkg.join("module.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![module_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: module.py is matched via bare import + named barrel chain
        let mapping = result.iter().find(|m| m.production_file == module_path);
        assert!(
            mapping.is_some(),
            "module.py not mapped; bare import + named barrel should resolve. mappings={:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_foo.py not in test_files for module.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-01: `import httpx\nhttpx.Client()\n`
    //             -> specifier="httpx", symbols=["Client"] (single attribute access)
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_01_bare_import_single_attribute() {
        // Given: source with a bare import and a single attribute access
        let source = "import httpx\nhttpx.Client()\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", ["Client"]) -- attribute access extracted as symbol
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert_eq!(
            symbols,
            &vec!["Client".to_string()],
            "expected [\"Client\"] for bare import with attribute access, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-02: `import httpx\nhttpx.Client()\nhttpx.get()\n`
    //             -> specifier="httpx", symbols contains "Client" and "get" (multiple attributes)
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_02_bare_import_multiple_attributes() {
        // Given: source with a bare import and multiple attribute accesses
        let source = "import httpx\nhttpx.Client()\nhttpx.get()\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", [...]) with both "Client" and "get"
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"Client".to_string()),
            "Client not in symbols: {:?}",
            symbols
        );
        assert!(
            symbols.contains(&"get".to_string()),
            "get not in symbols: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-03: `import httpx\nhttpx.Client()\nhttpx.Client()\n`
    //             -> specifier="httpx", symbols=["Client"] (deduplication)
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_03_bare_import_deduplicated_attributes() {
        // Given: source with a bare import and duplicate attribute accesses
        let source = "import httpx\nhttpx.Client()\nhttpx.Client()\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", ["Client"]) -- duplicates removed
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert_eq!(
            symbols,
            &vec!["Client".to_string()],
            "expected [\"Client\"] with deduplication, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-04: `import httpx\n` (no attribute access)
    //             -> specifier="httpx", symbols=[] (fallback: match all)
    //
    // NOTE: This test covers the same input as PY-IMPORT-01 but explicitly
    //       verifies the "no attribute access → symbols=[] fallback" contract
    //       introduced in Phase 16. PY-IMPORT-01 verifies the pre-Phase 16
    //       baseline; this test documents the Phase 16 intentional behaviour.
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_04_bare_import_no_attribute_fallback() {
        // Given: source with a bare import but no attribute access
        let source = "import httpx\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", []) -- no attribute access means match-all fallback
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(
            entry.is_some(),
            "httpx not found in {:?}; bare import without attribute access should be included",
            result
        );
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.is_empty(),
            "expected empty symbols (fallback) for bare import with no attribute access, got {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-05: `from httpx import Client\n`
    //             -> specifier="httpx", symbols=["Client"]
    //             (regression: Phase 16 changes must not affect from-import)
    //
    // NOTE: This is a regression test verifying that Phase 16 attribute-access
    //       filtering does not change the behaviour of `from X import Y` paths.
    //       PY-IMPORT-03 tests the same input as a baseline; this test
    //       explicitly documents the Phase 16 non-regression requirement.
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_05_from_import_regression() {
        // Given: source with a from-import (must not be affected by Phase 16 changes)
        let source = "from httpx import Client\n";

        // When: extract_all_import_specifiers is called
        let extractor = PythonExtractor::new();
        let result = extractor.extract_all_import_specifiers(source);

        // Then: contains ("httpx", ["Client"]) -- from-import path unchanged
        let entry = result.iter().find(|(spec, _)| spec == "httpx");
        assert!(entry.is_some(), "httpx not found in {:?}", result);
        let (_, symbols) = entry.unwrap();
        assert!(
            symbols.contains(&"Client".to_string()),
            "Client not in symbols: {:?}",
            symbols
        );
    }

    // -----------------------------------------------------------------------
    // PY-ATTR-06: e2e: `import pkg\npkg.Foo()\n`, barrel `from .mod import Foo`
    //             and `from .bar import Bar` -> mod.py mapped, bar.py NOT mapped
    //             (attribute-access filtering narrows barrel resolution)
    // -----------------------------------------------------------------------
    #[test]
    fn py_attr_06_e2e_attribute_access_narrows_barrel_mapping() {
        use tempfile::TempDir;

        // Given: tempdir with:
        //   pkg/__init__.py: re-exports Foo from .mod and Bar from .bar
        //   pkg/mod.py: defines Foo
        //   pkg/bar.py: defines Bar
        //   tests/test_foo.py: uses bare `import pkg` and accesses only `pkg.Foo()`
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(
            pkg.join("__init__.py"),
            "from .mod import Foo\nfrom .bar import Bar\n",
        )
        .unwrap();
        std::fs::write(pkg.join("mod.py"), "def Foo(): pass\n").unwrap();
        std::fs::write(pkg.join("bar.py"), "def Bar(): pass\n").unwrap();

        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        // Test file only accesses pkg.Foo, not pkg.Bar
        let test_content = "import pkg\npkg.Foo()\n";
        std::fs::write(tests_dir.join("test_foo.py"), test_content).unwrap();

        let mod_path = pkg.join("mod.py").to_string_lossy().into_owned();
        let bar_path = pkg.join("bar.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![mod_path.clone(), bar_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: mod.py is mapped (Foo is accessed via pkg.Foo())
        let mod_mapping = result.iter().find(|m| m.production_file == mod_path);
        assert!(
            mod_mapping.is_some(),
            "mod.py not mapped; pkg.Foo() should resolve to mod.py via barrel. mappings={:?}",
            result
        );
        assert!(
            mod_mapping.unwrap().test_files.contains(&test_path),
            "test_foo.py not in test_files for mod.py: {:?}",
            mod_mapping.unwrap().test_files
        );

        // Then: bar.py is NOT mapped (Bar is not accessed -- pkg.Bar() is absent)
        let bar_mapping = result.iter().find(|m| m.production_file == bar_path);
        let bar_not_mapped = bar_mapping
            .map(|m| !m.test_files.contains(&test_path))
            .unwrap_or(true);
        assert!(
            bar_not_mapped,
            "bar.py should NOT be mapped for test_foo.py (pkg.Bar() is not accessed), but got: {:?}",
            bar_mapping
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-01: stem-only fallback: tests/test_client.py -> pkg/_client.py (cross-directory)
    //
    // The key scenario: test file is in tests/ but prod is in pkg/.
    // L1 core uses (dir, stem) pair, so tests/test_client.py (dir=tests/) does NOT
    // match pkg/_client.py (dir=pkg/) via L1 core.
    // stem-only fallback should match them via stem "client" regardless of directory.
    // The test file has NO import statements to avoid L2 from resolving the mapping.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_01_stem_only_fallback_cross_directory() {
        use tempfile::TempDir;

        // Given: pkg/_client.py (prod) and tests/test_client.py (test, NO imports)
        //        L1 core cannot match (different dirs: pkg/ vs tests/)
        //        L2 cannot match (no import statements)
        //        stem-only fallback should match via stem "client"
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();

        // No imports -- forces reliance on stem-only fallback (not L2)
        let test_content = "def test_client():\n    pass\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_client.py is mapped to pkg/_client.py via stem-only fallback
        let mapping = result.iter().find(|m| m.production_file == client_path);
        assert!(
            mapping.is_some(),
            "pkg/_client.py not mapped; stem-only fallback should match across directories. mappings={:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_client.py not in test_files for pkg/_client.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-02: stem-only: tests/test_decoders.py -> pkg/_decoders.py (_ prefix prod)
    //
    // production_stem strips leading _ so "_decoders" -> "decoders".
    // test_stem strips "test_" prefix so "test_decoders" -> "decoders".
    // stem-only fallback should match them even though dirs differ.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_02_stem_only_underscore_prefix_prod() {
        use tempfile::TempDir;

        // Given: pkg/_decoders.py and tests/test_decoders.py (no imports)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_decoders.py"), "def decode(x): return x\n").unwrap();

        // No imports -- forces reliance on stem-only fallback
        let test_content = "def test_decode():\n    pass\n";
        std::fs::write(tests_dir.join("test_decoders.py"), test_content).unwrap();

        let decoders_path = pkg.join("_decoders.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_decoders.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![decoders_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_decoders.py is mapped to pkg/_decoders.py via stem-only fallback
        //       (production_stem strips '_' prefix: "_decoders" -> "decoders")
        let mapping = result.iter().find(|m| m.production_file == decoders_path);
        assert!(
            mapping.is_some(),
            "pkg/_decoders.py not mapped; stem-only fallback should strip _ prefix and match. mappings={:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_decoders.py not in test_files for pkg/_decoders.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-03: stem-only: tests/test_asgi.py -> pkg/transports/asgi.py (subdirectory)
    //
    // Prod is in a subdirectory (pkg/transports/), test is in tests/.
    // stem "asgi" should match across any directory depth.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_03_stem_only_subdirectory_prod() {
        use tempfile::TempDir;

        // Given: pkg/transports/asgi.py and tests/test_asgi.py (no imports)
        let dir = TempDir::new().unwrap();
        let transports = dir.path().join("pkg").join("transports");
        std::fs::create_dir_all(&transports).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            transports.join("asgi.py"),
            "class ASGITransport:\n    pass\n",
        )
        .unwrap();

        // No imports -- forces reliance on stem-only fallback
        let test_content = "def test_asgi_transport():\n    pass\n";
        std::fs::write(tests_dir.join("test_asgi.py"), test_content).unwrap();

        let asgi_path = transports.join("asgi.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_asgi.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![asgi_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_asgi.py is mapped to pkg/transports/asgi.py
        //       (stem "asgi" matches across directory depth)
        let mapping = result.iter().find(|m| m.production_file == asgi_path);
        assert!(
            mapping.is_some(),
            "pkg/transports/asgi.py not mapped; stem 'asgi' should match across directory depth. mappings={:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_asgi.py not in test_files for pkg/transports/asgi.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-04: stem collision -- no imports -> L1 stem-only fallback defers to L2
    //
    // When multiple prod files share the same stem and the test has no imports,
    // the collision guard prevents L1 stem-only from mapping to any of them.
    // Precision takes priority over recall in this case.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_04_stem_collision_defers_to_l2() {
        use tempfile::TempDir;

        // Given: pkg/client.py, pkg/aio/client.py, and tests/test_client.py (no imports)
        //        Both have stem "client"; test has stem "client" but no import -> collision guard fires
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        let pkg_aio = pkg.join("aio");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&pkg_aio).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg_aio.join("client.py"), "class AsyncClient:\n    pass\n").unwrap();

        // No imports -- collision guard should prevent stem-only fallback from mapping
        let test_content = "def test_client():\n    pass\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("client.py").to_string_lossy().into_owned();
        let aio_client_path = pkg_aio.join("client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), aio_client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_client.py is NOT mapped to pkg/client.py (collision guard defers to L2)
        let client_mapped = result
            .iter()
            .find(|m| m.production_file == client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            !client_mapped,
            "test_client.py should NOT be mapped to pkg/client.py (stem collision -> defer to L2). mappings={:?}",
            result
        );

        let aio_mapped = result
            .iter()
            .find(|m| m.production_file == aio_client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            !aio_mapped,
            "test_client.py should NOT be mapped to pkg/aio/client.py (stem collision -> defer to L2). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-05: L1 core match already found -> stem-only fallback does NOT fire
    //
    // When L1 core (dir, stem) already matches, stem-only fallback should be
    // suppressed for that test to avoid adding cross-directory duplicates.
    // Note: production file uses svc/ (not tests/) since Phase 20 excludes tests/ files.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_05_l1_core_match_suppresses_fallback() {
        use tempfile::TempDir;

        // Given: svc/client.py (L1 core match: dir=svc/, stem=client)
        //        pkg/client.py (would match via stem-only fallback if L1 core is absent)
        //        svc/test_client.py (no imports)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        let svc = dir.path().join("svc");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&svc).unwrap();

        std::fs::write(svc.join("client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg.join("client.py"), "class Client:\n    pass\n").unwrap();

        // No imports -- avoids L2 influence; only L1 core and stem-only fallback apply
        let test_content = "def test_client():\n    pass\n";
        std::fs::write(svc.join("test_client.py"), test_content).unwrap();

        let svc_client_path = svc.join("client.py").to_string_lossy().into_owned();
        let pkg_client_path = pkg.join("client.py").to_string_lossy().into_owned();
        let test_path = svc.join("test_client.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![svc_client_path.clone(), pkg_client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_client.py is mapped to svc/client.py only (L1 core match)
        let svc_client_mapped = result
            .iter()
            .find(|m| m.production_file == svc_client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            svc_client_mapped,
            "test_client.py should be mapped to svc/client.py via L1 core. mappings={:?}",
            result
        );

        // Then: fallback does NOT add pkg/client.py (L1 core match suppresses fallback)
        let pkg_not_mapped = result
            .iter()
            .find(|m| m.production_file == pkg_client_path)
            .map(|m| !m.test_files.contains(&test_path))
            .unwrap_or(true);
        assert!(
            pkg_not_mapped,
            "pkg/client.py should NOT be mapped (L1 core match suppresses stem-only fallback). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-06: stem collision + L2 import -> maps to the correct file via ImportTracing
    //
    // When multiple prod files share the same stem, stem-only fallback defers to L2.
    // If the test has a direct import, L2 resolves it to the correct file.
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_06_stem_collision_with_l2_import_resolves_correctly() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/client.py, pkg/aio/client.py (same stem "client")
        //        tests/test_client.py has "from pkg.client import Client" (direct import)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        let pkg_aio = pkg.join("aio");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&pkg_aio).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg_aio.join("client.py"), "class AsyncClient:\n    pass\n").unwrap();

        // Direct import to pkg.client -> L2 resolves to pkg/client.py
        let test_content =
            "from pkg.client import Client\n\ndef test_client():\n    assert Client()\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("client.py").to_string_lossy().into_owned();
        let aio_client_path = pkg_aio.join("client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), aio_client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: test_client.py is mapped to pkg/client.py only (L2 ImportTracing)
        let client_mapping = result.iter().find(|m| m.production_file == client_path);
        assert!(
            client_mapping.is_some(),
            "pkg/client.py not found in mappings: {:?}",
            result
        );
        let client_mapping = client_mapping.unwrap();
        assert!(
            client_mapping.test_files.contains(&test_path),
            "test_client.py should be mapped to pkg/client.py via L2. mappings={:?}",
            result
        );
        assert_eq!(
            client_mapping.strategy,
            MappingStrategy::ImportTracing,
            "strategy should be ImportTracing (L2), got {:?}",
            client_mapping.strategy
        );

        // Then: pkg/aio/client.py is NOT mapped (collision guard + L2 resolves to pkg/client.py)
        let aio_mapped = result
            .iter()
            .find(|m| m.production_file == aio_client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            !aio_mapped,
            "test_client.py should NOT be mapped to pkg/aio/client.py. mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-L1X-07: stem collision + barrel import -> L2 barrel resolves to correct file
    //
    // When multiple prod files share the same stem, and the test imports via barrel,
    // L2 barrel tracing resolves to the correct file (not all files with that stem).
    // -----------------------------------------------------------------------
    #[test]
    fn py_l1x_07_stem_collision_with_barrel_import_resolves_correctly() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/__init__.py (barrel: "from .client import Client")
        //        pkg/client.py, pkg/aio/client.py (same stem "client")
        //        tests/test_client.py has "from pkg import Client" (barrel import)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        let pkg_aio = pkg.join("aio");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&pkg_aio).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        // Barrel re-exports Client from pkg/client.py (not pkg/aio/client.py)
        std::fs::write(pkg.join("__init__.py"), "from .client import Client\n").unwrap();
        std::fs::write(pkg.join("client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg_aio.join("client.py"), "class AsyncClient:\n    pass\n").unwrap();

        // Import via barrel -> L2 barrel tracing should resolve to pkg/client.py
        let test_content = "from pkg import Client\n\ndef test_client():\n    assert Client()\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("client.py").to_string_lossy().into_owned();
        let aio_client_path = pkg_aio.join("client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), aio_client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: collision guard prevents L1 stem-only from mapping to both files
        //       L2 barrel import resolves to pkg/client.py (via __init__.py re-export)
        let client_mapped = result
            .iter()
            .find(|m| m.production_file == client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            client_mapped,
            "test_client.py should be mapped to pkg/client.py via barrel L2. mappings={:?}",
            result
        );

        // Then: pkg/aio/client.py is NOT mapped (barrel only re-exports pkg.client)
        let aio_mapped = result
            .iter()
            .find(|m| m.production_file == aio_client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            !aio_mapped,
            "test_client.py should NOT be mapped to pkg/aio/client.py. mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUP-01: barrel suppression: L1 stem-only matched test does not get barrel fan-out
    //
    // The httpx FP scenario:
    // - tests/test_client.py has NO specific imports (bare `import pkg` + no attribute access)
    // - Without barrel suppression: `import pkg` -> barrel -> _client.py + _utils.py (FP!)
    // - With stem-only L1 match + barrel suppression:
    //   test_client.py -> L1 stem-only -> _client.py only (barrel _utils.py suppressed)
    // -----------------------------------------------------------------------
    #[test]
    fn py_sup_01_barrel_suppression_l1_matched_no_barrel_fan_out() {
        use tempfile::TempDir;

        // Given: pkg/_client.py, pkg/_utils.py, pkg/__init__.py (barrel)
        //        tests/test_client.py: `import pkg` (bare import, NO attribute access)
        //        L1 stem-only fallback: test_client.py -> pkg/_client.py (stem "client")
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_utils.py"), "def format_url(u): return u\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._client import Client\nfrom ._utils import format_url\n",
        )
        .unwrap();

        // bare `import pkg` with NO attribute access -> symbols=[] -> barrel fan-out to all
        // Without barrel suppression: _client.py AND _utils.py both mapped (FP for _utils)
        // With barrel suppression (L1 matched): only _client.py mapped
        let test_content = "import pkg\n\ndef test_client():\n    pass\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let utils_path = pkg.join("_utils.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), utils_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _client.py IS mapped (L1 stem-only match)
        let client_mapped = result
            .iter()
            .find(|m| m.production_file == client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            client_mapped,
            "pkg/_client.py should be mapped via L1 stem-only. mappings={:?}",
            result
        );

        // Then: _utils.py is NOT mapped (barrel fan-out suppressed because L1 stem-only matched)
        let utils_not_mapped = result
            .iter()
            .find(|m| m.production_file == utils_path)
            .map(|m| !m.test_files.contains(&test_path))
            .unwrap_or(true);
        assert!(
            utils_not_mapped,
            "pkg/_utils.py should NOT be mapped (barrel suppression for L1-matched test_client.py). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUP-02: barrel suppression: L1 stem-only matched test still gets direct imports
    //
    // Direct imports (from pkg._utils import format_url) bypass barrel resolution.
    // Even if L1 stem-only matches _client.py, direct imports to _utils.py are added.
    // -----------------------------------------------------------------------
    #[test]
    fn py_sup_02_barrel_suppression_direct_import_still_added() {
        use tempfile::TempDir;

        // Given: pkg/_client.py, pkg/_utils.py, pkg/__init__.py (barrel)
        //        tests/test_client.py:
        //          - `import pkg` (bare import, no attribute access -> would fan-out to barrel)
        //          - `from pkg._utils import format_url` (direct import)
        //        L1 stem-only: test_client.py -> _client.py
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_utils.py"), "def format_url(u): return u\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._client import Client\nfrom ._utils import format_url\n",
        )
        .unwrap();

        // Direct import to _utils -- this is NOT via barrel, so suppression does not apply
        let test_content =
            "import pkg\nfrom pkg._utils import format_url\n\ndef test_client():\n    assert format_url('http://x')\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let utils_path = pkg.join("_utils.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), utils_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _utils.py IS mapped (direct import bypasses barrel suppression)
        let utils_mapped = result
            .iter()
            .find(|m| m.production_file == utils_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            utils_mapped,
            "pkg/_utils.py should be mapped via direct import (not barrel). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUP-03: barrel suppression: L1-unmatched test gets barrel fan-out as usual
    //
    // A test with NO matching stem in prod files should still get barrel fan-out.
    // Barrel suppression only applies to L1 stem-only matched tests.
    // -----------------------------------------------------------------------
    #[test]
    fn py_sup_03_barrel_suppression_l1_unmatched_gets_barrel() {
        use tempfile::TempDir;

        // Given: pkg/_client.py, pkg/_utils.py, pkg/__init__.py (barrel)
        //        tests/test_exported_members.py: `import pkg` (bare import, no attr access)
        //        stem "exported_members" has NO matching production file (L1 miss)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_utils.py"), "def format_url(u): return u\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._client import Client\nfrom ._utils import format_url\n",
        )
        .unwrap();

        // bare `import pkg` with NO attribute access -> should fan-out via barrel (L1 miss)
        let test_content = "import pkg\n\ndef test_exported_members():\n    pass\n";
        std::fs::write(tests_dir.join("test_exported_members.py"), test_content).unwrap();

        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let utils_path = pkg.join("_utils.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_exported_members.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![client_path.clone(), utils_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: barrel fan-out proceeds for L1-unmatched test
        //       BOTH _client.py and _utils.py should be mapped (barrel re-exports both)
        let client_mapped = result
            .iter()
            .find(|m| m.production_file == client_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        let utils_mapped = result
            .iter()
            .find(|m| m.production_file == utils_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);

        assert!(
            client_mapped && utils_mapped,
            "L1-unmatched test should fan-out via barrel to both _client.py and _utils.py. client_mapped={}, utils_mapped={}, mappings={:?}",
            client_mapped,
            utils_mapped,
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUP-04: E2E: httpx-like fixture demonstrates FP reduction (P >= 80%)
    //
    // Simulates the core httpx FP scenario:
    // - Multiple prod files under pkg/ with underscore prefix
    // - tests/ directory (different from pkg/)
    // - Some tests import pkg bare (no attribute access) -> currently fans-out to all
    // - stem-only fallback + barrel suppression should limit fan-out
    //
    // Note: P>=80% is the intermediate goal; Ship criteria is P>=98% (CONSTITUTION).
    // -----------------------------------------------------------------------
    #[test]
    fn py_sup_04_e2e_httpx_like_precision_improvement() {
        use tempfile::TempDir;
        use HashSet;

        // Given: httpx-like structure
        //   pkg/_client.py, pkg/_decoders.py, pkg/_utils.py
        //   pkg/__init__.py: barrel re-exporting Client, decode, format_url
        //   tests/test_client.py: bare `import pkg` NO attribute access (stem -> _client.py)
        //   tests/test_decoders.py: bare `import pkg` NO attribute access (stem -> _decoders.py)
        //   tests/test_utils.py: bare `import pkg` NO attribute access (stem -> _utils.py)
        //   tests/test_exported_members.py: bare `import pkg` NO attr access (L1 miss -> barrel OK)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_decoders.py"), "def decode(x): return x\n").unwrap();
        std::fs::write(pkg.join("_utils.py"), "def format_url(u): return u\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._client import Client\nfrom ._decoders import decode\nfrom ._utils import format_url\n",
        )
        .unwrap();

        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let decoders_path = pkg.join("_decoders.py").to_string_lossy().into_owned();
        let utils_path = pkg.join("_utils.py").to_string_lossy().into_owned();
        let production_files = vec![
            client_path.clone(),
            decoders_path.clone(),
            utils_path.clone(),
        ];

        // All test files use bare `import pkg` with NO attribute access
        // -> without suppression: all fan-out to all 3 prod files (P=33%)
        // -> with stem-only + barrel suppression: each maps to 1 (P=100% for L1-matched)
        let test_client_content = "import pkg\n\ndef test_client():\n    pass\n";
        let test_decoders_content = "import pkg\n\ndef test_decode():\n    pass\n";
        let test_utils_content = "import pkg\n\ndef test_format_url():\n    pass\n";
        let test_exported_content = "import pkg\n\ndef test_exported_members():\n    pass\n";

        let test_client_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();
        let test_decoders_path = tests_dir
            .join("test_decoders.py")
            .to_string_lossy()
            .into_owned();
        let test_utils_path = tests_dir
            .join("test_utils.py")
            .to_string_lossy()
            .into_owned();
        let test_exported_path = tests_dir
            .join("test_exported_members.py")
            .to_string_lossy()
            .into_owned();

        std::fs::write(&test_client_path, test_client_content).unwrap();
        std::fs::write(&test_decoders_path, test_decoders_content).unwrap();
        std::fs::write(&test_utils_path, test_utils_content).unwrap();
        std::fs::write(&test_exported_path, test_exported_content).unwrap();

        let test_sources: HashMap<String, String> = [
            (test_client_path.clone(), test_client_content.to_string()),
            (
                test_decoders_path.clone(),
                test_decoders_content.to_string(),
            ),
            (test_utils_path.clone(), test_utils_content.to_string()),
            (
                test_exported_path.clone(),
                test_exported_content.to_string(),
            ),
        ]
        .into_iter()
        .collect();

        let extractor = PythonExtractor::new();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Ground truth (expected TP pairs):
        // test_client.py -> _client.py  (L1 stem-only)
        // test_decoders.py -> _decoders.py  (L1 stem-only)
        // test_utils.py -> _utils.py  (L1 stem-only)
        // test_exported_members.py -> _client.py, _decoders.py, _utils.py  (barrel, L1 miss)
        let ground_truth_set: HashSet<(String, String)> = [
            (test_client_path.clone(), client_path.clone()),
            (test_decoders_path.clone(), decoders_path.clone()),
            (test_utils_path.clone(), utils_path.clone()),
            (test_exported_path.clone(), client_path.clone()),
            (test_exported_path.clone(), decoders_path.clone()),
            (test_exported_path.clone(), utils_path.clone()),
        ]
        .into_iter()
        .collect();

        let actual_pairs: HashSet<(String, String)> = result
            .iter()
            .flat_map(|m| {
                m.test_files
                    .iter()
                    .map(|t| (t.clone(), m.production_file.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();

        let tp = actual_pairs.intersection(&ground_truth_set).count();
        let fp = actual_pairs.difference(&ground_truth_set).count();

        // Precision = TP / (TP + FP)
        let precision = if tp + fp == 0 {
            0.0
        } else {
            tp as f64 / (tp + fp) as f64
        };

        // Then: precision >= 80% (intermediate goal)
        // Without stem-only + barrel suppression: all 4 tests fan-out to 3 prod files
        // = 12 pairs, but GT has 6 -> P = 6/12 = 50% (FAIL)
        // With suppression: 3 stem-matched tests -> 1 each + exported_members -> 3 = 6 pairs -> P = 100%
        assert!(
            precision >= 0.80,
            "Precision {:.1}% < 80% target. TP={}, FP={}, actual_pairs={:?}",
            precision * 100.0,
            tp,
            fp,
            actual_pairs
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-01: assignment + assertion tracking
    //   `client = Client(); assert client.ok` -> Client in asserted_imports
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_01_assert_via_assigned_var() {
        // Given: source where Client is assigned then asserted
        let source = r#"
from pkg.client import Client

def test_something():
    client = Client()
    assert client.ok
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: Client is in asserted_imports (assigned var `client` appears in assertion)
        assert!(
            result.contains("Client"),
            "Client should be in asserted_imports; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-02: non-asserted assignment is excluded
    //   `transport = MockTransport()` (not in assert) -> MockTransport NOT in asserted_imports
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_02_setup_only_import_excluded() {
        // Given: source where MockTransport is assigned but never asserted
        let source = r#"
from pkg.client import Client
from pkg.transport import MockTransport

def test_something():
    transport = MockTransport()
    client = Client(transport=transport)
    assert client.ok
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: MockTransport is NOT in asserted_imports (only used in setup)
        assert!(
            !result.contains("MockTransport"),
            "MockTransport should NOT be in asserted_imports (setup-only); got {:?}",
            result
        );
        // And: Client IS in asserted_imports (via chain: client -> Client)
        assert!(
            result.contains("Client"),
            "Client should be in asserted_imports; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-03: direct usage in assertion
    //   `assert A() == B()` -> both A, B in asserted_imports
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_03_direct_call_in_assertion() {
        // Given: source where two classes are directly called inside an assert
        let source = r#"
from pkg.models import A, B

def test_equality():
    assert A() == B()
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: both A and B are in asserted_imports
        assert!(
            result.contains("A"),
            "A should be in asserted_imports (used directly in assert); got {:?}",
            result
        );
        assert!(
            result.contains("B"),
            "B should be in asserted_imports (used directly in assert); got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-04: pytest.raises context
    //   `pytest.raises(HTTPError)` -> HTTPError in asserted_imports
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_04_pytest_raises_captures_exception_class() {
        // Given: source using pytest.raises with an imported exception class
        let source = r#"
import pytest
from pkg.exceptions import HTTPError

def test_raises():
    with pytest.raises(HTTPError):
        raise HTTPError("fail")
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: HTTPError is in asserted_imports (appears in pytest.raises assertion node)
        assert!(
            result.contains("HTTPError"),
            "HTTPError should be in asserted_imports (pytest.raises arg); got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-05: chain tracking (2-hop)
    //   `response = client.get(); assert response.ok` -> Client reachable via chain
    //   client -> Client (1-hop), response -> client (2-hop through method call source)
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_05_chain_tracking_two_hops() {
        // Given: source with a 2-hop chain: response derived from client, client from Client()
        let source = r#"
from pkg.client import Client

def test_response():
    client = Client()
    response = client.get("http://example.com/")
    assert response.ok
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: Client is reachable (response -> client -> Client, 2-hop chain)
        assert!(
            result.contains("Client"),
            "Client should be in asserted_imports via 2-hop chain; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-06a: no assertions -> empty asserted_imports -> fallback to all_matched
    //   When assertion.scm detects no assertions, asserted_imports is empty
    //   and the caller falls back to all_matched.
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_06a_no_assertions_returns_empty() {
        // Given: source with imports but zero assertion statements
        let source = r#"
from pkg.client import Client
from pkg.transport import MockTransport

def test_setup_no_assert():
    client = Client()
    transport = MockTransport()
    # No assert statement at all
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: asserted_imports is EMPTY (no assertions found, so no symbols traced)
        // The caller (map_test_files_with_imports) is responsible for the fallback.
        assert!(
            result.is_empty(),
            "expected empty asserted_imports when no assertions present; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-06b: assertions exist but no asserted import intersects with L2 imports
    //   -> asserted_imports non-empty but does not overlap with any import symbol
    //   -> fallback to all_matched (safe side)
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_06b_assertion_exists_but_no_import_intersection() {
        // Given: source where the assertion references a local variable (not an import)
        let source = r#"
from pkg.client import Client

def test_local_only():
    local_value = 42
    # Assertion references only a local literal, not any imported symbol
    assert local_value == 42
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: asserted_imports does NOT contain Client
        // (Client is never referenced inside an assertion node)
        assert!(
            !result.contains("Client"),
            "Client should NOT be in asserted_imports (not referenced in assertion); got {:?}",
            result
        );
        // Note: `result` may be empty or contain other identifiers from the assertion,
        // but the key property is that the imported symbol Client is absent.
    }

    // -----------------------------------------------------------------------
    // PY-AF-07: unittest self.assert* form
    //   `self.assertEqual(result.value, 42)` -> result's import captured
    //   result = MyModel() -> MyModel in asserted_imports
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_07_unittest_self_assert() {
        // Given: unittest-style test using self.assertEqual
        let source = r#"
import unittest
from pkg.models import MyModel

class TestMyModel(unittest.TestCase):
    def test_value(self):
        result = MyModel()
        self.assertEqual(result.value, 42)
"#;
        // When: extract_assertion_referenced_imports is called
        let result = extract_assertion_referenced_imports(source);

        // Then: MyModel is in asserted_imports (result -> MyModel, result in assertEqual)
        assert!(
            result.contains("MyModel"),
            "MyModel should be in asserted_imports via self.assertEqual; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-08: E2E integration — primary import kept, incidental filtered
    //
    // Fixture: af_pkg/
    //   pkg/client.py    (Client class)   <- should be mapped
    //   pkg/transport.py (MockTransport)  <- should NOT be mapped (assertion filter)
    //   tests/test_client.py imports both, asserts only client.is_ok
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_08_e2e_primary_kept_incidental_filtered() {
        use std::path::PathBuf;
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/af_pkg");

        let test_file = fixture_root
            .join("tests/test_client.py")
            .to_string_lossy()
            .into_owned();
        let client_prod = fixture_root
            .join("pkg/client.py")
            .to_string_lossy()
            .into_owned();
        let transport_prod = fixture_root
            .join("pkg/transport.py")
            .to_string_lossy()
            .into_owned();

        let production_files = vec![client_prod.clone(), transport_prod.clone()];
        let test_source =
            std::fs::read_to_string(&test_file).expect("fixture test file must exist");
        let mut test_sources = HashMap::new();
        test_sources.insert(test_file.clone(), test_source);

        // When: map_test_files_with_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &fixture_root,
            false,
        );

        // Then: test_client.py maps to client.py (Client is asserted)
        let client_mapping = result.iter().find(|m| m.production_file == client_prod);
        assert!(
            client_mapping.is_some(),
            "client.py should be in mappings; got {:?}",
            result
                .iter()
                .map(|m| &m.production_file)
                .collect::<Vec<_>>()
        );
        assert!(
            client_mapping.unwrap().test_files.contains(&test_file),
            "test_client.py should map to client.py"
        );

        // And: test_client.py does NOT map to transport.py (MockTransport not asserted)
        let transport_mapping = result.iter().find(|m| m.production_file == transport_prod);
        let transport_maps_test = transport_mapping
            .map(|m| m.test_files.contains(&test_file))
            .unwrap_or(false);
        assert!(
            !transport_maps_test,
            "test_client.py should NOT map to transport.py (assertion filter); got {:?}",
            result
                .iter()
                .map(|m| (&m.production_file, &m.test_files))
                .collect::<Vec<_>>()
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-09: E2E — ALL imports incidental -> fallback, no regression (FN prevented)
    //
    // Fixture: af_e2e_fallback/
    //   pkg/helpers.py (HelperA, HelperB)
    //   tests/test_helpers.py: imports both, assertion is about `result is None`
    //   -> asserted_matched would be empty -> fallback to all_matched
    //   -> helpers.py MUST appear in the mapping (no FN)
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_09_e2e_all_incidental_fallback_no_fn() {
        use std::path::PathBuf;
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/af_e2e_fallback");

        let test_file = fixture_root
            .join("tests/test_helpers.py")
            .to_string_lossy()
            .into_owned();
        let helpers_prod = fixture_root
            .join("pkg/helpers.py")
            .to_string_lossy()
            .into_owned();

        let production_files = vec![helpers_prod.clone()];
        let test_source =
            std::fs::read_to_string(&test_file).expect("fixture test file must exist");
        let mut test_sources = HashMap::new();
        test_sources.insert(test_file.clone(), test_source);

        // When: map_test_files_with_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &fixture_root,
            false,
        );

        // Then: helpers.py IS mapped (fallback activated because asserted_matched is empty)
        let helpers_mapping = result.iter().find(|m| m.production_file == helpers_prod);
        assert!(
            helpers_mapping.is_some(),
            "helpers.py should be in mappings (fallback); got {:?}",
            result
                .iter()
                .map(|m| &m.production_file)
                .collect::<Vec<_>>()
        );
        assert!(
            helpers_mapping.unwrap().test_files.contains(&test_file),
            "test_helpers.py should map to helpers.py (fallback, no FN)"
        );
    }

    // -----------------------------------------------------------------------
    // PY-AF-10: E2E — third_party_http_client pattern, FP reduction confirmed
    //
    // Fixture: af_e2e_http/
    //   pkg/http_client.py (HttpClient, HttpResponse) <- primary SUT
    //   pkg/exceptions.py  (RequestError)             <- incidental (pytest.raises)
    //   tests/test_http_client.py: asserts response.ok, response.status_code == 201
    //
    // HttpClient is reachable via chain (response -> client -> HttpClient).
    // exceptions.py: RequestError appears inside pytest.raises() which IS an
    // assertion node, so it will be in asserted_imports.
    // This test verifies http_client.py is always mapped (no FN on primary SUT).
    // -----------------------------------------------------------------------
    #[test]
    fn py_af_10_e2e_http_client_primary_mapped() {
        use std::path::PathBuf;
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/python/observe/af_e2e_http");

        let test_file = fixture_root
            .join("tests/test_http_client.py")
            .to_string_lossy()
            .into_owned();
        let http_client_prod = fixture_root
            .join("pkg/http_client.py")
            .to_string_lossy()
            .into_owned();
        let exceptions_prod = fixture_root
            .join("pkg/exceptions.py")
            .to_string_lossy()
            .into_owned();

        let production_files = vec![http_client_prod.clone(), exceptions_prod.clone()];
        let test_source =
            std::fs::read_to_string(&test_file).expect("fixture test file must exist");
        let mut test_sources = HashMap::new();
        test_sources.insert(test_file.clone(), test_source);

        // When: map_test_files_with_imports is called
        let extractor = PythonExtractor::new();
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            &fixture_root,
            false,
        );

        // Then: http_client.py IS mapped (primary SUT, must not be a FN)
        let http_client_mapping = result
            .iter()
            .find(|m| m.production_file == http_client_prod);
        assert!(
            http_client_mapping.is_some(),
            "http_client.py should be in mappings; got {:?}",
            result
                .iter()
                .map(|m| &m.production_file)
                .collect::<Vec<_>>()
        );
        assert!(
            http_client_mapping.unwrap().test_files.contains(&test_file),
            "test_http_client.py should map to http_client.py (primary SUT)"
        );
    }

    // -----------------------------------------------------------------------
    // PY-E2E-HELPER: test helper excluded from mappings
    // -----------------------------------------------------------------------
    #[test]
    fn py_e2e_helper_excluded_from_mappings() {
        // Given: tests/helpers.py is a test helper imported by tests/test_client.py
        //        pkg/client.py is the production SUT
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Write fixture files
        let files: &[(&str, &str)] = &[
            ("pkg/__init__.py", ""),
            ("pkg/client.py", "class Client:\n    def connect(self):\n        return True\n"),
            ("tests/__init__.py", ""),
            ("tests/helpers.py", "def mock_client():\n    return \"mock\"\n"),
            (
                "tests/test_client.py",
                "from pkg.client import Client\nfrom tests.helpers import mock_client\n\ndef test_connect():\n    client = Client()\n    assert client.connect()\n\ndef test_with_mock():\n    mc = mock_client()\n    assert mc == \"mock\"\n",
            ),
        ];
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content).unwrap();
        }

        let extractor = PythonExtractor::new();

        // production_files: pkg/client.py and tests/helpers.py
        // (discover_files would put helpers.py in production_files since it's not test_*.py)
        let client_abs = root.join("pkg/client.py").to_string_lossy().into_owned();
        let helpers_abs = root.join("tests/helpers.py").to_string_lossy().into_owned();
        let production_files = vec![client_abs.clone(), helpers_abs.clone()];

        let test_abs = root
            .join("tests/test_client.py")
            .to_string_lossy()
            .into_owned();
        let test_content = "from pkg.client import Client\nfrom tests.helpers import mock_client\n\ndef test_connect():\n    client = Client()\n    assert client.connect()\n\ndef test_with_mock():\n    mc = mock_client()\n    assert mc == \"mock\"\n";
        let test_sources: HashMap<String, String> = [(test_abs.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, root, false);

        // Then: tests/helpers.py should NOT appear as a production_file in any mapping
        for m in &mappings {
            assert!(
                !m.production_file.contains("helpers.py"),
                "helpers.py should be excluded as test helper, but found in mapping: {:?}",
                m
            );
        }

        // Then: pkg/client.py SHOULD be mapped to test_client.py
        let client_mapping = mappings
            .iter()
            .find(|m| m.production_file.contains("client.py"));
        assert!(
            client_mapping.is_some(),
            "pkg/client.py should be mapped; got {:?}",
            mappings
                .iter()
                .map(|m| &m.production_file)
                .collect::<Vec<_>>()
        );
        let client_mapping = client_mapping.unwrap();
        assert!(
            client_mapping
                .test_files
                .iter()
                .any(|t| t.contains("test_client.py")),
            "pkg/client.py should map to test_client.py; got {:?}",
            client_mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-FP-01: MockTransport fixture re-exported via barrel should NOT be mapped.
    //
    // is_non_sut_helper excludes mock*.py files from production_files.
    // -----------------------------------------------------------------------
    #[test]
    fn py_fp_01_mock_transport_fixture_not_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let pkg = root.join("pkg");
        let transports = pkg.join("_transports");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&transports).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            transports.join("mock.py"),
            "class MockTransport:\n    pass\n",
        )
        .unwrap();
        std::fs::write(
            transports.join("__init__.py"),
            "from .mock import MockTransport\n",
        )
        .unwrap();
        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._transports import *\nfrom ._client import Client\n",
        )
        .unwrap();

        let test_content = "import pkg\n\ndef test_hooks():\n    client = pkg.Client(transport=pkg.MockTransport())\n    assert client is not None\n";
        std::fs::write(tests_dir.join("test_hooks.py"), test_content).unwrap();

        let mock_path = transports.join("mock.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_hooks.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![mock_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, root, false);

        let mock_mapping = result.iter().find(|m| m.production_file == mock_path);
        assert!(
            mock_mapping.is_none() || mock_mapping.unwrap().test_files.is_empty(),
            "mock.py should NOT be mapped (fixture); mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-FP-02: __version__.py incidental should NOT be mapped.
    //
    // is_non_sut_helper excludes __version__.py from production_files.
    // -----------------------------------------------------------------------
    #[test]
    fn py_fp_02_version_py_incidental_not_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let pkg = root.join("pkg");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("__version__.py"), "__version__ = \"1.0.0\"\n").unwrap();
        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from .__version__ import __version__\nfrom ._client import Client\n",
        )
        .unwrap();

        let test_content = "import pkg\n\ndef test_headers():\n    expected = f\"python-pkg/{pkg.__version__}\"\n    assert expected == \"python-pkg/1.0.0\"\n";
        std::fs::write(tests_dir.join("test_headers.py"), test_content).unwrap();

        let version_path = pkg.join("__version__.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_headers.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![version_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, root, false);

        let version_mapping = result.iter().find(|m| m.production_file == version_path);
        assert!(
            version_mapping.is_none() || version_mapping.unwrap().test_files.is_empty(),
            "__version__.py should NOT be mapped (metadata); mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-FP-03: _types.py type-annotation-only should NOT be mapped.
    //
    // is_non_sut_helper excludes _types.py from production_files.
    // -----------------------------------------------------------------------
    #[test]
    fn py_fp_03_types_py_annotation_not_mapped() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let pkg = root.join("pkg");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(
            pkg.join("_types.py"),
            "from typing import Union\nQueryParamTypes = Union[str, dict]\n",
        )
        .unwrap();
        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._types import *\nfrom ._client import Client\n",
        )
        .unwrap();

        let test_content = "import pkg\n\ndef test_client():\n    client = pkg.Client()\n    assert client is not None\n";
        std::fs::write(tests_dir.join("test_client.py"), test_content).unwrap();

        let types_path = pkg.join("_types.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_client.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![types_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, root, false);

        let types_mapping = result.iter().find(|m| m.production_file == types_path);
        assert!(
            types_mapping.is_none() || types_mapping.unwrap().test_files.is_empty(),
            "_types.py should NOT be mapped (type definitions); mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-RESOLVE-PRIORITY-01: file wins over package when both exist
    // -----------------------------------------------------------------------
    #[test]
    fn py_resolve_priority_01_file_wins_over_package() {
        // Given: tempdir with both foo/bar/baz.py and foo/bar/baz/__init__.py
        let tmp = tempfile::tempdir().unwrap();
        let baz_dir = tmp.path().join("foo").join("bar").join("baz");
        std::fs::create_dir_all(&baz_dir).unwrap();
        let baz_file = tmp.path().join("foo").join("bar").join("baz.py");
        std::fs::write(&baz_file, "class Baz: pass\n").unwrap();
        let baz_init = baz_dir.join("__init__.py");
        std::fs::write(&baz_init, "from .impl import Baz\n").unwrap();

        let canonical_root = tmp.path().canonicalize().unwrap();
        let base = tmp.path().join("foo").join("bar").join("baz");
        let extractor = PythonExtractor::new();

        // When: resolve_absolute_base_to_file is called
        let result =
            exspec_core::observe::resolve_absolute_base_to_file(&extractor, &base, &canonical_root);

        // Then: resolves to baz.py (file), not baz/__init__.py (package)
        assert!(result.is_some(), "expected resolution, got None");
        let resolved = result.unwrap();
        assert!(
            resolved.ends_with("baz.py"),
            "expected baz.py (file wins over package), got: {resolved}"
        );
        assert!(
            !resolved.contains("__init__"),
            "should NOT resolve to __init__.py, got: {resolved}"
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-01: direct import + barrel coexist, assertion filter bypass
    //
    // `from pkg._urlparse import normalize` is a direct import to _urlparse.py.
    // Even though `normalize` does not appear in `assert pkg.URL(...)`,
    // _urlparse.py SHOULD be mapped because it was directly imported (not via barrel).
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_01_direct_import_bypasses_assertion_filter() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/_urlparse.py, pkg/_client.py, pkg/__init__.py (re-exports _client only)
        //        tests/test_whatwg.py:
        //          from pkg._urlparse import normalize   <- direct import (not in assertion)
        //          import pkg                            <- barrel import
        //          def test_url():
        //              assert pkg.URL("http://example.com")  <- assertion uses URL (from _client)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_urlparse.py"), "def normalize(url): return url\n").unwrap();
        std::fs::write(pkg.join("_client.py"), "class URL:\n    pass\n").unwrap();
        // __init__.py re-exports _client only (NOT _urlparse)
        std::fs::write(pkg.join("__init__.py"), "from ._client import URL\n").unwrap();

        let test_content = "from pkg._urlparse import normalize\nimport pkg\n\ndef test_url():\n    assert pkg.URL(\"http://example.com\")\n";
        std::fs::write(tests_dir.join("test_whatwg.py"), test_content).unwrap();

        let urlparse_path = pkg.join("_urlparse.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_whatwg.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![urlparse_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _urlparse.py IS mapped (direct import bypasses assertion filter)
        let urlparse_mapped = result
            .iter()
            .find(|m| m.production_file == urlparse_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            urlparse_mapped,
            "pkg/_urlparse.py should be mapped via direct import (assertion filter bypass). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-02: un-re-exported sub-module with direct import
    //
    // `from pkg._internal import helper` imports a module that is NOT in __init__.py.
    // Since it is directly imported and helper() appears in assertion, _internal.py SHOULD be mapped.
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_02_unre_exported_direct_import_mapped() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/_internal.py (NOT in __init__.py), tests/test_internal.py:
        //          from pkg._internal import helper
        //          def test_it():
        //              assert helper()
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_internal.py"), "def helper(): return True\n").unwrap();
        // __init__.py does NOT re-export _internal
        std::fs::write(pkg.join("__init__.py"), "# empty barrel\n").unwrap();

        let test_content =
            "from pkg._internal import helper\n\ndef test_it():\n    assert helper()\n";
        std::fs::write(tests_dir.join("test_internal.py"), test_content).unwrap();

        let internal_path = pkg.join("_internal.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_internal.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![internal_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _internal.py IS mapped
        let internal_mapped = result
            .iter()
            .find(|m| m.production_file == internal_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            internal_mapped,
            "pkg/_internal.py should be mapped via direct import. mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-03: nested sub-module direct import
    //
    // `from pkg._internal._helpers import util` imports a nested sub-module.
    // _helpers.py SHOULD be mapped.
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_03_nested_submodule_direct_import_mapped() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/_internal/_helpers.py, tests/test_helpers.py:
        //          from pkg._internal._helpers import util
        //          def test_util():
        //              assert util()
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        let internal = pkg.join("_internal");
        std::fs::create_dir_all(&internal).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(internal.join("_helpers.py"), "def util(): return True\n").unwrap();
        std::fs::write(internal.join("__init__.py"), "# empty\n").unwrap();
        std::fs::write(pkg.join("__init__.py"), "# empty barrel\n").unwrap();

        let test_content =
            "from pkg._internal._helpers import util\n\ndef test_util():\n    assert util()\n";
        std::fs::write(tests_dir.join("test_helpers.py"), test_content).unwrap();

        let helpers_path = internal.join("_helpers.py").to_string_lossy().into_owned();
        let test_path = tests_dir
            .join("test_helpers.py")
            .to_string_lossy()
            .into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![helpers_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _helpers.py IS mapped
        let helpers_mapped = result
            .iter()
            .find(|m| m.production_file == helpers_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            helpers_mapped,
            "pkg/_internal/_helpers.py should be mapped via nested direct import. mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-05: non-bare relative direct import bypass
    //
    // `from ._config import Config` is a non-bare relative direct import.
    // Even though `Config` does not appear in assertions (only `Client` is
    // asserted, which comes from the barrel), _config.py SHOULD be mapped
    // because direct_import_indices bypass the assertion filter.
    // (Fixed in #146: relative import branches now populate direct_import_indices)
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_05_non_bare_relative_direct_import_bypass() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/_config.py (non-barrel production file, has Config)
        //        pkg/_client.py (non-barrel production file, has Client)
        //        pkg/__init__.py (barrel: re-exports Client from ._client)
        //        pkg/test_app.py (stem "app", no L1 match to _config or _client):
        //          import pkg                    <- barrel import
        //          from ._config import Config   <- non-bare relative direct import
        //          def test_something():
        //              assert pkg.Client()       <- assertion uses Client (from _client),
        //                                           NOT Config (from _config)
        //
        // Key: test file is named "test_app.py" (stem "app") so L1 stem matching
        //      does NOT match _config.py (stem "config") or _client.py (stem "client").
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(pkg.join("_config.py"), "class Config:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        // __init__.py re-exports Client (NOT Config)
        std::fs::write(pkg.join("__init__.py"), "from ._client import Client\n").unwrap();

        let test_content = "import pkg\nfrom ._config import Config\n\ndef test_something():\n    assert pkg.Client()\n";
        std::fs::write(pkg.join("test_app.py"), test_content).unwrap();

        let config_path = pkg.join("_config.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = pkg.join("test_app.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![config_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _config.py IS mapped (non-bare relative direct import bypasses assertion filter)
        let config_mapped = result
            .iter()
            .find(|m| m.production_file == config_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            config_mapped,
            "pkg/_config.py should be mapped via non-bare relative direct import (assertion filter bypass). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-06: bare relative direct import bypass
    //
    // `from . import utils` is a bare relative direct import.
    // Even though `utils` does not appear in assertions (only `Client` is
    // asserted, which comes from the barrel), utils.py SHOULD be mapped
    // because direct_import_indices bypass the assertion filter.
    // (Fixed in #146: relative import branches now populate direct_import_indices)
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_06_bare_relative_direct_import_bypass() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/utils.py (non-barrel production file, has helper)
        //        pkg/_client.py (non-barrel production file, has Client)
        //        pkg/__init__.py (barrel: re-exports Client from ._client)
        //        pkg/test_app.py (stem "app", no L1 match to utils or _client):
        //          import pkg              <- barrel import
        //          from . import utils     <- bare relative direct import
        //          def test_something():
        //              assert pkg.Client() <- assertion uses Client (from _client),
        //                                    NOT utils.helper
        //
        // Key: test file is named "test_app.py" (stem "app") so L1 stem matching
        //      does NOT match utils.py (stem "utils") or _client.py (stem "client").
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(pkg.join("utils.py"), "def helper(): return True\n").unwrap();
        std::fs::write(pkg.join("_client.py"), "class Client:\n    pass\n").unwrap();
        // __init__.py re-exports Client (NOT utils)
        std::fs::write(pkg.join("__init__.py"), "from ._client import Client\n").unwrap();

        let test_content =
            "import pkg\nfrom . import utils\n\ndef test_something():\n    assert pkg.Client()\n";
        std::fs::write(pkg.join("test_app.py"), test_content).unwrap();

        let utils_path = pkg.join("utils.py").to_string_lossy().into_owned();
        let client_path = pkg.join("_client.py").to_string_lossy().into_owned();
        let test_path = pkg.join("test_app.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![utils_path.clone(), client_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: utils.py IS mapped (bare relative direct import bypasses assertion filter)
        let utils_mapped = result
            .iter()
            .find(|m| m.production_file == utils_path)
            .map(|m| m.test_files.contains(&test_path))
            .unwrap_or(false);
        assert!(
            utils_mapped,
            "pkg/utils.py should be mapped via bare relative direct import (assertion filter bypass). mappings={:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // PY-SUBMOD-04: regression — barrel-only import still filtered by assertion
    //
    // `import pkg` + `assert pkg.Config()` → _config.py IS mapped.
    // `import pkg` + no assertion on Model → _models.py is NOT mapped.
    // Assertion filter continues to work for barrel imports.
    // -----------------------------------------------------------------------
    #[test]
    fn py_submod_04_regression_barrel_only_assertion_filter_preserved() {
        use std::collections::HashMap;
        use tempfile::TempDir;

        // Given: pkg/_config.py, pkg/_models.py, pkg/__init__.py (re-exports both)
        //        tests/test_foo.py:
        //          import pkg
        //          def test_foo():
        //              assert pkg.Config()   <- assertion uses Config (from _config.py)
        //                                   <- no assertion on Model (from _models.py)
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        let tests_dir = dir.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        std::fs::write(pkg.join("_config.py"), "class Config:\n    pass\n").unwrap();
        std::fs::write(pkg.join("_models.py"), "class Model:\n    pass\n").unwrap();
        // __init__.py re-exports both
        std::fs::write(
            pkg.join("__init__.py"),
            "from ._config import Config\nfrom ._models import Model\n",
        )
        .unwrap();

        let test_content = "import pkg\n\ndef test_foo():\n    assert pkg.Config()\n";
        std::fs::write(tests_dir.join("test_foo.py"), test_content).unwrap();

        let config_path = pkg.join("_config.py").to_string_lossy().into_owned();
        let models_path = pkg.join("_models.py").to_string_lossy().into_owned();
        let test_path = tests_dir.join("test_foo.py").to_string_lossy().into_owned();

        let extractor = PythonExtractor::new();
        let production_files = vec![config_path.clone(), models_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_content.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result = extractor.map_test_files_with_imports(
            &production_files,
            &test_sources,
            dir.path(),
            false,
        );

        // Then: _models.py is NOT mapped (barrel import, assertion filter still applies)
        let models_not_mapped = result
            .iter()
            .find(|m| m.production_file == models_path)
            .map(|m| !m.test_files.contains(&test_path))
            .unwrap_or(true);
        assert!(
            models_not_mapped,
            "pkg/_models.py should NOT be mapped (barrel import, no assertion on Model). mappings={:?}",
            result
        );
    }
}
