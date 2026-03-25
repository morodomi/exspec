use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionFunction {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub class_name: Option<String>,
    pub is_exported: bool,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ImportMapping {
    pub symbol_name: String,
    pub module_specifier: String,
    pub file: String,
    pub line: usize,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BarrelReExport {
    pub symbols: Vec<String>,
    pub from_specifier: String,
    pub wildcard: bool,
    /// True when this is a namespace re-export (`export * as Ns from '...'`).
    /// The namespace name changes the symbol space, so nested resolution must
    /// treat this as an opaque wildcard (symbols dropped on recursion).
    pub namespace_wildcard: bool,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

pub trait ObserveExtractor: Send + Sync {
    fn extract_production_functions(
        &self,
        source: &str,
        file_path: &str,
    ) -> Vec<ProductionFunction>;
    fn extract_imports(&self, source: &str, file_path: &str) -> Vec<ImportMapping>;
    fn extract_all_import_specifiers(&self, source: &str) -> Vec<(String, Vec<String>)>;
    fn extract_barrel_re_exports(&self, source: &str, file_path: &str) -> Vec<BarrelReExport>;
    fn source_extensions(&self) -> &[&str];
    fn index_file_names(&self) -> &[&str];
    fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str>;
    fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str>;
    fn is_non_sut_helper(&self, file_path: &str, is_known_production: bool) -> bool;

    // Default implementations
    fn is_barrel_file(&self, path: &str) -> bool {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        self.index_file_names().contains(&file_name)
    }

    fn file_exports_any_symbol(&self, _path: &Path, _symbols: &[String]) -> bool {
        true
    }

    fn resolve_alias_imports(
        &self,
        _source: &str,
        _scan_root: &Path,
    ) -> Vec<(String, Vec<String>, Option<PathBuf>)> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

pub const MAX_BARREL_DEPTH: usize = 3;

/// Layer 1: Map test files to production files by filename convention (stem matching).
pub fn map_test_files(
    ext: &dyn ObserveExtractor,
    production_files: &[String],
    test_files: &[String],
) -> Vec<FileMapping> {
    let mut tests_by_key: HashMap<(String, String), Vec<String>> = HashMap::new();

    for test_file in test_files {
        let Some(stem) = ext.test_stem(test_file) else {
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
            let test_matches = ext
                .production_stem(production_file)
                .and_then(|stem| {
                    let directory = Path::new(production_file)
                        .parent()
                        .map(|parent| parent.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    tests_by_key.get(&(directory, stem.to_string()))
                })
                .cloned()
                .unwrap_or_default();
            FileMapping {
                production_file: production_file.clone(),
                test_files: test_matches,
                strategy: MappingStrategy::FileNameConvention,
            }
        })
        .collect()
}

/// Resolve a module specifier to an absolute file path.
/// Returns None if the file does not exist or is outside scan_root.
pub fn resolve_import_path(
    ext: &dyn ObserveExtractor,
    module_specifier: &str,
    from_file: &Path,
    scan_root: &Path,
) -> Option<String> {
    let base_dir_raw = from_file.parent()?;
    let base_dir = base_dir_raw
        .canonicalize()
        .unwrap_or_else(|_| base_dir_raw.to_path_buf());
    let raw_path = base_dir.join(module_specifier);
    let canonical_root = scan_root.canonicalize().ok()?;
    resolve_absolute_base_to_file(ext, &raw_path, &canonical_root)
}

/// Resolve an already-computed absolute base path to an actual source file.
///
/// Probes in order:
/// 1. Direct hit (when `base` already has a known extension).
/// 2. Append each known extension.
/// 3. Directory index fallback.
pub fn resolve_absolute_base_to_file(
    ext: &dyn ObserveExtractor,
    base: &Path,
    canonical_root: &Path,
) -> Option<String> {
    let extensions = ext.source_extensions();
    let has_known_ext = base
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.contains(&e));

    let candidates: Vec<PathBuf> = if has_known_ext {
        vec![base.to_path_buf()]
    } else {
        let base_str = base.as_os_str().to_string_lossy();
        extensions
            .iter()
            .map(|e| PathBuf::from(format!("{base_str}.{e}")))
            .collect()
    };

    for candidate in &candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            if canonical.starts_with(canonical_root) {
                return Some(canonical.to_string_lossy().into_owned());
            }
        }
    }

    // Fallback: directory index
    if !has_known_ext {
        let base_str = base.as_os_str().to_string_lossy();
        for index_name in ext.index_file_names() {
            let candidate = PathBuf::from(format!("{base_str}/{index_name}"));
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.starts_with(canonical_root) {
                    return Some(canonical.to_string_lossy().into_owned());
                }
            }
        }
    }

    None
}

/// Resolve barrel re-exports starting from `barrel_path` for the given `symbols`.
/// Follows up to MAX_BARREL_DEPTH hops, prevents cycles via `visited` set.
pub fn resolve_barrel_exports(
    ext: &dyn ObserveExtractor,
    barrel_path: &Path,
    symbols: &[String],
    scan_root: &Path,
) -> Vec<PathBuf> {
    let canonical_root = match scan_root.canonicalize() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut results: Vec<PathBuf> = Vec::new();
    resolve_barrel_exports_inner(
        ext,
        barrel_path,
        symbols,
        scan_root,
        &canonical_root,
        &mut visited,
        0,
        &mut results,
    );
    results
}

#[allow(clippy::too_many_arguments)]
fn resolve_barrel_exports_inner(
    ext: &dyn ObserveExtractor,
    barrel_path: &Path,
    symbols: &[String],
    scan_root: &Path,
    canonical_root: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
    results: &mut Vec<PathBuf>,
) {
    if depth >= MAX_BARREL_DEPTH {
        return;
    }

    let canonical_barrel = match barrel_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return,
    };
    if !visited.insert(canonical_barrel) {
        return;
    }

    let source = match std::fs::read_to_string(barrel_path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let re_exports = ext.extract_barrel_re_exports(&source, &barrel_path.to_string_lossy());

    for re_export in &re_exports {
        if !re_export.wildcard {
            let has_match =
                symbols.is_empty() || symbols.iter().any(|s| re_export.symbols.contains(s));
            if !has_match {
                continue;
            }
        }

        if let Some(resolved_str) =
            resolve_import_path(ext, &re_export.from_specifier, barrel_path, scan_root)
        {
            if ext.is_barrel_file(&resolved_str) {
                // Namespace re-export (`export * as Ns from '...'`) changes the symbol
                // namespace, so the caller's symbol names no longer match the nested
                // exports. Treat as opaque wildcard by passing empty symbols.
                let nested_symbols: &[String] = if re_export.namespace_wildcard {
                    &[]
                } else {
                    symbols
                };
                resolve_barrel_exports_inner(
                    ext,
                    &PathBuf::from(&resolved_str),
                    nested_symbols,
                    scan_root,
                    canonical_root,
                    visited,
                    depth + 1,
                    results,
                );
            } else if !ext.is_non_sut_helper(&resolved_str, false) {
                // Non-barrel path: namespace_wildcard does not change symbols filtering here.
                // The caller's symbols are used as-is for file_exports_any_symbol check.
                if !symbols.is_empty()
                    && re_export.wildcard
                    && !ext.file_exports_any_symbol(Path::new(&resolved_str), symbols)
                {
                    continue;
                }
                if let Ok(canonical) = PathBuf::from(&resolved_str).canonicalize() {
                    if canonical.starts_with(canonical_root) && !results.contains(&canonical) {
                        results.push(canonical);
                    }
                }
            }
        }
    }
}

/// Helper: given a resolved file path, follow barrel re-exports if needed and
/// collect matching production-file indices.
pub fn collect_import_matches(
    ext: &dyn ObserveExtractor,
    resolved: &str,
    symbols: &[String],
    canonical_to_idx: &HashMap<String, usize>,
    indices: &mut HashSet<usize>,
    canonical_root: &Path,
) {
    if ext.is_barrel_file(resolved) {
        let barrel_path = PathBuf::from(resolved);
        let resolved_files = resolve_barrel_exports(ext, &barrel_path, symbols, canonical_root);
        for prod in resolved_files {
            let prod_str = prod.to_string_lossy().into_owned();
            if !ext.is_non_sut_helper(&prod_str, canonical_to_idx.contains_key(&prod_str)) {
                if let Some(&idx) = canonical_to_idx.get(&prod_str) {
                    indices.insert(idx);
                }
            }
        }
        // Also check if the barrel file itself directly defines/exports the symbols
        if ext.file_exports_any_symbol(Path::new(resolved), symbols)
            && !ext.is_non_sut_helper(resolved, canonical_to_idx.contains_key(resolved))
        {
            if let Some(&idx) = canonical_to_idx.get(resolved) {
                indices.insert(idx);
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

    struct MockExtractor;

    impl ObserveExtractor for MockExtractor {
        fn extract_production_functions(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<ProductionFunction> {
            vec![]
        }
        fn extract_imports(&self, _source: &str, _file_path: &str) -> Vec<ImportMapping> {
            vec![]
        }
        fn extract_all_import_specifiers(&self, _source: &str) -> Vec<(String, Vec<String>)> {
            vec![]
        }
        fn extract_barrel_re_exports(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<BarrelReExport> {
            vec![]
        }
        fn source_extensions(&self) -> &[&str] {
            &["ts", "tsx", "js", "jsx"]
        }
        fn index_file_names(&self) -> &[&str] {
            &["index.ts", "index.tsx"]
        }
        fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            Path::new(path).file_stem()?.to_str()
        }
        fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            let stem = Path::new(path).file_stem()?.to_str()?;
            stem.strip_suffix(".spec")
                .or_else(|| stem.strip_suffix(".test"))
        }
        fn is_non_sut_helper(&self, _file_path: &str, _is_known_production: bool) -> bool {
            false
        }
    }

    /// Configurable MockExtractor for CORE-CIM tests.
    struct ConfigurableMockExtractor {
        barrel_file_names: Vec<String>,
        helper_file_paths: Vec<String>,
    }

    impl ConfigurableMockExtractor {
        fn new() -> Self {
            Self {
                barrel_file_names: vec!["index.ts".to_string()],
                helper_file_paths: vec![],
            }
        }

        fn with_helpers(helper_paths: Vec<String>) -> Self {
            Self {
                barrel_file_names: vec!["index.ts".to_string()],
                helper_file_paths: helper_paths,
            }
        }
    }

    impl ObserveExtractor for ConfigurableMockExtractor {
        fn extract_production_functions(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<ProductionFunction> {
            vec![]
        }
        fn extract_imports(&self, _source: &str, _file_path: &str) -> Vec<ImportMapping> {
            vec![]
        }
        fn extract_all_import_specifiers(&self, _source: &str) -> Vec<(String, Vec<String>)> {
            vec![]
        }
        fn extract_barrel_re_exports(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<BarrelReExport> {
            // Returns empty to avoid real fs access; barrel resolution tested separately
            vec![]
        }
        fn source_extensions(&self) -> &[&str] {
            &["ts", "tsx"]
        }
        fn index_file_names(&self) -> &[&str] {
            // Return a static slice matching our barrel file names
            &["index.ts"]
        }
        fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            Path::new(path).file_stem()?.to_str()
        }
        fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            let stem = Path::new(path).file_stem()?.to_str()?;
            stem.strip_suffix(".spec")
                .or_else(|| stem.strip_suffix(".test"))
        }
        fn is_non_sut_helper(&self, file_path: &str, _is_known_production: bool) -> bool {
            self.helper_file_paths.iter().any(|h| h == file_path)
        }
    }

    // TC-01: map_test_files で Layer 1 stem matching が動作
    #[test]
    fn tc01_map_test_files_stem_matching() {
        let mock = MockExtractor;
        let production = vec!["src/user.service.ts".to_string()];
        let tests = vec!["src/user.service.spec.ts".to_string()];
        let result = map_test_files(&mock, &production, &tests);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].production_file, "src/user.service.ts");
        assert_eq!(result[0].test_files, vec!["src/user.service.spec.ts"]);
        assert_eq!(result[0].strategy, MappingStrategy::FileNameConvention);
    }

    // TC-01b: map_test_files でマッチしない場合は空
    #[test]
    fn tc01b_map_test_files_no_match() {
        let mock = MockExtractor;
        let production = vec!["src/user.service.ts".to_string()];
        let tests = vec!["src/order.service.spec.ts".to_string()];
        let result = map_test_files(&mock, &production, &tests);
        assert_eq!(result.len(), 1);
        assert!(result[0].test_files.is_empty());
    }

    // TC-03: is_barrel_file が index_file_names で判定
    #[test]
    fn tc03_is_barrel_file_default_impl() {
        let mock = MockExtractor;
        assert!(mock.is_barrel_file("src/index.ts"));
        assert!(mock.is_barrel_file("src/index.tsx"));
        assert!(!mock.is_barrel_file("src/user.service.ts"));
        assert!(!mock.is_barrel_file("src/index.rs")); // not in mock's index_file_names
    }

    // TC-06: Send + Sync bound
    #[test]
    fn tc06_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockExtractor>();
        // Box<dyn ObserveExtractor> should also work
        let _: Box<dyn ObserveExtractor + Send + Sync> = Box::new(MockExtractor);
    }

    // CORE-CIM-01: barrel file 経由の production match
    //
    // Given: is_barrel_file returns true (path ends in index.ts), but resolve_barrel_exports
    //        returns no files (in-memory extractor avoids real fs access). The barrel path itself
    //        is also present in canonical_to_idx as a fallback production entry.
    //        file_exports_any_symbol returns true (default).
    // When:  collect_import_matches is called with the barrel path
    // Then:  barrel self-match fix: barrel path itself is added when file_exports_any_symbol=true,
    //        NOT via the else-if direct-match branch, but via the barrel branch self-match check.
    #[test]
    fn core_cim_01_barrel_file_skips_direct_match_branch() {
        // Given
        let ext = ConfigurableMockExtractor::new();
        let barrel_path = "/project/src/index.ts";
        let symbols: Vec<String> = vec!["UserService".to_string()];
        let canonical_root = Path::new("/project/src");

        // canonical_to_idx contains the barrel path itself
        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(barrel_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When: barrel file — resolve_barrel_exports returns empty (no real fs).
        //       file_exports_any_symbol returns true (ConfigurableMockExtractor default).
        collect_import_matches(
            &ext,
            barrel_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then: barrel branch was taken (not the else-if direct-match branch).
        //       Because file_exports_any_symbol=true, barrel self-match adds index 0.
        assert!(
            indices.contains(&0),
            "barrel path (file_exports_any_symbol=true) must be added via barrel \
             self-match check, not via direct-match branch. Got indices: {:?}",
            indices
        );
    }

    // CORE-CIM-02: 非 barrel file の直接 match
    //
    // Given: is_barrel_file returns false, is_non_sut_helper returns false,
    //        production file exists in canonical_to_idx at index 0
    // When: collect_import_matches is called with the production file path
    // Then: index 0 is inserted into indices
    #[test]
    fn core_cim_02_non_barrel_direct_match() {
        // Given
        let ext = ConfigurableMockExtractor::new();
        let prod_path = "/project/src/user.service.ts";
        let symbols: Vec<String> = vec!["UserService".to_string()];
        let canonical_root = Path::new("/project/src");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(prod_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            prod_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then
        assert!(
            indices.contains(&0),
            "production file index must be inserted for non-barrel direct match"
        );
        assert_eq!(indices.len(), 1);
    }

    // CORE-CIM-03: helper file はスキップ
    //
    // Given: is_non_sut_helper returns true for the resolved path
    // When: collect_import_matches is called
    // Then: indices stays empty
    #[test]
    fn core_cim_03_helper_file_skipped() {
        // Given
        let helper_path = "/project/src/test-utils.ts";
        let ext = ConfigurableMockExtractor::with_helpers(vec![helper_path.to_string()]);
        let symbols: Vec<String> = vec![];
        let canonical_root = Path::new("/project/src");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(helper_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            helper_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then
        assert!(
            indices.is_empty(),
            "helper files must be skipped and not added to indices"
        );
    }

    // ---------------------------------------------------------------------------
    // Barrel self-match tests (TC-01 through TC-06 from cycle 20260325_2303)
    // ---------------------------------------------------------------------------

    /// MockExtractor that treats `mod.rs` as a barrel file and exposes
    /// `file_exports_any_symbol` as a configurable flag.
    struct BarrelSelfMatchMock {
        exports_symbol: bool,
    }

    impl ObserveExtractor for BarrelSelfMatchMock {
        fn extract_production_functions(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<ProductionFunction> {
            vec![]
        }
        fn extract_imports(&self, _source: &str, _file_path: &str) -> Vec<ImportMapping> {
            vec![]
        }
        fn extract_all_import_specifiers(&self, _source: &str) -> Vec<(String, Vec<String>)> {
            vec![]
        }
        fn extract_barrel_re_exports(
            &self,
            _source: &str,
            _file_path: &str,
        ) -> Vec<BarrelReExport> {
            vec![]
        }
        fn source_extensions(&self) -> &[&'static str] {
            &["rs"]
        }
        fn index_file_names(&self) -> &[&'static str] {
            &["mod.rs"]
        }
        fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            Path::new(path).file_stem()?.to_str()
        }
        fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
            let stem = Path::new(path).file_stem()?.to_str()?;
            stem.strip_suffix("_test")
        }
        fn is_non_sut_helper(&self, _file_path: &str, _is_known_production: bool) -> bool {
            false
        }
        fn file_exports_any_symbol(&self, _path: &Path, _symbols: &[String]) -> bool {
            self.exports_symbol
        }
    }

    // TC-01: barrel file (mod.rs) がシンボルを直接定義する場合、mod.rs 自体が candidate に含まれること
    //
    // Given: mod.rs は barrel file (is_barrel_file=true)
    //        file_exports_any_symbol=true (mod.rs に `pub struct Foo` が直接定義されている)
    //        canonical_to_idx に mod.rs が index 0 で登録されている
    // When:  test imports `use crate::module::Foo` -> collect_import_matches(mod.rs, ["Foo"], ...)
    // Then:  index 0 が indices に追加される (mod.rs が production candidate)
    //
    // RED state: collect_import_matches の barrel 分岐が mod.rs 自体を除外するため FAIL する
    #[test]
    fn tc01_barrel_self_match_when_exports_symbol_directly() {
        // Given
        let ext = BarrelSelfMatchMock {
            exports_symbol: true,
        };
        let barrel_path = "/project/src/filter/mod.rs";
        let symbols: Vec<String> = vec!["Foo".to_string()];
        let canonical_root = Path::new("/project/src");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(barrel_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            barrel_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then: mod.rs 自体が candidate に含まれる
        assert!(
            indices.contains(&0),
            "barrel file (mod.rs) that directly defines the imported symbol \
             must be added as a production candidate (index 0). \
             Got indices: {:?}",
            indices
        );
    }

    // TC-02: barrel file (mod.rs) がシンボルを直接定義しない場合、mod.rs 自体は candidate に含まれないこと
    //
    // Given: mod.rs は barrel file (is_barrel_file=true)
    //        file_exports_any_symbol=false (mod.rs にシンボル定義なし、子ファイルのみ re-export)
    //        canonical_to_idx に mod.rs が index 0 で登録されている
    // When:  collect_import_matches(mod.rs, ["Foo"], ...)
    // Then:  indices は空のまま (mod.rs は candidate に含まれない)
    //
    // regression guard: 修正後もこの動作が維持されること
    #[test]
    fn tc02_barrel_no_self_match_when_symbol_not_defined_directly() {
        // Given
        let ext = BarrelSelfMatchMock {
            exports_symbol: false,
        };
        let barrel_path = "/project/src/filter/mod.rs";
        let symbols: Vec<String> = vec!["Foo".to_string()];
        let canonical_root = Path::new("/project/src");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(barrel_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            barrel_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then: mod.rs は candidate に含まれない
        assert!(
            indices.is_empty(),
            "barrel file (mod.rs) that does NOT directly define the symbol \
             must NOT be added as a production candidate. \
             Got indices: {:?}",
            indices
        );
    }

    // TC-03: TS index.ts barrel — exports_symbol=true の場合 index.ts 自体が candidate に含まれること
    //
    // Given: index.ts は barrel file (is_barrel_file=true)
    //        file_exports_any_symbol=true (デフォルト: ConfigurableMockExtractor)
    //        canonical_to_idx に index.ts が index 0 で登録されている
    //        resolve_barrel_exports は空を返す (fs アクセスなし)
    // When:  collect_import_matches(index.ts, ["UserService"], ...)
    // Then:  修正後は index.ts が candidate に含まれること (TS barrel self-match)
    //
    // RED state: 修正前は FAIL する (TC-01 と同じ根本原因)
    #[test]
    fn tc03_ts_index_barrel_self_match_regression_guard() {
        // Given: ConfigurableMockExtractor (index_file_names=["index.ts"], file_exports_any_symbol=true by default)
        let ext = ConfigurableMockExtractor::new();
        let barrel_path = "/project/src/services/index.ts";
        let symbols: Vec<String> = vec!["UserService".to_string()];
        let canonical_root = Path::new("/project/src");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(barrel_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            barrel_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then: index.ts が直接シンボルを定義している場合、candidate に含まれること
        assert!(
            indices.contains(&0),
            "TS index.ts barrel (file_exports_any_symbol=true) \
             must be added as a production candidate after barrel self-match fix. \
             Got indices: {:?}",
            indices
        );
    }

    // TC-04: Python __init__.py barrel — exports_symbol=true の場合 __init__.py 自体が candidate に含まれること
    //
    // Given: __init__.py は barrel file (is_barrel_file=true)
    //        file_exports_any_symbol=true (デフォルト)
    //        canonical_to_idx に __init__.py が index 0 で登録されている
    // When:  collect_import_matches(__init__.py, ["Foo"], ...)
    // Then:  修正後は __init__.py が candidate に含まれること
    //
    // RED state: 修正前は FAIL する (TC-01/TC-03 と同じ根本原因)
    #[test]
    fn tc04_python_init_barrel_self_match_regression_guard() {
        struct PythonBarrelMock;

        impl ObserveExtractor for PythonBarrelMock {
            fn extract_production_functions(
                &self,
                _source: &str,
                _file_path: &str,
            ) -> Vec<ProductionFunction> {
                vec![]
            }
            fn extract_imports(&self, _source: &str, _file_path: &str) -> Vec<ImportMapping> {
                vec![]
            }
            fn extract_all_import_specifiers(&self, _source: &str) -> Vec<(String, Vec<String>)> {
                vec![]
            }
            fn extract_barrel_re_exports(
                &self,
                _source: &str,
                _file_path: &str,
            ) -> Vec<BarrelReExport> {
                vec![]
            }
            fn source_extensions(&self) -> &[&'static str] {
                &["py"]
            }
            fn index_file_names(&self) -> &[&'static str] {
                &["__init__.py"]
            }
            fn production_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
                Path::new(path).file_stem()?.to_str()
            }
            fn test_stem<'a>(&self, path: &'a str) -> Option<&'a str> {
                let stem = Path::new(path).file_stem()?.to_str()?;
                stem.strip_prefix("test_")
            }
            fn is_non_sut_helper(&self, _file_path: &str, _is_known_production: bool) -> bool {
                false
            }
            // file_exports_any_symbol: default = true
        }

        // Given
        let ext = PythonBarrelMock;
        let barrel_path = "/project/pkg/__init__.py";
        let symbols: Vec<String> = vec!["Foo".to_string()];
        let canonical_root = Path::new("/project/pkg");

        let mut canonical_to_idx: HashMap<String, usize> = HashMap::new();
        canonical_to_idx.insert(barrel_path.to_string(), 0);
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            barrel_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then: __init__.py が直接シンボルを定義している場合、candidate に含まれること
        assert!(
            indices.contains(&0),
            "Python __init__.py barrel (file_exports_any_symbol=true) \
             must be added as a production candidate after barrel self-match fix. \
             Got indices: {:?}",
            indices
        );
    }

    // TC-05: tower observe R > 78.3% after barrel self-match fix (integration test)
    //
    // Given: tower crate at /tmp/exspec-dogfood/tower
    // When:  observe runs with Rust extractor after fix
    // Then:  recall > 78.3% (improvement from mod.rs barrel FN being resolved)
    #[test]
    #[ignore = "requires /tmp/exspec-dogfood/tower to be present"]
    fn tc05_tower_observe_recall_improves_after_barrel_self_match() {
        // Evidence collection:
        //   cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tower
        // Expected: mapped test files / total test files > 0.783
        panic!(
            "TC-05: integration test. \
             Run: cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tower \
             and verify recall > 78.3%"
        );
    }

    // TC-06: tokio observe R >= 50.8% after fix (no regression)
    //
    // Given: tokio crate at /tmp/exspec-dogfood/tokio
    // When:  observe runs with Rust extractor after fix
    // Then:  recall >= 50.8% (no regression from baseline)
    #[test]
    #[ignore = "requires /tmp/exspec-dogfood/tokio to be present"]
    fn tc06_tokio_observe_no_regression_after_barrel_self_match() {
        // Evidence collection:
        //   cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio
        // Expected: mapped test files / total test files >= 0.508
        panic!(
            "TC-06: integration test. \
             Run: cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio \
             and verify recall >= 50.8%"
        );
    }

    // CORE-CIM-04: canonical_to_idx に存在しない file はスキップ
    //
    // Given: canonical_to_idx is empty
    // When: collect_import_matches is called with any non-barrel, non-helper path
    // Then: indices stays empty
    #[test]
    fn core_cim_04_unknown_file_skipped() {
        // Given
        let ext = ConfigurableMockExtractor::new();
        let unknown_path = "/project/src/unknown.service.ts";
        let symbols: Vec<String> = vec![];
        let canonical_root = Path::new("/project/src");

        let canonical_to_idx: HashMap<String, usize> = HashMap::new(); // empty
        let mut indices: HashSet<usize> = HashSet::new();

        // When
        collect_import_matches(
            &ext,
            unknown_path,
            &symbols,
            &canonical_to_idx,
            &mut indices,
            canonical_root,
        );

        // Then
        assert!(
            indices.is_empty(),
            "file not in canonical_to_idx must be skipped"
        );
    }
}
