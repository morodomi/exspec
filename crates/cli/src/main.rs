use std::collections::HashMap;
use std::process;

use clap::Parser;
use exspec_core::config::ExspecConfig;
use exspec_core::extractor::{FileAnalysis, LanguageExtractor};
use exspec_core::metrics::compute_metrics;
use exspec_core::output::{compute_exit_code, format_json, format_sarif, format_terminal};
use exspec_core::rules::{evaluate_file_rules, evaluate_project_rules, evaluate_rules, Config};
use exspec_lang_php::PhpExtractor;
use exspec_lang_python::PythonExtractor;
use exspec_lang_rust::RustExtractor;
use exspec_lang_typescript::TypeScriptExtractor;
use ignore::WalkBuilder;

#[derive(Parser, Debug)]
#[command(name = "exspec", version, about = "Executable Specification Analyzer")]
pub struct Cli {
    /// Path to analyze
    #[arg(default_value = ".")]
    pub path: String,

    /// Output format
    #[arg(long, default_value = "terminal")]
    pub format: String,

    /// Language filter (python, typescript, php, rust)
    #[arg(long)]
    pub lang: Option<String>,

    /// Treat WARN as errors (exit 1)
    #[arg(long)]
    pub strict: bool,

    /// Path to config file
    #[arg(long, default_value = ".exspec.toml")]
    pub config: String,
}

fn is_python_test_file(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    (filename.starts_with("test_") || filename.ends_with("_test.py")) && filename.ends_with(".py")
}

fn is_typescript_test_file(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    filename.ends_with(".test.ts")
        || filename.ends_with(".test.tsx")
        || filename.ends_with(".spec.ts")
        || filename.ends_with(".spec.tsx")
}

fn is_python_source_file(path: &str) -> bool {
    path.ends_with(".py")
}

fn is_typescript_source_file(path: &str) -> bool {
    path.ends_with(".ts") || path.ends_with(".tsx")
}

fn is_php_test_file(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    filename.ends_with(".php")
        && (filename.ends_with("Test.php") || filename.ends_with("_test.php"))
}

fn is_php_source_file(path: &str) -> bool {
    path.ends_with(".php")
}

fn is_rust_test_file(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if !filename.ends_with(".rs") {
        return false;
    }
    // tests/**/*.rs or *_test.rs patterns
    filename.ends_with("_test.rs") || path.contains("/tests/") || path.contains("\\tests\\")
}

fn is_rust_source_file(path: &str) -> bool {
    path.ends_with(".rs")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Language {
    Python,
    TypeScript,
    Php,
    Rust,
}

struct DiscoverResult {
    test_files: HashMap<Language, Vec<String>>,
    source_file_count: usize,
}

fn discover_files(root: &str, lang: Option<&str>) -> DiscoverResult {
    let mut test_files: HashMap<Language, Vec<String>> = HashMap::new();
    let mut source_count = 0;
    let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();

    let include_python = lang.is_none() || lang == Some("python");
    let include_ts = lang.is_none() || lang == Some("typescript");
    let include_php = lang.is_none() || lang == Some("php");
    let include_rust = lang.is_none() || lang == Some("rust");

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path().to_string_lossy().to_string();

        let detected_test = if include_python && is_python_test_file(&path) {
            Some(Language::Python)
        } else if include_ts && is_typescript_test_file(&path) {
            Some(Language::TypeScript)
        } else if include_php && is_php_test_file(&path) {
            Some(Language::Php)
        } else if include_rust && is_rust_test_file(&path) {
            Some(Language::Rust)
        } else {
            None
        };

        if let Some(lang_key) = detected_test {
            test_files.entry(lang_key).or_default().push(path);
        } else {
            let is_source = (include_python && is_python_source_file(&path))
                || (include_ts && is_typescript_source_file(&path))
                || (include_php && is_php_source_file(&path))
                || (include_rust && is_rust_source_file(&path));
            if is_source {
                source_count += 1;
            }
        }
    }

    for files in test_files.values_mut() {
        files.sort();
    }

    DiscoverResult {
        test_files,
        source_file_count: source_count,
    }
}

