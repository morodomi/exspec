use std::collections::HashMap;
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
    None
}

/// Extract stem from a production file path.
/// `user.py` -> `Some("user")`
/// `__init__.py` -> `None`
/// `test_user.py` -> `None`
pub fn production_stem(path: &str) -> Option<&str> {
    let file_name = Path::new(path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".py")?;
    // Exclude __init__.py
    if stem == "__init__" {
        return None;
    }
    // Exclude test files
    if stem.starts_with("test_") || stem.ends_with("_test") {
        return None;
    }
    Some(stem)
}

/// Determine if a file is a non-SUT helper (should be excluded from mapping).
pub fn is_non_sut_helper(file_path: &str, is_known_production: bool) -> bool {
    // If the file is already known to be a production file, it's not a helper.
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

    // Files directly inside tests/ or test/ or __pycache__/ that are NOT test files.
    let parent_is_test_dir = Path::new(file_path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .map(|s| s == "tests" || s == "test" || s == "__pycache__")
        .unwrap_or(false);

    if parent_is_test_dir && !file_name.starts_with("test_") && !file_name.ends_with("_test.py") {
        return true;
    }

    false
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
        let mut seen = std::collections::HashSet::new();
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

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        let mut specifier_symbols: HashMap<String, Vec<String>> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut module_text: Option<String> = None;
            let mut symbol_text: Option<String> = None;

            for cap in m.captures {
                if module_name_idx == Some(cap.index) {
                    module_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                } else if symbol_name_idx == Some(cap.index) {
                    symbol_text = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                }
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

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        // Group symbols by from_specifier
        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();

        while let Some(m) = matches.next() {
            let mut from_spec: Option<String> = None;
            let mut sym: Option<String> = None;

            for cap in m.captures {
                if cap.index == from_specifier_idx {
                    let raw = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                    from_spec = Some(python_module_to_relative_specifier(&raw));
                } else if symbol_name_idx == Some(cap.index) {
                    sym = Some(cap.node.utf8_text(source_bytes).unwrap_or("").to_string());
                }
            }

            if let (Some(spec), Some(symbol)) = (from_spec, sym) {
                // Only include relative re-exports
                if spec.starts_with("./") || spec.starts_with("../") {
                    grouped.entry(spec).or_default().push(symbol);
                }
            }
        }

        grouped
            .into_iter()
            .map(|(from_specifier, symbols)| BarrelReExport {
                symbols,
                from_specifier,
                wildcard: false,
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

impl PythonExtractor {
    /// Layer 1 + Layer 2: Map test files to production files.
    pub fn map_test_files_with_imports(
        &self,
        production_files: &[String],
        test_sources: &HashMap<String, String>,
        scan_root: &Path,
    ) -> Vec<FileMapping> {
        let test_file_list: Vec<String> = test_sources.keys().cloned().collect();

        // Layer 1: filename convention
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

        // Layer 2: import tracing
        for (test_file, source) in test_sources {
            let imports = <Self as ObserveExtractor>::extract_imports(self, source, test_file);
            let from_file = Path::new(test_file);
            let mut matched_indices = std::collections::HashSet::<usize>::new();

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
                            collect_import_matches(
                                self,
                                &resolved,
                                &import.symbols,
                                &canonical_to_idx,
                                &mut matched_indices,
                                &canonical_root,
                            );
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
                    collect_import_matches(
                        self,
                        &resolved,
                        &import.symbols,
                        &canonical_to_idx,
                        &mut matched_indices,
                        &canonical_root,
                    );
                }
            }

            // Layer 2 (absolute imports): resolve from scan_root
            let abs_specifiers = self.extract_all_import_specifiers(source);
            for (specifier, symbols) in &abs_specifiers {
                let base = canonical_root.join(specifier);
                if let Some(resolved) = exspec_core::observe::resolve_absolute_base_to_file(
                    self,
                    &base,
                    &canonical_root,
                ) {
                    collect_import_matches(
                        self,
                        &resolved,
                        symbols,
                        &canonical_to_idx,
                        &mut matched_indices,
                        &canonical_root,
                    );
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

/// Helper: given a resolved file path, follow barrel re-exports if needed and
/// collect matching production-file indices.
fn collect_import_matches(
    ext: &PythonExtractor,
    resolved: &str,
    symbols: &[String],
    canonical_to_idx: &HashMap<String, usize>,
    indices: &mut std::collections::HashSet<usize>,
    canonical_root: &Path,
) {
    if ext.is_barrel_file(resolved) {
        let barrel_path = PathBuf::from(resolved);
        let resolved_files = exspec_core::observe::resolve_barrel_exports(
            ext,
            &barrel_path,
            symbols,
            canonical_root,
        );
        for prod in resolved_files {
            let prod_str = prod.to_string_lossy().into_owned();
            if !ext.is_non_sut_helper(&prod_str, canonical_to_idx.contains_key(&prod_str)) {
                if let Some(&idx) = canonical_to_idx.get(&prod_str) {
                    indices.insert(idx);
                }
            }
        }
    } else if !ext.is_non_sut_helper(resolved, canonical_to_idx.contains_key(resolved)) {
        if let Some(&idx) = canonical_to_idx.get(resolved) {
            indices.insert(idx);
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

        // Then: "os" is not in the result (plain imports are skipped)
        let os_entry = result.iter().find(|(spec, _)| spec == "os");
        assert!(
            os_entry.is_none(),
            "plain 'import os' should be skipped, got {:?}",
            result
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
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, &scan_root);

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
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, &fixture_root);

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
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, &scan_root);

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
    // PY-ABS-01: `from models.cars import Car` -> mapped to models/cars.py via Layer 2
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_01_absolute_import_nested_module() {
        // Given: `from models.cars import Car` in tests/unit/test_car.py,
        //        models/cars.py exists at scan_root
        let tmp = tempfile::tempdir().unwrap();
        let models_dir = tmp.path().join("models");
        let tests_unit_dir = tmp.path().join("tests").join("unit");
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::create_dir_all(&tests_unit_dir).unwrap();

        let cars_py = models_dir.join("cars.py");
        std::fs::write(&cars_py, "class Car:\n    pass\n").unwrap();

        let test_car_py = tests_unit_dir.join("test_car.py");
        let test_source = "from models.cars import Car\n\ndef test_car():\n    pass\n";
        std::fs::write(&test_car_py, test_source).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = cars_py.to_string_lossy().into_owned();
        let test_path = test_car_py.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

        // Then: models/cars.py is mapped to test_car.py via Layer 2 (ImportTracing)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(
            mapping.is_some(),
            "models/cars.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
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
        let tmp = tempfile::tempdir().unwrap();
        let utils_dir = tmp.path().join("utils");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&utils_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        let publish_state_py = utils_dir.join("publish_state.py");
        std::fs::write(&publish_state_py, "class PublishState:\n    pass\n").unwrap();

        let test_pub_py = tests_dir.join("test_pub.py");
        let test_source =
            "from utils.publish_state import PublishState\n\ndef test_pub():\n    pass\n";
        std::fs::write(&test_pub_py, test_source).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = publish_state_py.to_string_lossy().into_owned();
        let test_path = test_pub_py.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

        // Then: utils/publish_state.py is mapped to test_pub.py via Layer 2
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(
            mapping.is_some(),
            "utils/publish_state.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
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
        // Given: `from .models import X` in tests/test_something.py,
        //        tests/models.py exists relative to test file
        let tmp = tempfile::tempdir().unwrap();
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();

        let models_py = tests_dir.join("models.py");
        std::fs::write(&models_py, "class X:\n    pass\n").unwrap();

        let test_py = tests_dir.join("test_something.py");
        let test_source = "from .models import X\n\ndef test_x():\n    pass\n";
        std::fs::write(&test_py, test_source).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = models_py.to_string_lossy().into_owned();
        let test_path = test_py.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

        // Then: models.py is mapped to test_something.py (relative import resolves from parent dir)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        assert!(
            mapping.is_some(),
            "tests/models.py not found in mappings: {:?}",
            result
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&test_path),
            "test_something.py not in test_files for tests/models.py: {:?}",
            mapping.test_files
        );
    }

    // -----------------------------------------------------------------------
    // PY-ABS-04: `from nonexistent.module import X` -> no mapping added (graceful skip)
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_04_nonexistent_absolute_import_skipped() {
        // Given: `from nonexistent.module import X` in test file,
        //        nonexistent/module.py does NOT exist at scan_root
        let tmp = tempfile::tempdir().unwrap();
        let models_dir = tmp.path().join("models");
        let tests_dir = tmp.path().join("tests");
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        // A real production file to have something in production_files
        let real_py = models_dir.join("real.py");
        std::fs::write(&real_py, "class Real:\n    pass\n").unwrap();

        let test_py = tests_dir.join("test_missing.py");
        let test_source = "from nonexistent.module import X\n\ndef test_x():\n    pass\n";
        std::fs::write(&test_py, test_source).unwrap();

        let extractor = PythonExtractor::new();
        let prod_path = real_py.to_string_lossy().into_owned();
        let test_path = test_py.to_string_lossy().into_owned();
        let production_files = vec![prod_path.clone()];
        let test_sources: HashMap<String, String> = [(test_path.clone(), test_source.to_string())]
            .into_iter()
            .collect();

        // When: map_test_files_with_imports is called
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

        // Then: test_missing.py is NOT mapped to models/real.py (unresolvable import skipped)
        let mapping = result.iter().find(|m| m.production_file == prod_path);
        if let Some(mapping) = mapping {
            assert!(
                !mapping.test_files.contains(&test_path),
                "test_missing.py should NOT be mapped to models/real.py: {:?}",
                mapping.test_files
            );
        }
        // passing if no mapping or test_path not in mapping
    }

    // -----------------------------------------------------------------------
    // PY-ABS-05: mixed absolute + relative imports in same test file -> both resolved
    // -----------------------------------------------------------------------
    #[test]
    fn py_abs_05_mixed_absolute_and_relative_imports() {
        // Given: a test file with both `from models.cars import Car` (absolute)
        //        and `from .helpers import setup` (relative),
        //        models/cars.py and tests/helpers.py both exist at scan_root
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
        let result =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

        // Then: models/cars.py is mapped via absolute import (Layer 2)
        let cars_mapping = result.iter().find(|m| m.production_file == cars_prod);
        assert!(
            cars_mapping.is_some(),
            "models/cars.py not found in mappings: {:?}",
            result
        );
        assert!(
            cars_mapping.unwrap().test_files.contains(&test_path),
            "test_mixed.py not mapped to models/cars.py via absolute import: {:?}",
            cars_mapping.unwrap().test_files
        );

        // Then: tests/helpers.py is mapped via relative import (Layer 2)
        let helpers_mapping = result.iter().find(|m| m.production_file == helpers_prod);
        assert!(
            helpers_mapping.is_some(),
            "tests/helpers.py not found in mappings: {:?}",
            result
        );
        assert!(
            helpers_mapping.unwrap().test_files.contains(&test_path),
            "test_mixed.py not mapped to tests/helpers.py via relative import: {:?}",
            helpers_mapping.unwrap().test_files
        );
    }
}
