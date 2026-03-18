use std::collections::HashMap;
use std::path::Path;
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
                            exspec_core::observe::collect_import_matches(
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
                    exspec_core::observe::collect_import_matches(
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
                let resolved = exspec_core::observe::resolve_absolute_base_to_file(
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
                if let Some(resolved) = resolved {
                    exspec_core::observe::collect_import_matches(
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

        let mappings =
            extractor.map_test_files_with_imports(&production_files, &test_sources, tmp.path());

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
        // Given: `from .models import X` in tests/test_something.py,
        //        tests/models.py exists relative to test file
        let r = run_import_test(
            "tests/models.py",
            "class X:\n    pass\n",
            "tests/test_something.py",
            "from .models import X\n\ndef test_x():\n    pass\n",
            &[],
        );

        // Then: models.py is mapped to test_something.py (relative import resolves from parent dir)
        let mapping = r.mappings.iter().find(|m| m.production_file == r.prod_path);
        assert!(
            mapping.is_some(),
            "tests/models.py not found in mappings: {:?}",
            r.mappings
        );
        let mapping = mapping.unwrap();
        assert!(
            mapping.test_files.contains(&r.test_path),
            "test_something.py not in test_files for tests/models.py: {:?}",
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
        // Layer 1 does not match because prod dir (src/mypackage) != test dir (tests),
        // so this is resolved via Layer 2 (ImportTracing) with src/ fallback.
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
        assert_eq!(mapping.strategy, MappingStrategy::ImportTracing);
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
        // Layer 1 does not match because prod dir (mypackage) != test dir (tests),
        // so this is resolved via Layer 2 (ImportTracing) without src/ fallback.
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
        assert_eq!(mapping.strategy, MappingStrategy::ImportTracing);
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
        let cars_m = cars_mapping.unwrap();
        assert!(
            cars_m.test_files.contains(&test_path),
            "test_mixed.py not mapped to models/cars.py via absolute import: {:?}",
            cars_m.test_files
        );

        // Then: tests/helpers.py is mapped via relative import (Layer 2)
        let helpers_mapping = result.iter().find(|m| m.production_file == helpers_prod);
        assert!(
            helpers_mapping.is_some(),
            "tests/helpers.py not found in mappings: {:?}",
            result
        );
        let helpers_m = helpers_mapping.unwrap();
        assert!(
            helpers_m.test_files.contains(&test_path),
            "test_mixed.py not mapped to tests/helpers.py via relative import: {:?}",
            helpers_m.test_files
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
    let mut seen = std::collections::HashSet::new();

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
    let mut seen = std::collections::HashSet::new();

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
}