fn load_config(config_path: &str) -> Config {
    match std::fs::read_to_string(config_path) {
        Ok(content) => match ExspecConfig::from_toml(&content) {
            Ok(ec) => ec.into(),
            Err(e) => {
                eprintln!("warning: invalid config {config_path}: {e}");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

const SUPPORTED_LANGUAGES: &[&str] = &["python", "typescript", "php", "rust"];
const SUPPORTED_FORMATS: &[&str] = &["terminal", "json", "sarif"];

fn validate_format(format: &str) -> Result<(), String> {
    if !SUPPORTED_FORMATS.contains(&format) {
        return Err(format!(
            "unsupported format: {format}. Supported: {}",
            SUPPORTED_FORMATS.join(", ")
        ));
    }
    Ok(())
}

fn validate_lang(lang: Option<&str>) -> Result<(), String> {
    if let Some(l) = lang {
        if !SUPPORTED_LANGUAGES.contains(&l) {
            return Err(format!(
                "unsupported language: {l}. Supported: {}",
                SUPPORTED_LANGUAGES.join(", ")
            ));
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = validate_lang(cli.lang.as_deref()) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    if let Err(e) = validate_format(&cli.format) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    let config = load_config(&cli.config);
    let py_extractor = PythonExtractor::new();
    let ts_extractor = TypeScriptExtractor::new();
    let php_extractor = PhpExtractor::new();
    let rust_extractor = RustExtractor::new();

    let discovered = discover_files(&cli.path, cli.lang.as_deref());
    let test_file_count: usize = discovered.test_files.values().map(|v| v.len()).sum();
    let mut all_analyses: Vec<FileAnalysis> = Vec::new();

    let extractors: &[(Language, &dyn LanguageExtractor)] = &[
        (Language::Python, &py_extractor),
        (Language::TypeScript, &ts_extractor),
        (Language::Php, &php_extractor),
        (Language::Rust, &rust_extractor),
    ];

    for (lang_key, extractor) in extractors {
        if let Some(files) = discovered.test_files.get(lang_key) {
            for file_path in files {
                let source = match std::fs::read_to_string(file_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("warning: cannot read {file_path}: {e}");
                        continue;
                    }
                };
                all_analyses.push(extractor.extract_file_analysis(&source, file_path));
            }
        }
    }

    // Collect all functions for per-function rules (T001-T003)
    let all_functions: Vec<_> = all_analyses
        .iter()
        .flat_map(|a| a.functions.iter())
        .cloned()
        .collect();

    // Per-function rules (T001-T003)
    let mut diagnostics = evaluate_rules(&all_functions, &config);

    // Per-file rules (T004-T006, T008)
    diagnostics.extend(evaluate_file_rules(&all_analyses, &config));

    // Per-project rules (T007)
    let source_file_count = discovered.source_file_count;
    diagnostics.extend(evaluate_project_rules(
        test_file_count,
        source_file_count,
        &config,
    ));

    let metrics = compute_metrics(&all_analyses, source_file_count);

    let output = match cli.format.as_str() {
        "json" => format_json(&diagnostics, test_file_count, all_functions.len(), &metrics),
        "sarif" => format_sarif(&diagnostics),
        _ => format_terminal(&diagnostics, test_file_count, all_functions.len(), &metrics),
    };

    if !output.is_empty() {
        println!("{output}");
    }

    let exit_code = compute_exit_code(&diagnostics, cli.strict);
    process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_path_argument() {
        let cli = Cli::try_parse_from(["exspec", "."]).unwrap();
        assert_eq!(cli.path, ".");
    }

    #[test]
    fn cli_default_path() {
        let cli = Cli::try_parse_from(["exspec"]).unwrap();
        assert_eq!(cli.path, ".");
    }

    #[test]
    fn cli_strict_flag() {
        let cli = Cli::try_parse_from(["exspec", "--strict", "src/"]).unwrap();
        assert!(cli.strict);
        assert_eq!(cli.path, "src/");
    }

    #[test]
    fn cli_format_option() {
        let cli = Cli::try_parse_from(["exspec", "--format", "json", "."]).unwrap();
        assert_eq!(cli.format, "json");
    }

    #[test]
    fn cli_lang_option() {
        let cli = Cli::try_parse_from(["exspec", "--lang", "python", "."]).unwrap();
        assert_eq!(cli.lang, Some("python".to_string()));
    }

    #[test]
    fn cli_help_does_not_panic() {
        let result = Cli::try_parse_from(["exspec", "--help"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_config_option() {
        let cli = Cli::try_parse_from(["exspec", "--config", "my.toml", "."]).unwrap();
        assert_eq!(cli.config, "my.toml");
    }

    #[test]
    fn cli_config_default() {
        let cli = Cli::try_parse_from(["exspec"]).unwrap();
        assert_eq!(cli.config, ".exspec.toml");
    }

    // --- Python file discovery ---

    #[test]
    fn is_python_test_file_matches_test_prefix() {
        assert!(is_python_test_file("tests/test_foo.py"));
        assert!(is_python_test_file("test_bar.py"));
    }

    #[test]
    fn is_python_test_file_matches_test_suffix() {
        assert!(is_python_test_file("foo_test.py"));
    }

    #[test]
    fn is_python_test_file_rejects_non_test() {
        assert!(!is_python_test_file("foo.py"));
        assert!(!is_python_test_file("helper.py"));
        assert!(!is_python_test_file("test_foo.js"));
    }

    // --- TypeScript file discovery ---

    #[test]
    fn is_typescript_test_file_matches_test_patterns() {
        assert!(is_typescript_test_file("foo.test.ts"));
        assert!(is_typescript_test_file("bar.spec.ts"));
        assert!(is_typescript_test_file("baz.test.tsx"));
        assert!(is_typescript_test_file("qux.spec.tsx"));
    }

    #[test]
    fn is_typescript_test_file_rejects_non_test() {
        assert!(!is_typescript_test_file("foo.ts"));
        assert!(!is_typescript_test_file("helper.ts"));
        assert!(!is_typescript_test_file("test.js"));
    }

    // --- PHP file discovery ---

    #[test]
    fn is_php_test_file_matches_test_suffix() {
        assert!(is_php_test_file("UserTest.php"));
        assert!(is_php_test_file("tests/UserTest.php"));
        assert!(is_php_test_file("user_test.php"));
    }

    #[test]
    fn is_php_test_file_rejects_non_test() {
        assert!(!is_php_test_file("User.php"));
        assert!(!is_php_test_file("helper.php"));
        assert!(!is_php_test_file("UserTest.py"));
    }

    // --- Source file detection ---

    #[test]
    fn is_python_source_file_detects_py() {
        assert!(is_python_source_file("foo.py"));
        assert!(!is_python_source_file("foo.ts"));
    }

    #[test]
    fn is_typescript_source_file_detects_ts_tsx() {
        assert!(is_typescript_source_file("foo.ts"));
        assert!(is_typescript_source_file("foo.tsx"));
        assert!(!is_typescript_source_file("foo.py"));
    }

    #[test]
    fn is_php_source_file_detects_php() {
        assert!(is_php_source_file("User.php"));
        assert!(!is_php_source_file("foo.py"));
    }

    // --- Multi-language discovery ---

    fn get_test_files(result: &DiscoverResult, lang: Language) -> &[String] {
        result.test_files.get(&lang).map_or(&[], |v| v.as_slice())
    }

    #[test]
    fn discover_test_files_finds_test_pattern() {
        let dir = std::env::temp_dir().join(format!("exspec_test_discover_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("bar_test.py"), "").unwrap();
        std::fs::write(dir.join("helper.py"), "").unwrap();
        std::fs::write(dir.join("baz.test.ts"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None);
        assert_eq!(get_test_files(&result, Language::Python).len(), 2);
        assert_eq!(get_test_files(&result, Language::TypeScript).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_files_lang_filter_python() {
        let dir = std::env::temp_dir().join(format!("exspec_test_lang_py_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("baz.test.ts"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), Some("python"));
        assert_eq!(get_test_files(&result, Language::Python).len(), 1);
        assert_eq!(get_test_files(&result, Language::TypeScript).len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_files_lang_filter_typescript() {
        let dir = std::env::temp_dir().join(format!("exspec_test_lang_ts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("baz.test.ts"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), Some("typescript"));
        assert_eq!(get_test_files(&result, Language::Python).len(), 0);
        assert_eq!(get_test_files(&result, Language::TypeScript).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_files_lang_filter_php() {
        let dir = std::env::temp_dir().join(format!("exspec_test_lang_php_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("UserTest.php"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), Some("php"));
        assert_eq!(get_test_files(&result, Language::Python).len(), 0);
        assert_eq!(get_test_files(&result, Language::Php).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_files_ignores_venv() {
        let result = discover_files(".", None);
        let py = get_test_files(&result, Language::Python);
        assert!(py.iter().all(|f| !f.contains(".venv")));
    }

    // --- Source file counting ---

    #[test]
    fn count_source_files_excludes_test_files() {
        let dir =
            std::env::temp_dir().join(format!("exspec_test_src_count_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("app.py"), "").unwrap();
        std::fs::write(dir.join("test_app.py"), "").unwrap();
        std::fs::write(dir.join("utils.ts"), "").unwrap();
        std::fs::write(dir.join("utils.test.ts"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None);
        assert_eq!(result.source_file_count, 2); // app.py + utils.ts (test files excluded)
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Combined walk ---

    #[test]
    fn discover_files_returns_test_and_source_counts() {
        let dir = std::env::temp_dir().join(format!("exspec_test_combined_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("app.py"), "").unwrap();
        std::fs::write(dir.join("baz.test.ts"), "").unwrap();
        std::fs::write(dir.join("utils.ts"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None);
        assert_eq!(get_test_files(&result, Language::Python).len(), 1);
        assert_eq!(get_test_files(&result, Language::TypeScript).len(), 1);
        assert_eq!(result.source_file_count, 2); // app.py + utils.ts
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Hidden directory skip ---

    #[test]
    fn discover_files_skips_hidden_directories() {
        let dir = std::env::temp_dir().join(format!("exspec_test_hidden_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join(".hidden/test_foo.py"), "").unwrap();
        std::fs::write(dir.join("test_visible.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(py.len(), 1, "should only find visible file: {py:?}");
        assert!(py[0].contains("test_visible.py"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- PHP file discovery in combined walk ---

    #[test]
    fn discover_files_finds_php_test_files() {
        let dir = std::env::temp_dir().join(format!("exspec_test_php_disc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("UserTest.php"), "").unwrap();
        std::fs::write(dir.join("User.php"), "").unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None);
        assert_eq!(get_test_files(&result, Language::Php).len(), 1);
        assert_eq!(result.source_file_count, 1); // User.php
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Config loading ---

    #[test]
    fn load_config_missing_file_returns_default() {
        let config = load_config("/nonexistent/.exspec.toml");
        let defaults = Config::default();
        assert_eq!(config.mock_max, defaults.mock_max);
    }

    #[test]
    fn load_config_valid_file() {
        let config_path = format!(
            "{}/tests/fixtures/config/valid.toml",
            env!("CARGO_MANIFEST_DIR").replace("/crates/cli", ""),
        );
        let config = load_config(&config_path);
        assert_eq!(config.mock_max, 10);
        assert_eq!(config.disabled_rules.len(), 2);
    }

    // --- E2E ---

    fn fixture_path(lang: &str, name: &str) -> String {
        format!(
            "{}/tests/fixtures/{}/{}",
            env!("CARGO_MANIFEST_DIR").replace("/crates/cli", ""),
            lang,
            name,
        )
    }

    fn analyze_python_fixtures(files: &[&str]) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = PythonExtractor::new();
        let config = Config::default();
        let mut all_functions = Vec::new();
        for name in files {
            let path = fixture_path("python", name);
            let source = std::fs::read_to_string(&path).unwrap();
            all_functions.extend(extractor.extract_test_functions(&source, &path));
        }
        evaluate_rules(&all_functions, &config)
    }

    fn analyze_ts_fixtures(files: &[&str]) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = TypeScriptExtractor::new();
        let config = Config::default();
        let mut all_functions = Vec::new();
        for name in files {
            let path = fixture_path("typescript", name);
            let source = std::fs::read_to_string(&path).unwrap();
            all_functions.extend(extractor.extract_test_functions(&source, &path));
        }
        evaluate_rules(&all_functions, &config)
    }

    fn analyze_python_file_rules(
        files: &[&str],
        config: &Config,
    ) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = PythonExtractor::new();
        let mut analyses = Vec::new();
        for name in files {
            let path = fixture_path("python", name);
            let source = std::fs::read_to_string(&path).unwrap();
            analyses.push(extractor.extract_file_analysis(&source, &path));
        }
        evaluate_file_rules(&analyses, config)
    }

    fn analyze_ts_file_rules(
        files: &[&str],
        config: &Config,
    ) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = TypeScriptExtractor::new();
        let mut analyses = Vec::new();
        for name in files {
            let path = fixture_path("typescript", name);
            let source = std::fs::read_to_string(&path).unwrap();
            analyses.push(extractor.extract_file_analysis(&source, &path));
        }
        evaluate_file_rules(&analyses, config)
    }

    fn analyze_php_fixtures(files: &[&str]) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = PhpExtractor::new();
        let config = Config::default();
        let mut all_functions = Vec::new();
        for name in files {
            let path = fixture_path("php", name);
            let source = std::fs::read_to_string(&path).unwrap();
            all_functions.extend(extractor.extract_test_functions(&source, &path));
        }
        evaluate_rules(&all_functions, &config)
    }

    fn analyze_php_file_rules(
        files: &[&str],
        config: &Config,
    ) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = PhpExtractor::new();
        let mut analyses = Vec::new();
        for name in files {
            let path = fixture_path("php", name);
            let source = std::fs::read_to_string(&path).unwrap();
            analyses.push(extractor.extract_file_analysis(&source, &path));
        }
        evaluate_file_rules(&analyses, config)
    }

    fn analyze_rust_fixtures(files: &[&str]) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = RustExtractor::new();
        let config = Config::default();
        let mut all_functions = Vec::new();
        for name in files {
            let path = fixture_path("rust", name);
            let source = std::fs::read_to_string(&path).unwrap();
            all_functions.extend(extractor.extract_test_functions(&source, &path));
        }
        evaluate_rules(&all_functions, &config)
    }

    fn analyze_rust_file_rules(
        files: &[&str],
        config: &Config,
    ) -> Vec<exspec_core::rules::Diagnostic> {
        let extractor = RustExtractor::new();
        let mut analyses = Vec::new();
        for name in files {
            let path = fixture_path("rust", name);
            let source = std::fs::read_to_string(&path).unwrap();
            analyses.push(extractor.extract_file_analysis(&source, &path));
        }
        evaluate_file_rules(&analyses, config)
    }

    // Python E2E (T001-T003)
    #[test]
    fn e2e_t001_violation_detected() {
        let diags = analyze_python_fixtures(&["t001_violation.py"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T001"));
    }

    #[test]
    fn e2e_t002_violation_detected() {
        let diags = analyze_python_fixtures(&["t002_violation.py"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T002"));
    }

    #[test]
    fn e2e_t003_violation_detected() {
        let diags = analyze_python_fixtures(&["t003_violation.py"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T003"));
    }

    #[test]
    fn e2e_pass_files_no_diagnostics() {
        let diags = analyze_python_fixtures(&["t001_pass.py", "t002_pass.py", "t003_pass.py"]);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // TypeScript E2E (T001-T003)
    #[test]
    fn e2e_ts_t001_violation_detected() {
        let diags = analyze_ts_fixtures(&["t001_violation.test.ts"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T001"));
    }

    #[test]
    fn e2e_ts_t002_violation_detected() {
        let diags = analyze_ts_fixtures(&["t002_violation.test.ts"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T002"));
    }

    #[test]
    fn e2e_ts_t003_violation_detected() {
        let diags = analyze_ts_fixtures(&["t003_violation.test.ts"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T003"));
    }

    #[test]
    fn e2e_ts_pass_files_no_diagnostics() {
        let diags = analyze_ts_fixtures(&[
            "t001_pass.test.ts",
            "t002_pass.test.ts",
            "t003_pass.test.ts",
        ]);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // Suppression E2E
    #[test]
    fn e2e_python_suppression_hides_t002() {
        let diags = analyze_python_fixtures(&["suppressed.py"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T002"),
            "T002 should be suppressed"
        );
    }

    #[test]
    fn e2e_ts_suppression_hides_t002() {
        let diags = analyze_ts_fixtures(&["suppressed.test.ts"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T002"),
            "T002 should be suppressed"
        );
    }

    // PHP E2E (T001-T003)
    #[test]
    fn e2e_php_t001_violation_detected() {
        let diags = analyze_php_fixtures(&["t001_violation.php"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T001"));
    }

    #[test]
    fn e2e_php_t002_violation_detected() {
        let diags = analyze_php_fixtures(&["t002_violation.php"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T002"));
    }

    #[test]
    fn e2e_php_t003_violation_detected() {
        let diags = analyze_php_fixtures(&["t003_violation.php"]);
        assert!(diags.iter().any(|d| d.rule.0 == "T003"));
    }

    #[test]
    fn e2e_php_pass_files_no_diagnostics() {
        let diags = analyze_php_fixtures(&["t001_pass.php", "t002_pass.php", "t003_pass.php"]);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }

    // PHP Suppression E2E
    #[test]
    fn e2e_php_suppression_hides_t002() {
        let diags = analyze_php_fixtures(&["suppressed.php"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T002"),
            "T002 should be suppressed"
        );
    }

    // PHP E2E: FQCN attribute and Pest arrow function
    #[test]
    fn e2e_php_fqcn_attribute_pass() {
        let extractor = PhpExtractor::new();
        let path = fixture_path("php", "t001_pass_fqcn_attribute.php");
        let source = std::fs::read_to_string(&path).unwrap();
        let funcs = extractor.extract_test_functions(&source, &path);
        assert_eq!(
            funcs.len(),
            1,
            "should detect 1 test function via FQCN attribute"
        );
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(
            diags.is_empty(),
            "FQCN attribute test should pass, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_pest_arrow_pass() {
        let extractor = PhpExtractor::new();
        let path = fixture_path("php", "t001_pass_pest_arrow.php");
        let source = std::fs::read_to_string(&path).unwrap();
        let funcs = extractor.extract_test_functions(&source, &path);
        assert_eq!(
            funcs.len(),
            1,
            "should detect 1 test function via Pest arrow"
        );
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(
            diags.is_empty(),
            "Pest arrow test should pass, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_pest_arrow_chained_pass() {
        let extractor = PhpExtractor::new();
        let path = fixture_path("php", "t001_pass_pest_arrow_chained.php");
        let source = std::fs::read_to_string(&path).unwrap();
        let funcs = extractor.extract_test_functions(&source, &path);
        assert_eq!(
            funcs.len(),
            1,
            "should detect 1 test function via Pest arrow chained"
        );
        let diags = evaluate_rules(&funcs, &Config::default());
        assert!(
            diags.is_empty(),
            "Pest arrow chained test should pass, got: {diags:?}"
        );
    }

    // PHP E2E: File-level rules (T004-T008)
    #[test]
    fn e2e_php_t004_violation_detected() {
        let diags = analyze_php_file_rules(&["t004_violation.php"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T004"),
            "expected T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t004_pass_no_t004() {
        let diags = analyze_php_file_rules(&["t004_pass.php"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T004"),
            "expected no T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t005_violation_detected() {
        // PHP PBT is not mature, so T005 always triggers
        let diags = analyze_php_file_rules(&["t005_violation.php"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T005"),
            "expected T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t006_violation_detected() {
        let diags = analyze_php_file_rules(&["t006_violation.php"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T006"),
            "expected T006, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t006_pass_no_t006() {
        let diags = analyze_php_file_rules(&["t006_pass.php"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T006"),
            "expected no T006, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t008_violation_detected() {
        let diags = analyze_php_file_rules(&["t008_violation.php"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T008"),
            "expected T008, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_php_t008_pass_no_t008() {
        let diags = analyze_php_file_rules(&["t008_pass.php"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T008"),
            "expected no T008, got: {diags:?}"
        );
    }

    // --- Rust E2E (TC-25, TC-26, TC-27) ---

    // TC-25: T001-T003 pass/violation
    #[test]
    fn e2e_rust_t001_violation_detected() {
        let diags = analyze_rust_fixtures(&["t001_violation.rs"]);
        assert!(
            diags.iter().any(|d| d.rule.0 == "T001"),
            "expected T001, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t001_pass_no_t001() {
        let diags = analyze_rust_fixtures(&["t001_pass.rs"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T001"),
            "expected no T001, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t002_violation_detected() {
        let diags = analyze_rust_fixtures(&["t002_violation.rs"]);
        assert!(
            diags.iter().any(|d| d.rule.0 == "T002"),
            "expected T002, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t002_pass_no_t002() {
        let diags = analyze_rust_fixtures(&["t002_pass.rs"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T002"),
            "expected no T002, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t003_violation_detected() {
        let diags = analyze_rust_fixtures(&["t003_violation.rs"]);
        assert!(
            diags.iter().any(|d| d.rule.0 == "T003"),
            "expected T003, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t003_pass_no_t003() {
        let diags = analyze_rust_fixtures(&["t003_pass.rs"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T003"),
            "expected no T003, got: {diags:?}"
        );
    }

    // TC-26: T004-T006, T008 pass/violation
    #[test]
    fn e2e_rust_t004_violation_detected() {
        let diags = analyze_rust_file_rules(&["t004_violation.rs"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T004"),
            "expected T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t004_pass_no_t004() {
        let diags = analyze_rust_file_rules(&["t004_pass.rs"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T004"),
            "expected no T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t005_violation_detected() {
        let diags = analyze_rust_file_rules(&["t005_violation.rs"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T005"),
            "expected T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t005_pass_no_t005() {
        let diags = analyze_rust_file_rules(&["t005_pass.rs"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T005"),
            "expected no T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_rust_t008_violation_detected() {
        let diags = analyze_rust_file_rules(&["t008_violation.rs"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T008"),
            "expected T008 (always INFO for Rust), got: {diags:?}"
        );
    }

    // TC-27: suppression E2E
    #[test]
    fn e2e_rust_suppression_hides_t001() {
        let diags = analyze_rust_fixtures(&["suppressed.rs"]);
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T001"),
            "T001 should be suppressed, got: {diags:?}"
        );
    }

    // TC-23: --lang rust で Rust のみ解析 (validate_lang covered by supported_lang_rust_ok)

    // --- E2E: File-level rules (T004-T008) ---

    // T004
    #[test]
    fn e2e_t004_violation_detected() {
        let diags = analyze_python_file_rules(&["t004_violation.py"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T004"),
            "expected T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_t004_pass_no_t004() {
        let diags = analyze_python_file_rules(&["t004_pass.py"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T004"),
            "expected no T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t004_violation_detected() {
        let diags = analyze_ts_file_rules(&["t004_violation.test.ts"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T004"),
            "expected T004, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t004_pass_no_t004() {
        let diags = analyze_ts_file_rules(&["t004_pass.test.ts"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T004"),
            "expected no T004, got: {diags:?}"
        );
    }

    // T005
    #[test]
    fn e2e_t005_violation_detected() {
        let diags = analyze_python_file_rules(&["t005_violation.py"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T005"),
            "expected T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_t005_pass_no_t005() {
        let diags = analyze_python_file_rules(&["t005_pass.py"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T005"),
            "expected no T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t005_violation_detected() {
        let diags = analyze_ts_file_rules(&["t005_violation.test.ts"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T005"),
            "expected T005, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t005_pass_no_t005() {
        let diags = analyze_ts_file_rules(&["t005_pass.test.ts"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T005"),
            "expected no T005, got: {diags:?}"
        );
    }

    // T006
    #[test]
    fn e2e_t006_violation_detected() {
        let diags = analyze_python_file_rules(&["t006_violation.py"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T006"),
            "expected T006, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_t006_pass_no_t006() {
        let diags = analyze_python_file_rules(&["t006_pass.py"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T006"),
            "expected no T006, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t006_violation_detected() {
        let diags = analyze_ts_file_rules(&["t006_violation.test.ts"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T006"),
            "expected T006, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t006_pass_no_t006() {
        let diags = analyze_ts_file_rules(&["t006_pass.test.ts"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T006"),
            "expected no T006, got: {diags:?}"
        );
    }

    // T008
    #[test]
    fn e2e_t008_violation_detected() {
        let diags = analyze_python_file_rules(&["t008_violation.py"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T008"),
            "expected T008, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_t008_pass_no_t008() {
        let diags = analyze_python_file_rules(&["t008_pass.py"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T008"),
            "expected no T008, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t008_violation_detected() {
        let diags = analyze_ts_file_rules(&["t008_violation.test.ts"], &Config::default());
        assert!(
            diags.iter().any(|d| d.rule.0 == "T008"),
            "expected T008, got: {diags:?}"
        );
    }

    #[test]
    fn e2e_ts_t008_pass_no_t008() {
        let diags = analyze_ts_file_rules(&["t008_pass.test.ts"], &Config::default());
        assert!(
            !diags.iter().any(|d| d.rule.0 == "T008"),
            "expected no T008, got: {diags:?}"
        );
    }

    // --- E2E: Config disable + file-level rules ---

    #[test]
    fn e2e_config_disables_t004() {
        let config = Config {
            disabled_rules: vec![exspec_core::rules::RuleId::new("T004")],
            ..Config::default()
        };
        let diags = analyze_python_file_rules(&["t004_violation.py"], &config);
        assert!(!diags.iter().any(|d| d.rule.0 == "T004"));
    }

    #[test]
    fn e2e_config_disables_t005() {
        let config = Config {
            disabled_rules: vec![exspec_core::rules::RuleId::new("T005")],
            ..Config::default()
        };
        let diags = analyze_python_file_rules(&["t005_violation.py"], &config);
        assert!(!diags.iter().any(|d| d.rule.0 == "T005"));
    }

    // --- E2E: T007 project-level ---

    // --- --lang validation ---

    #[test]
    fn unsupported_lang_returns_error_message() {
        let result = validate_lang(Some("cobol"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cobol"));
    }

    #[test]
    fn unsupported_lang_error_shows_supported_list() {
        let err = validate_lang(Some("Python")).unwrap_err();
        assert!(err.contains("python"), "should hint lowercase: {err}");
        assert!(
            err.contains("typescript"),
            "should list all supported: {err}"
        );
    }

    #[test]
    fn supported_lang_python_ok() {
        assert!(validate_lang(Some("python")).is_ok());
    }

    #[test]
    fn supported_lang_typescript_ok() {
        assert!(validate_lang(Some("typescript")).is_ok());
    }

    #[test]
    fn supported_lang_php_ok() {
        assert!(validate_lang(Some("php")).is_ok());
    }

    #[test]
    fn supported_lang_rust_ok() {
        // TC-24: SUPPORTED_LANGUAGES contains "rust"
        assert!(validate_lang(Some("rust")).is_ok());
    }

    #[test]
    fn no_lang_ok() {
        assert!(validate_lang(None).is_ok());
    }

    // --- Rust file discovery (TC-20, TC-21, TC-22) ---

    #[test]
    fn is_rust_test_file_matches_tests_dir() {
        // TC-20: tests/**/*.rs is a Rust test file
        assert!(is_rust_test_file("tests/integration/foo_test.rs"));
        assert!(is_rust_test_file("/project/tests/test_something.rs"));
    }

    #[test]
    fn is_rust_test_file_matches_test_suffix() {
        // TC-21: *_test.rs is a Rust test file
        assert!(is_rust_test_file("foo_test.rs"));
        assert!(is_rust_test_file("user_service_test.rs"));
    }

    #[test]
    fn is_rust_test_file_rejects_src_files() {
        // TC-22: src/*.rs is NOT a test file
        assert!(!is_rust_test_file("src/lib.rs"));
        assert!(!is_rust_test_file("src/main.rs"));
        assert!(!is_rust_test_file("crates/core/src/rules.rs"));
    }

    #[test]
    fn is_rust_test_file_rejects_non_rs() {
        assert!(!is_rust_test_file("test_foo.py"));
        assert!(!is_rust_test_file("foo.test.ts"));
    }

    #[test]
    fn is_rust_source_file_detects_rs() {
        assert!(is_rust_source_file("src/lib.rs"));
        assert!(is_rust_source_file("src/main.rs"));
        assert!(!is_rust_source_file("foo.py"));
        assert!(!is_rust_source_file("foo.ts"));
    }

    // --- Rust discover_files filter (TC-28) ---

    #[test]
    fn discover_files_finds_rust_test_files() {
        // TC-28: discover_files with rust filter
        // tests/ directory files are test files; src/ files are source files
        let dir =
            std::env::temp_dir().join(format!("exspec_test_rust_disc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("tests/foo_test.rs"), "").unwrap();
        std::fs::write(dir.join("tests/integration.rs"), "").unwrap(); // also in tests/ -> test file
        std::fs::write(dir.join("src/lib.rs"), "").unwrap(); // src/ -> source file
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), Some("rust"));
        assert_eq!(
            get_test_files(&result, Language::Rust).len(),
            2,
            "should find 2 rust test files (tests/ dir)"
        );
        assert_eq!(
            get_test_files(&result, Language::Python).len(),
            0,
            "rust-only filter should exclude Python files"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn e2e_t007_produces_ratio() {
        let diags = evaluate_project_rules(5, 10, &Config::default());
        assert!(diags.iter().any(|d| d.rule.0 == "T007"));
        assert!(diags[0].message.contains("5/10"));
    }

    // --- --format validation ---

    #[test]
    fn validate_format_terminal_ok() {
        assert!(validate_format("terminal").is_ok());
    }

    #[test]
    fn validate_format_json_ok() {
        assert!(validate_format("json").is_ok());
    }

    #[test]
    fn validate_format_sarif_ok() {
        assert!(validate_format("sarif").is_ok());
    }

    #[test]
    fn validate_format_unknown_error() {
        let err = validate_format("xml").unwrap_err();
        assert!(err.contains("xml"), "should mention invalid format: {err}");
        assert!(err.contains("terminal"), "should list supported: {err}");
        assert!(err.contains("json"), "should list supported: {err}");
        assert!(err.contains("sarif"), "should list supported: {err}");
    }

    #[test]
    fn validate_format_ai_prompt_error() {
        let err = validate_format("ai-prompt").unwrap_err();
        assert!(err.contains("ai-prompt"));
    }

    // --- E2E: SARIF output ---

    #[test]
    fn e2e_sarif_output_valid_json() {
        let output = exspec_core::output::format_sarif(&[]);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["version"], "2.1.0");
    }

    // --- E2E: JSON metrics ---

    #[test]
    fn e2e_json_metrics_has_values() {
        let extractor = PythonExtractor::new();
        let path = fixture_path("python", "t001_pass.py");
        let source = std::fs::read_to_string(&path).unwrap();
        let analyses = vec![extractor.extract_file_analysis(&source, &path)];
        let metrics = exspec_core::metrics::compute_metrics(&analyses, 1);
        let diags = evaluate_rules(&analyses[0].functions, &Config::default());
        let output =
            exspec_core::output::format_json(&diags, 1, analyses[0].functions.len(), &metrics);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["metrics"].is_object());
        assert!(parsed["metrics"]["assertion_density_avg"].is_number());
    }
}
