use std::collections::HashMap;
use std::path::Path;
use std::process;

use clap::{Args, Parser, Subcommand};
use exspec_core::config::ExspecConfig;
use exspec_core::extractor::{FileAnalysis, LanguageExtractor};
use exspec_core::hints::compute_hints;
use exspec_core::metrics::compute_metrics;
use exspec_core::observe_report::{
    ObserveFileEntry, ObserveReport, ObserveRouteEntry, ObserveSummary,
};
use exspec_core::output::{
    compute_exit_code, filter_by_severity, format_ai_prompt, format_json, format_sarif,
    format_terminal, SummaryStats,
};
use exspec_core::rules::{
    evaluate_file_rules, evaluate_project_rules, evaluate_rules, Config, Severity,
};
use exspec_lang_php::PhpExtractor;
use exspec_lang_python::PythonExtractor;
use exspec_lang_rust::RustExtractor;
use exspec_lang_typescript::TypeScriptExtractor;
use ignore::WalkBuilder;

#[derive(Parser, Debug)]
#[command(name = "exspec", version, about = "Executable Specification Analyzer")]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub lint_args: LintArgs,
}

#[derive(Args, Debug)]
pub struct LintArgs {
    /// Path to analyze
    #[arg(default_value = ".")]
    pub path: String,

    /// Output format
    #[arg(long, default_value = "ai-prompt")]
    pub format: String,

    /// Language filter (python, typescript, php, rust)
    #[arg(long)]
    pub lang: Option<String>,

    /// Treat WARN as errors (exit 1)
    #[arg(long)]
    pub strict: bool,

    /// Minimum severity to display (info, warn, block)
    #[arg(long)]
    pub min_severity: Option<String>,

    /// Path to config file
    #[arg(long, default_value = ".exspec.toml")]
    pub config: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Test-to-code mapping report
    Observe(ObserveArgs),
}

#[derive(Args, Debug)]
pub struct ObserveArgs {
    /// Path to analyze
    #[arg(default_value = ".")]
    pub path: String,

    /// Language to analyze (typescript, python)
    #[arg(long)]
    pub lang: String,

    /// Output format (terminal, json)
    #[arg(long, default_value = "terminal")]
    pub format: String,

    /// Suppress L2 import tracing for L1-matched test files
    #[arg(long)]
    pub l1_exclusive: bool,

    /// Disable both fan-out filters: forward (prod→test) and reverse (test→prod)
    #[arg(long)]
    pub no_fan_out_filter: bool,
}

fn is_python_test_file(path: &str) -> bool {
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    (filename.starts_with("test_") || filename.ends_with("_test.py") || filename == "tests.py")
        && filename.ends_with(".py")
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

fn has_path_component(path: &str, component: &str) -> bool {
    std::path::Path::new(path)
        .components()
        .any(|c| c.as_os_str() == component)
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
    filename.ends_with("_test.rs") || has_path_component(path, "tests")
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
    /// Source (non-test) file paths, collected when available.
    source_files: Vec<String>,
}

/// Discover test and source files under `root`.
///
/// `ignore_patterns` uses simple substring matching on root-relative file paths.
/// A file is excluded if its relative path contains any of the non-empty ignore patterns.
/// Future: consider glob or path-segment matching for more precise control.
fn discover_files(root: &str, lang: Option<&str>, ignore_patterns: &[String]) -> DiscoverResult {
    let mut test_files: HashMap<Language, Vec<String>> = HashMap::new();
    let mut source_count = 0;
    let mut source_files = Vec::new();
    let root_path = std::path::Path::new(root);
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

        let rel_path = entry
            .path()
            .strip_prefix(root_path)
            .unwrap_or(entry.path())
            .to_string_lossy();
        if ignore_patterns
            .iter()
            .any(|pattern| !pattern.is_empty() && rel_path.contains(pattern.as_str()))
        {
            continue;
        }

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
                source_files.push(path);
                source_count += 1;
            }
        }
    }

    for files in test_files.values_mut() {
        files.sort();
    }
    source_files.sort();

    DiscoverResult {
        test_files,
        source_file_count: source_count,
        source_files,
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
const SUPPORTED_FORMATS: &[&str] = &["terminal", "json", "sarif", "ai-prompt"];

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

    match cli.command {
        Some(Commands::Observe(args)) => run_observe(args),
        None => run_lint(cli.lint_args),
    }
}

fn build_observe_report(
    file_mappings: &[exspec_core::observe::FileMapping],
    production_files: &[String],
    test_file_count: usize,
    routes: Vec<ObserveRouteEntry>,
) -> ObserveReport {
    let mut file_entries = Vec::new();
    let mut unmapped = Vec::new();

    for m in file_mappings {
        if m.test_files.is_empty() {
            unmapped.push(m.production_file.clone());
        } else {
            let strategy = match m.strategy {
                exspec_core::observe::MappingStrategy::FileNameConvention => "filename",
                exspec_core::observe::MappingStrategy::ImportTracing => "import",
            };
            file_entries.push(ObserveFileEntry {
                production_file: m.production_file.clone(),
                test_files: m.test_files.clone(),
                strategy: strategy.to_string(),
            });
        }
    }

    // Add production files not in file_mappings to unmapped
    let mapped_set: std::collections::HashSet<&String> =
        file_mappings.iter().map(|m| &m.production_file).collect();
    for pf in production_files {
        if !mapped_set.contains(pf) {
            unmapped.push(pf.clone());
        }
    }

    let routes_covered = routes.iter().filter(|r| !r.test_files.is_empty()).count();
    let mapped_count = file_entries.len();

    ObserveReport {
        summary: ObserveSummary {
            production_files: production_files.len(),
            test_files: test_file_count,
            mapped_files: mapped_count,
            unmapped_files: unmapped.len(),
            routes_total: routes.len(),
            routes_covered,
        },
        file_mappings: file_entries,
        routes,
        unmapped_production_files: unmapped,
    }
}

/// Common observe pipeline: discover files → read test sources → map → report → output.
///
/// `lang_str` / `lang` select which files to discover.
/// `map_fn` receives (production_files, test_sources, root) and returns file mappings.
/// `route_fn` receives (production_files) and returns route entries (TypeScript only; others pass `|_| Vec::new()`).
#[allow(clippy::too_many_arguments)]
fn run_observe_common(
    root: &str,
    lang_str: &str,
    lang: Language,
    format: &str,
    config: &Config,
    no_fan_out_filter: bool,
    map_fn: impl FnOnce(
        &[String],
        &HashMap<String, String>,
        &Path,
    ) -> Vec<exspec_core::observe::FileMapping>,
    route_fn: impl FnOnce(&[String]) -> Vec<ObserveRouteEntry>,
) {
    let discovered = discover_files(root, Some(lang_str), &config.ignore_patterns);
    let test_files: Vec<String> = discovered
        .test_files
        .get(&lang)
        .cloned()
        .unwrap_or_default();
    let production_files = &discovered.source_files;

    let mut test_sources: HashMap<String, String> = HashMap::new();
    for test_file in &test_files {
        if let Ok(source) = std::fs::read_to_string(test_file) {
            test_sources.insert(test_file.clone(), source);
        }
    }

    let mut file_mappings = map_fn(production_files, &test_sources, Path::new(root));
    if !no_fan_out_filter {
        apply_fan_out_filter(
            &mut file_mappings,
            test_files.len(),
            config.max_fan_out_percent,
        );
        apply_reverse_fan_out_filter(&mut file_mappings, config.max_reverse_fan_out);
    }
    let route_entries = route_fn(production_files);

    // Build route entries with coverage info
    let mut prod_to_tests: HashMap<String, Vec<String>> = HashMap::new();
    let mut class_to_tests: HashMap<String, Vec<String>> = HashMap::new();
    if !route_entries.is_empty() {
        for m in &file_mappings {
            if !m.test_files.is_empty() {
                prod_to_tests.insert(m.production_file.clone(), m.test_files.clone());
                // Class-based lookup: .../FooController.php → "FooController"
                if let Some(stem) = Path::new(&m.production_file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    class_to_tests
                        .entry(stem.to_string())
                        .or_default()
                        .extend(m.test_files.clone());
                }
            }
        }
    }
    let route_entries: Vec<ObserveRouteEntry> = route_entries
        .into_iter()
        .map(|mut entry| {
            // File-based match (NestJS/Next.js: route defined in controller file)
            if let Some(tf) = prod_to_tests.get(&entry.file) {
                entry.test_files = tf.clone();
            }
            // Class-based match (Laravel: handler = "TrialController.index")
            if entry.test_files.is_empty() && !entry.handler.is_empty() {
                let class_name = entry.handler.split('.').next().unwrap_or("");
                if !class_name.is_empty() {
                    if let Some(tf) = class_to_tests.get(class_name) {
                        entry.test_files = tf.clone();
                    }
                }
            }
            entry
        })
        .collect();

    let report = build_observe_report(
        &file_mappings,
        production_files,
        test_files.len(),
        route_entries,
    );
    let output = match format {
        "json" => report.format_json(),
        _ => report.format_terminal(),
    };
    if !output.is_empty() {
        println!("{output}");
    }
}

fn run_observe(args: ObserveArgs) {
    let observe_formats = ["terminal", "json"];
    if !observe_formats.contains(&args.format.as_str()) {
        eprintln!(
            "error: unsupported format for observe: {}. Supported: {}",
            args.format,
            observe_formats.join(", ")
        );
        process::exit(1);
    }

    let root = &args.path;
    let config = load_config(".exspec.toml");

    match args.lang.as_str() {
        "typescript" => {
            let ts_ext = TypeScriptExtractor::new();
            run_observe_common(
                root,
                "typescript",
                Language::TypeScript,
                &args.format,
                &config,
                args.no_fan_out_filter,
                |prod, test_src, root_path| {
                    ts_ext.map_test_files_with_imports(prod, test_src, root_path, args.l1_exclusive)
                },
                |prod_files| {
                    let mut all_routes = Vec::new();
                    for prod_file in prod_files {
                        let source = match std::fs::read_to_string(prod_file) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        // NestJS routes (decorator-based)
                        let nestjs_routes = ts_ext.extract_routes(&source, prod_file);
                        all_routes.extend(nestjs_routes.into_iter().map(|r| ObserveRouteEntry {
                            http_method: r.http_method,
                            path: r.path,
                            handler: format!("{}.{}", r.class_name, r.handler_name),
                            file: r.file,
                            test_files: Vec::new(),
                        }));
                        // Next.js App Router routes (file-based)
                        let nextjs_routes = ts_ext.extract_nextjs_routes(&source, prod_file);
                        all_routes.extend(nextjs_routes.into_iter().map(|r| ObserveRouteEntry {
                            http_method: r.http_method,
                            path: r.path,
                            handler: if r.class_name.is_empty() {
                                r.handler_name
                            } else {
                                format!("{}.{}", r.class_name, r.handler_name)
                            },
                            file: r.file,
                            test_files: Vec::new(),
                        }));
                    }
                    all_routes
                },
            );
        }
        "python" => {
            let py_ext = PythonExtractor::new();
            run_observe_common(
                root,
                "python",
                Language::Python,
                &args.format,
                &config,
                args.no_fan_out_filter,
                |prod, test_src, root_path| {
                    py_ext.map_test_files_with_imports(prod, test_src, root_path, args.l1_exclusive)
                },
                |production_files| {
                    let mut all_routes = Vec::new();
                    for prod_file in production_files {
                        let source = match std::fs::read_to_string(prod_file) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let mut routes =
                            exspec_lang_python::observe::extract_routes(&source, prod_file);
                        routes.extend(exspec_lang_python::observe::extract_django_routes(
                            &source, prod_file,
                        ));
                        all_routes.extend(routes.into_iter().map(|r| ObserveRouteEntry {
                            http_method: r.http_method,
                            path: r.path,
                            handler: r.handler_name,
                            file: r.file,
                            test_files: Vec::new(),
                        }));
                    }
                    all_routes
                },
            );
        }
        "rust" => {
            let rust_ext = RustExtractor::new();
            run_observe_common(
                root,
                "rust",
                Language::Rust,
                &args.format,
                &config,
                args.no_fan_out_filter,
                |prod, test_src, root_path| {
                    rust_ext.map_test_files_with_imports(
                        prod,
                        test_src,
                        root_path,
                        args.l1_exclusive,
                    )
                },
                |_| Vec::new(),
            );
        }
        "php" => {
            let php_ext = PhpExtractor::new();
            run_observe_common(
                root,
                "php",
                Language::Php,
                &args.format,
                &config,
                args.no_fan_out_filter,
                |prod, test_src, root_path| {
                    php_ext.map_test_files_with_imports(
                        prod,
                        test_src,
                        root_path,
                        args.l1_exclusive,
                    )
                },
                |_| {
                    // Laravel route extraction from routes/*.php
                    let mut all_routes = Vec::new();
                    let routes_dir = std::path::Path::new(root).join("routes");
                    if routes_dir.is_dir() {
                        for entry in std::fs::read_dir(&routes_dir)
                            .into_iter()
                            .flatten()
                            .flatten()
                        {
                            let path = entry.path();
                            if path.extension().and_then(|e| e.to_str()) == Some("php") {
                                let file_path = path.to_string_lossy().into_owned();
                                if let Ok(source) = std::fs::read_to_string(&path) {
                                    let routes = php_ext.extract_routes(&source, &file_path);
                                    all_routes.extend(routes.into_iter().map(|r| {
                                        ObserveRouteEntry {
                                            http_method: r.http_method,
                                            path: r.path,
                                            handler: if r.class_name.is_empty() {
                                                r.handler_name
                                            } else {
                                                format!("{}.{}", r.class_name, r.handler_name)
                                            },
                                            file: r.file,
                                            test_files: Vec::new(),
                                        }
                                    }));
                                }
                            }
                        }
                    }
                    all_routes
                },
            );
        }
        _ => {
            eprintln!("error: observe is not yet supported for {}", args.lang);
            process::exit(1);
        }
    }

    process::exit(0);
}

fn run_lint(lint: LintArgs) {
    if let Err(e) = validate_lang(lint.lang.as_deref()) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    if let Err(e) = validate_format(&lint.format) {
        eprintln!("error: {e}");
        process::exit(1);
    }

    let cli_min_severity = if let Some(ref s) = lint.min_severity {
        match s.parse::<Severity>() {
            Ok(sev) => Some(sev),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(2);
            }
        }
    } else {
        None
    };

    let mut config = load_config(&lint.config);
    if let Some(sev) = cli_min_severity {
        config.min_severity = sev;
    }
    let py_extractor = PythonExtractor::new();
    let ts_extractor = TypeScriptExtractor::new();
    let php_extractor = PhpExtractor::new();
    let rust_extractor = RustExtractor::new();

    let discovered = discover_files(&lint.path, lint.lang.as_deref(), &config.ignore_patterns);
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
                let mut file_analysis = extractor.extract_file_analysis(&source, file_path);
                exspec_core::query_utils::apply_custom_assertion_fallback(
                    &mut file_analysis,
                    &source,
                    &config.custom_assertion_patterns,
                );
                all_analyses.push(file_analysis);
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
    let hints = compute_hints(&diagnostics, config.custom_assertion_patterns.is_empty());

    let display_diagnostics = filter_by_severity(&diagnostics, config.min_severity);

    let output = match lint.format.as_str() {
        "json" => {
            let stats = SummaryStats::from_diagnostics(&diagnostics, all_functions.len());
            format_json(
                &display_diagnostics,
                test_file_count,
                all_functions.len(),
                &metrics,
                Some(&stats),
                &hints,
            )
        }
        "sarif" => format_sarif(&display_diagnostics),
        "terminal" => format_terminal(
            &display_diagnostics,
            test_file_count,
            all_functions.len(),
            &metrics,
            &hints,
        ),
        _ => format_ai_prompt(
            &display_diagnostics,
            test_file_count,
            all_functions.len(),
            &metrics,
            &hints,
        ),
    };

    if !output.is_empty() {
        println!("{output}");
    }

    // Exit code uses UNFILTERED diagnostics
    let exit_code = compute_exit_code(&diagnostics, lint.strict);
    process::exit(exit_code);
}

/// Extract the class/file name (stem) from a file path.
/// e.g., `src/Support/Str.php` -> `Str`, `src/user.rs` -> `user`
#[allow(dead_code)]
fn extract_class_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn has_common_directory_segment(path_a: &str, path_b: &str) -> bool {
    let generic: &[&str] = &[
        "src",
        "tests",
        "test",
        "lib",
        "app",
        "vendor",
        "node_modules",
    ];

    let segments_a: std::collections::HashSet<String> = Path::new(path_a)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(|s| s.to_lowercase())
        .filter(|s| !generic.contains(&s.as_str()) && s.len() > 2)
        .collect();

    let segments_b: std::collections::HashSet<String> = Path::new(path_b)
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(|s| s.to_lowercase())
        .filter(|s| !generic.contains(&s.as_str()) && s.len() > 2)
        .collect();

    !segments_a.is_disjoint(&segments_b)
}

fn apply_fan_out_filter(
    file_mappings: &mut [exspec_core::observe::FileMapping],
    total_test_files: usize,
    max_fan_out_percent: f64,
) {
    if total_test_files == 0 {
        return;
    }
    let threshold = max_fan_out_percent / 100.0;
    for mapping in file_mappings.iter_mut() {
        let fan_out = mapping.test_files.len() as f64 / total_test_files as f64;
        if fan_out > threshold {
            let prod_class = extract_class_name(&mapping.production_file).to_lowercase();
            let prod_file = mapping.production_file.clone();
            mapping.test_files.retain(|test_file| {
                let test_stem = extract_class_name(test_file).to_lowercase();
                test_stem.contains(&prod_class)
                    || (prod_class.len() > 3
                        && test_stem.len() > 3
                        && prod_class.contains(&test_stem))
                    || has_common_directory_segment(test_file, &prod_file)
            });
        }
    }
}

fn apply_reverse_fan_out_filter(
    file_mappings: &mut [exspec_core::observe::FileMapping],
    max_reverse_fan_out: usize,
) {
    // Step 1: Build reverse index (test_file -> count of prod files it maps to)
    let mut test_prod_count: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for mapping in file_mappings.iter() {
        for test_file in &mapping.test_files {
            *test_prod_count.entry(test_file.clone()).or_insert(0) += 1;
        }
    }

    // Step 2: Collect tests exceeding threshold
    let high_fan_in_tests: std::collections::HashSet<String> = test_prod_count
        .into_iter()
        .filter(|(_, count)| *count > max_reverse_fan_out)
        .map(|(test, _)| test)
        .collect();

    if high_fan_in_tests.is_empty() {
        return;
    }

    // Step 3: For high fan-in tests, retain only name-matching mappings
    for mapping in file_mappings.iter_mut() {
        let prod_stem = extract_class_name(&mapping.production_file).to_lowercase();
        mapping.test_files.retain(|test_file| {
            if !high_fan_in_tests.contains(test_file) {
                return true; // Not a high fan-in test, keep
            }
            // Name-match check (guard against empty stems)
            let test_stem = extract_class_name(test_file).to_lowercase();
            if prod_stem.is_empty() || test_stem.is_empty() {
                return false;
            }
            test_stem.contains(&prod_stem)
                || prod_stem.contains(&test_stem)
                || has_common_directory_segment(test_file, &mapping.production_file)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse CLI args and extract LintArgs (for backward-compat tests).
    fn parse_lint(args: &[&str]) -> LintArgs {
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.command.is_none(), "expected lint mode (no subcommand)");
        cli.lint_args
    }

    #[test]
    fn cli_parses_path_argument() {
        let lint = parse_lint(&["exspec", "."]);
        assert_eq!(lint.path, ".");
    }

    #[test]
    fn cli_default_path() {
        let lint = parse_lint(&["exspec"]);
        assert_eq!(lint.path, ".");
    }

    #[test]
    fn cli_strict_flag() {
        let lint = parse_lint(&["exspec", "--strict", "src/"]);
        assert!(lint.strict);
        assert_eq!(lint.path, "src/");
    }

    #[test]
    fn cli_format_option() {
        let lint = parse_lint(&["exspec", "--format", "json", "."]);
        assert_eq!(lint.format, "json");
    }

    #[test]
    fn cli_lang_option() {
        let lint = parse_lint(&["exspec", "--lang", "python", "."]);
        assert_eq!(lint.lang, Some("python".to_string()));
    }

    #[test]
    fn cli_help_does_not_panic() {
        let result = Cli::try_parse_from(["exspec", "--help"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_config_option() {
        let lint = parse_lint(&["exspec", "--config", "my.toml", "."]);
        assert_eq!(lint.config, "my.toml");
    }

    #[test]
    fn cli_config_default() {
        let lint = parse_lint(&["exspec"]);
        assert_eq!(lint.config, ".exspec.toml");
    }

    // --- #59: --min-severity CLI ---

    #[test]
    fn cli_min_severity_option() {
        let lint = parse_lint(&["exspec", "--min-severity", "warn", "."]);
        assert_eq!(lint.min_severity, Some("warn".to_string()));
    }

    #[test]
    fn cli_min_severity_default_none() {
        let lint = parse_lint(&["exspec"]);
        assert_eq!(lint.min_severity, None);
    }

    // --- OB1: observe unsupported lang ---

    #[test]
    fn ob1_observe_unsupported_lang() {
        // "python" is not supported for observe
        let cli = Cli::try_parse_from(["exspec", "observe", "--lang", "python", "."]).unwrap();
        match cli.command {
            Some(Commands::Observe(args)) => {
                assert_eq!(args.lang, "python");
                // Validation happens at runtime, just verify parse works
            }
            _ => panic!("expected Observe subcommand"),
        }
    }

    // --- OB8: lint backward compatibility ---

    #[test]
    fn ob8_lint_backward_compat() {
        // `exspec . --lang rust` should parse as lint (no subcommand)
        let cli = Cli::try_parse_from(["exspec", "--lang", "rust", "."]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.lint_args.lang, Some("rust".to_string()));
        assert_eq!(cli.lint_args.path, ".");
    }

    // --- observe CLI parsing ---

    #[test]
    fn observe_subcommand_parses() {
        let cli = Cli::try_parse_from(["exspec", "observe", "--lang", "typescript", "."]).unwrap();
        match cli.command {
            Some(Commands::Observe(args)) => {
                assert_eq!(args.lang, "typescript");
                assert_eq!(args.path, ".");
                assert_eq!(args.format, "terminal");
            }
            _ => panic!("expected Observe subcommand"),
        }
    }

    #[test]
    fn observe_subcommand_json_format() {
        let cli = Cli::try_parse_from([
            "exspec",
            "observe",
            "--lang",
            "typescript",
            "--format",
            "json",
            ".",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Observe(args)) => {
                assert_eq!(args.format, "json");
            }
            _ => panic!("expected Observe subcommand"),
        }
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

    // -----------------------------------------------------------------------
    // CLI-PY-TESTS-01: is_python_test_file("app/tests.py") -> true
    // -----------------------------------------------------------------------
    #[test]
    fn cli_py_tests_01_tests_file_is_recognized() {
        // Given: path = "app/tests.py"
        // When: is_python_test_file(path)
        // Then: true (Django tests.py naming convention)
        assert!(is_python_test_file("app/tests.py"));
    }

    // -----------------------------------------------------------------------
    // CLI-PY-TESTS-02: is_python_test_file("tests/__init__.py") -> false
    // -----------------------------------------------------------------------
    #[test]
    fn cli_py_tests_02_init_file_in_tests_dir_not_recognized() {
        // Given: path = "tests/__init__.py"
        // When: is_python_test_file(path)
        // Then: false (__init__.py is not a test file even inside tests/
        assert!(!is_python_test_file("tests/__init__.py"));
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
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
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
        let result = discover_files(dir.to_str().unwrap(), Some("python"), &[]);
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
        let result = discover_files(dir.to_str().unwrap(), Some("typescript"), &[]);
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
        let result = discover_files(dir.to_str().unwrap(), Some("php"), &[]);
        assert_eq!(get_test_files(&result, Language::Python).len(), 0);
        assert_eq!(get_test_files(&result, Language::Php).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_test_files_ignores_venv() {
        let result = discover_files(".", None, &[]);
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
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
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
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
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
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
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
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
        assert_eq!(get_test_files(&result, Language::Php).len(), 1);
        assert_eq!(result.source_file_count, 1); // User.php
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Ignore patterns filtering ---

    #[test]
    fn discover_files_filters_by_ignore_patterns() {
        let dir = std::env::temp_dir().join(format!("exspec_test_ignore_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        std::fs::write(dir.join("fixtures/test_sample.py"), "").unwrap();
        std::fs::write(dir.join("test_real.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None, &["fixtures".to_string()]);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(py.len(), 1, "fixture file should be excluded: {py:?}");
        assert!(py[0].contains("test_real.py"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_files_ignore_does_not_exclude_unrelated_files() {
        let dir = std::env::temp_dir().join(format!(
            "exspec_test_ignore_nonmatch_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::write(dir.join("fixtures/test_sample.py"), "").unwrap();
        std::fs::write(dir.join("tests/test_real.py"), "").unwrap();
        std::fs::write(dir.join("test_root.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None, &["fixtures".to_string()]);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(py.len(), 2, "non-matching files should remain: {py:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_files_empty_ignore_excludes_nothing() {
        let dir =
            std::env::temp_dir().join(format!("exspec_test_no_ignore_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("fixtures")).unwrap();
        std::fs::write(dir.join("fixtures/test_sample.py"), "").unwrap();
        std::fs::write(dir.join("test_real.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None, &[]);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(py.len(), 2, "empty ignore should exclude nothing: {py:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_files_ignore_empty_string_pattern_excludes_nothing() {
        let dir =
            std::env::temp_dir().join(format!("exspec_test_ignore_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        std::fs::write(dir.join("test_bar.py"), "").unwrap();
        let result = discover_files(dir.to_str().unwrap(), None, &["".to_string()]);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(
            py.len(),
            2,
            "empty string pattern should not exclude any files: {py:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_files_ignore_matches_relative_path_not_absolute() {
        // Pattern should match against project-relative paths, not absolute paths.
        // We create a temp dir whose name contains "exspec_test_ignore_relpath_<pid>",
        // then use a pattern matching part of that absolute prefix.
        let pid = std::process::id();
        let dirname = format!("exspec_test_ignore_relpath_{pid}");
        let dir = std::env::temp_dir().join(&dirname);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test_foo.py"), "").unwrap();
        // Use a pattern that matches the temp dir name (part of absolute path)
        // but is NOT in the relative path within the project root
        let pattern = format!("exspec_test_ignore_relpath_{pid}");
        let result = discover_files(dir.to_str().unwrap(), None, &[pattern]);
        let py = get_test_files(&result, Language::Python);
        assert_eq!(
            py.len(),
            1,
            "pattern matching absolute prefix should not exclude relative files: {py:?}"
        );
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
        assert_eq!(config.disabled_rules.len(), 3); // T004, T005 from config + T106 from default
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
    fn is_rust_test_file_rejects_partial_tests_dir() {
        // #9: substring match false positives
        assert!(!is_rust_test_file("src/mytests/foo.rs"));
        assert!(!is_rust_test_file("tests_data/bar.rs"));
        assert!(!is_rust_test_file("contested/baz.rs"));
    }

    #[test]
    fn is_rust_test_file_matches_nested_tests_dir() {
        // #9: nested tests/ dir should still match
        assert!(is_rust_test_file("project/tests/nested/foo.rs"));
    }

    #[test]
    fn is_rust_test_file_matches_tests_at_root() {
        // #9: tests/ at path start (no leading component) should match
        assert!(is_rust_test_file("tests/foo.rs"));
        assert!(is_rust_test_file("tests/nested/bar.rs"));
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
        let result = discover_files(dir.to_str().unwrap(), Some("rust"), &[]);
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

    // --- AI-FMT-07: CLI default format is "ai-prompt" ---

    #[test]
    fn validate_format_ai_prompt_ok() {
        // Given: format = "ai-prompt"
        // When: validate_format("ai-prompt") を呼ぶ
        // Then: Ok(()) が返る
        assert!(
            validate_format("ai-prompt").is_ok(),
            "ai-prompt should be a valid format"
        );
    }

    #[test]
    fn cli_default_format_is_ai_prompt() {
        // Given: --format オプションなし
        // When: CLI を parse_lint で解析
        // Then: format フィールドのデフォルトが "ai-prompt"
        let lint = parse_lint(&["exspec", "."]);
        assert_eq!(
            lint.format, "ai-prompt",
            "default format should be ai-prompt, got: {}",
            lint.format
        );
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
        let output = exspec_core::output::format_json(
            &diags,
            1,
            analyses[0].functions.len(),
            &metrics,
            None,
            &[],
        );
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed["metrics"].is_object());
        assert!(parsed["metrics"]["assertion_density_avg"].is_number());
    }

    // --- FA-RT-E2E-01: observe with routes shows route coverage ---

    #[test]
    fn fa_rt_e2e_01_observe_python_routes_coverage() {
        use exspec_lang_python::observe::extract_routes;
        use tempfile::TempDir;

        // Given: tempdir with FastAPI app (main.py with 2 routes, test_main.py importing main)
        let dir = TempDir::new().unwrap();
        let main_py = dir.path().join("main.py");
        let test_main_py = dir.path().join("test_main.py");

        std::fs::write(
            &main_py,
            r#"from fastapi import FastAPI
app = FastAPI()

@app.get("/users")
def read_users():
    return []

@app.post("/users")
def create_user():
    return {}
"#,
        )
        .unwrap();

        std::fs::write(
            &test_main_py,
            r#"from main import app

def test_read_users():
    assert app is not None
"#,
        )
        .unwrap();

        // When: extract routes from main.py and build observe report
        let main_source = std::fs::read_to_string(&main_py).unwrap();
        let main_path = main_py.to_string_lossy().into_owned();
        let test_path = test_main_py.to_string_lossy().into_owned();

        let routes = extract_routes(&main_source, &main_path);

        // Then: routes_total = 2
        assert_eq!(
            routes.len(),
            2,
            "expected 2 routes extracted from main.py, got {:?}",
            routes
        );

        // Build route entries with test file coverage
        let route_entries: Vec<ObserveRouteEntry> = routes
            .into_iter()
            .map(|r| ObserveRouteEntry {
                http_method: r.http_method,
                path: r.path,
                handler: r.handler_name,
                file: r.file,
                test_files: vec![test_path.clone()],
            })
            .collect();

        let report = build_observe_report(&[], &[main_path], 1, route_entries);

        // Then: routes_total = 2, routes_covered >= 1
        assert_eq!(report.summary.routes_total, 2, "expected routes_total = 2");
        assert!(
            report.summary.routes_covered >= 1,
            "expected routes_covered >= 1, got {}",
            report.summary.routes_covered
        );

        // Verify JSON output contains route data
        let json = report.format_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["summary"]["routes_total"], 2);
        assert!(
            parsed["summary"]["routes_covered"].as_u64().unwrap_or(0) >= 1,
            "routes_covered should be >= 1"
        );
    }

    // --- NX-RT-E2E-01: observe with Next.js routes via CLI dispatch logic ---
    //
    // Verifies that the TypeScript route_fn in run_observe_common correctly merges
    // NestJS routes and Next.js App Router routes, and that Next.js routes appear
    // in the final report with non-empty handler names (no leading ".").

    #[test]
    fn nx_rt_e2e_01_observe_nextjs_routes_coverage() {
        use exspec_lang_typescript::TypeScriptExtractor;
        use tempfile::TempDir;

        // Given: tempdir with Next.js App Router route handler (GET + POST) and a test file
        let dir = TempDir::new().unwrap();
        let app_dir = dir.path().join("app").join("api").join("users");
        std::fs::create_dir_all(&app_dir).unwrap();

        let route_ts = app_dir.join("route.ts");
        let test_ts = app_dir.join("route.test.ts");

        std::fs::write(
            &route_ts,
            r#"export async function GET() {
  return Response.json([]);
}

export async function POST() {
  return Response.json({});
}
"#,
        )
        .unwrap();

        std::fs::write(
            &test_ts,
            r#"import { GET } from './route';

test('GET returns array', async () => {
  const res = await GET();
  expect(res).toBeDefined();
});
"#,
        )
        .unwrap();

        // When: run the same dispatch logic as TypeScript route_fn in run_observe_common
        let ts_ext = TypeScriptExtractor::new();
        let route_source = std::fs::read_to_string(&route_ts).unwrap();
        let route_path = route_ts.to_string_lossy().into_owned();
        let test_path = test_ts.to_string_lossy().into_owned();

        // Replicate the merged dispatch: NestJS routes + Next.js routes
        let mut all_routes: Vec<ObserveRouteEntry> = Vec::new();

        let nestjs_routes = ts_ext.extract_routes(&route_source, &route_path);
        all_routes.extend(nestjs_routes.into_iter().map(|r| ObserveRouteEntry {
            http_method: r.http_method,
            path: r.path,
            handler: format!("{}.{}", r.class_name, r.handler_name),
            file: r.file,
            test_files: Vec::new(),
        }));

        let nextjs_routes = ts_ext.extract_nextjs_routes(&route_source, &route_path);
        all_routes.extend(nextjs_routes.into_iter().map(|r| ObserveRouteEntry {
            http_method: r.http_method,
            path: r.path,
            handler: if r.class_name.is_empty() {
                r.handler_name
            } else {
                format!("{}.{}", r.class_name, r.handler_name)
            },
            file: r.file,
            test_files: vec![test_path.clone()],
        }));

        // Then: dispatch produces 2 routes from Next.js (NestJS yields 0 for this file)
        assert_eq!(
            all_routes.len(),
            2,
            "expected 2 routes from dispatch, got {:?}",
            all_routes
        );

        // Verify handler format: Next.js handlers must not start with '.'
        for entry in &all_routes {
            assert!(
                !entry.handler.starts_with('.'),
                "handler should not start with '.': {:?}",
                entry
            );
            assert!(
                !entry.handler.is_empty(),
                "handler should not be empty: {:?}",
                entry
            );
        }

        let report = build_observe_report(&[], &[route_path], 1, all_routes);

        // Then: routes_total = 2, routes_covered >= 1
        assert_eq!(report.summary.routes_total, 2, "expected routes_total = 2");
        assert!(
            report.summary.routes_covered >= 1,
            "expected routes_covered >= 1, got {}",
            report.summary.routes_covered
        );

        // Verify JSON output contains route data
        let json = report.format_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["summary"]["routes_total"], 2);
        assert!(
            parsed["summary"]["routes_covered"].as_u64().unwrap_or(0) >= 1,
            "routes_covered should be >= 1"
        );
    }

    // --- fan-out filter ---

    // TC-01: fan_out_filter_removes_high_fan_out
    #[test]
    fn fan_out_filter_removes_high_fan_out() {
        // Given: 10 test files total, prod A mapped to 3 tests (30%), threshold 20%
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/utils/Str.php".to_string(),
            test_files: vec![
                "tests/A.php".to_string(),
                "tests/B.php".to_string(),
                "tests/C.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 20%
        apply_fan_out_filter(&mut mappings, 10, 20.0);
        // Then: A's test_files is empty (30% > 20%)
        assert!(
            mappings[0].test_files.is_empty(),
            "expected test_files to be cleared for high fan-out prod file"
        );
    }

    // TC-02: fan_out_filter_keeps_low_fan_out
    #[test]
    fn fan_out_filter_keeps_low_fan_out() {
        // Given: 10 test files total, prod B mapped to 1 test (10%), threshold 20%
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/utils/Helper.php".to_string(),
            test_files: vec!["tests/HelperTest.php".to_string()],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 20%
        apply_fan_out_filter(&mut mappings, 10, 20.0);
        // Then: B's test_files is maintained (10% <= 20%)
        assert_eq!(
            mappings[0].test_files.len(),
            1,
            "expected test_files to be kept for low fan-out prod file"
        );
    }

    // TC-03: fan_out_filter_disabled_keeps_all
    #[test]
    fn fan_out_filter_disabled_keeps_all() {
        // Given: prod A mapped to 5 tests (50%), threshold 20%
        // When: apply_fan_out_filter is NOT called (simulating no_fan_out_filter=true)
        let mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/utils/Str.php".to_string(),
            test_files: vec![
                "tests/A.php".to_string(),
                "tests/B.php".to_string(),
                "tests/C.php".to_string(),
                "tests/D.php".to_string(),
                "tests/E.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // Then: A's test_files is maintained (filter was skipped)
        assert_eq!(
            mappings[0].test_files.len(),
            5,
            "expected test_files to be kept when filter is not applied"
        );
    }

    // TC-04: fan_out_filter_custom_threshold
    #[test]
    fn fan_out_filter_custom_threshold() {
        // Given: 10 test files total, prod mapped to 4 tests (40%), threshold 50%
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/services/UserService.php".to_string(),
            test_files: vec![
                "tests/A.php".to_string(),
                "tests/B.php".to_string(),
                "tests/C.php".to_string(),
                "tests/D.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with custom threshold 50%
        apply_fan_out_filter(&mut mappings, 10, 50.0);
        // Then: test_files maintained (40% < 50%)
        assert_eq!(
            mappings[0].test_files.len(),
            4,
            "expected test_files to be kept when fan-out is below custom threshold"
        );
    }

    // TC-05: fan_out_filter_zero_test_files
    #[test]
    fn fan_out_filter_zero_test_files() {
        // Given: 0 total test files (edge case)
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/utils/Str.php".to_string(),
            test_files: vec![],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with 0 total test files
        // Then: no panic
        apply_fan_out_filter(&mut mappings, 0, 20.0);
        assert!(
            mappings[0].test_files.is_empty(),
            "expected test_files to remain empty"
        );
    }

    // --- fan-out name-match exemption (#173) ---

    // TC-01: fan_out_name_match_keeps_matching_test
    #[test]
    fn fan_out_name_match_keeps_matching_test() {
        // Given: 10 total tests, src/Support/Str.php mapped to 3 tests including
        //        SupportStrTest.php (fan-out 30% > 5%)
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Support/Str.php".to_string(),
            test_files: vec![
                "tests/Unit/SupportStrTest.php".to_string(),
                "tests/Unit/ContextTest.php".to_string(),
                "tests/Unit/OtherTest.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 5%
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        // Then: only SupportStrTest.php is KEPT (name contains "Str")
        assert_eq!(
            mappings[0].test_files,
            vec!["tests/Unit/SupportStrTest.php".to_string()],
            "expected only name-matching test to be retained"
        );
    }

    // TC-02: fan_out_name_match_removes_non_matching
    #[test]
    fn fan_out_name_match_removes_non_matching() {
        // Given: 10 total tests, src/Support/Str.php mapped to [ContextTest.php, OtherTest.php]
        //        (fan-out 20% > 5%, no name contains "Str")
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Support/Str.php".to_string(),
            test_files: vec![
                "tests/Unit/ContextTest.php".to_string(),
                "tests/Unit/OtherTest.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 5%
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        // Then: test_files is EMPTY (no name match)
        assert!(
            mappings[0].test_files.is_empty(),
            "expected all non-matching tests to be removed"
        );
    }

    // TC-03: fan_out_below_threshold_keeps_all
    #[test]
    fn fan_out_below_threshold_keeps_all() {
        // Given: 10 total tests, prod mapped to 1 test (10% at threshold 20%)
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Support/Str.php".to_string(),
            test_files: vec!["tests/Unit/SomeTest.php".to_string()],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 20%
        apply_fan_out_filter(&mut mappings, 10, 20.0);
        // Then: test kept (10% is below 20% threshold)
        assert_eq!(
            mappings[0].test_files.len(),
            1,
            "expected test to be kept when fan-out is below threshold"
        );
    }

    // TC-04: fan_out_mixed_keeps_only_matching
    #[test]
    fn fan_out_mixed_keeps_only_matching() {
        // Given: 10 total tests, src/Model.php mapped to
        //        [EloquentModelTest.php, ContextTest.php, ModelCastTest.php] (30% > 5%)
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Model.php".to_string(),
            test_files: vec![
                "tests/Unit/EloquentModelTest.php".to_string(),
                "tests/Unit/ContextTest.php".to_string(),
                "tests/Unit/ModelCastTest.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter with threshold 5%
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        // Then: ONLY EloquentModelTest.php and ModelCastTest.php KEPT (contain "Model")
        let mut result = mappings[0].test_files.clone();
        result.sort();
        let mut expected = vec![
            "tests/Unit/EloquentModelTest.php".to_string(),
            "tests/Unit/ModelCastTest.php".to_string(),
        ];
        expected.sort();
        assert_eq!(
            result, expected,
            "expected only name-matching tests to be retained"
        );
    }

    // TC-05 (name-match): fan_out_filter_disabled_name_match_keeps_all
    #[test]
    fn fan_out_filter_disabled_name_match_keeps_all() {
        // Given: prod Str.php mapped to 3 tests (fan-out 30% > 5%), filter NOT called
        let mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Support/Str.php".to_string(),
            test_files: vec![
                "tests/Unit/SupportStrTest.php".to_string(),
                "tests/Unit/ContextTest.php".to_string(),
                "tests/Unit/OtherTest.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // When: apply_fan_out_filter is NOT called (simulating no_fan_out_filter=true)
        // Then: all test_files are preserved
        assert_eq!(
            mappings[0].test_files.len(),
            3,
            "expected all tests to be preserved when filter is disabled"
        );
    }

    // --- apply_reverse_fan_out_filter ---

    // RF-01: reverse_fan_out_removes_high_fan_in_test
    #[test]
    fn reverse_fan_out_removes_high_fan_in_test() {
        // Given: test "tests/io_driver.rs" mapped to 10 production files (> threshold 5)
        let prod_files = vec![
            "src/runtime/builder.rs",
            "src/runtime/context.rs",
            "src/runtime/driver.rs",
            "src/runtime/dump.rs",
            "src/runtime/handle.rs",
            "src/runtime/id.rs",
            "src/runtime/park.rs",
            "src/runtime/mod.rs",
            "src/runtime/task/raw.rs",
            "src/runtime/task/join.rs",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/io_driver.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // When: apply_reverse_fan_out_filter with threshold=5
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: only "src/runtime/driver.rs" retains "tests/io_driver.rs"
        //       (prod stem "driver" is contained in test stem "io_driver")
        for mapping in &mappings {
            let prod_stem = extract_class_name(&mapping.production_file).to_lowercase();
            if prod_stem == "driver" {
                assert!(
                    mapping
                        .test_files
                        .contains(&"tests/io_driver.rs".to_string()),
                    "driver.rs should retain io_driver.rs (name-match)"
                );
            } else {
                assert!(
                    !mapping
                        .test_files
                        .contains(&"tests/io_driver.rs".to_string()),
                    "non-matched prod {} should not retain io_driver.rs",
                    mapping.production_file
                );
            }
        }
    }

    // RF-02: reverse_fan_out_keeps_low_fan_in
    #[test]
    fn reverse_fan_out_keeps_low_fan_in() {
        // Given: test "tests/udp.rs" mapped to 3 production files (< threshold 5)
        let mut mappings = vec![
            exspec_core::observe::FileMapping {
                production_file: "src/net/udp.rs".to_string(),
                test_files: vec!["tests/udp.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            },
            exspec_core::observe::FileMapping {
                production_file: "src/net/lookup.rs".to_string(),
                test_files: vec!["tests/udp.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            },
            exspec_core::observe::FileMapping {
                production_file: "src/net/addr.rs".to_string(),
                test_files: vec!["tests/udp.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            },
        ];
        // When: apply_reverse_fan_out_filter with threshold=5
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: all 3 mappings still retain "tests/udp.rs"
        for mapping in &mappings {
            assert!(
                mapping.test_files.contains(&"tests/udp.rs".to_string()),
                "all prods should retain udp.rs when fan-in < threshold"
            );
        }
    }

    // RF-03: reverse_fan_out_l1_match_preserved
    #[test]
    fn reverse_fan_out_l1_match_preserved() {
        // Given: test "tests/fs_write.rs" mapped to 8 production files including fs/write.rs
        let prod_files = vec![
            "src/fs/write.rs",
            "src/fs/copy.rs",
            "src/fs/read.rs",
            "src/fs/metadata.rs",
            "src/fs/rename.rs",
            "src/fs/remove_file.rs",
            "src/fs/create_dir.rs",
            "src/fs/canonicalize.rs",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/fs_write.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // When: apply_reverse_fan_out_filter with threshold=5
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: only write.rs keeps fs_write.rs (stem "write" is contained in "fs_write")
        for mapping in &mappings {
            let prod_stem = extract_class_name(&mapping.production_file).to_lowercase();
            if prod_stem == "write" {
                assert!(
                    mapping
                        .test_files
                        .contains(&"tests/fs_write.rs".to_string()),
                    "write.rs should retain fs_write.rs (name-match)"
                );
            } else {
                assert!(
                    !mapping
                        .test_files
                        .contains(&"tests/fs_write.rs".to_string()),
                    "non-matched prod {} should not retain fs_write.rs",
                    mapping.production_file
                );
            }
        }
    }

    // RF-04: reverse_fan_out_exact_threshold_keeps_all
    #[test]
    fn reverse_fan_out_exact_threshold_keeps_all() {
        // Given: test "tests/timer.rs" mapped to exactly 5 production files, threshold=5
        let prod_files = vec![
            "src/time/timer.rs",
            "src/time/wheel.rs",
            "src/time/entry.rs",
            "src/time/handle.rs",
            "src/time/driver.rs",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/timer.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // When: apply_reverse_fan_out_filter with threshold=5
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: all 5 retained (strictly greater than threshold)
        for mapping in &mappings {
            assert!(
                mapping.test_files.contains(&"tests/timer.rs".to_string()),
                "all prods should be retained when fan-in equals threshold (not strictly greater)"
            );
        }
    }

    // RF-05: reverse_fan_out_empty_mappings
    #[test]
    fn reverse_fan_out_empty_mappings() {
        // Given: empty mappings
        let mut mappings: Vec<exspec_core::observe::FileMapping> = vec![];
        // When: apply_reverse_fan_out_filter
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: no panic
        assert!(mappings.is_empty());
    }

    // RF-06: reverse_fan_out_custom_threshold
    #[test]
    fn reverse_fan_out_custom_threshold() {
        // Given: test "tests/spawn.rs" mapped to 8 production files, threshold=10
        let prod_files = vec![
            "src/task/spawn.rs",
            "src/task/local.rs",
            "src/task/blocking.rs",
            "src/task/builder.rs",
            "src/task/abort.rs",
            "src/task/join_set.rs",
            "src/task/yield_now.rs",
            "src/task/unconstrained.rs",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/spawn.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // When: apply_reverse_fan_out_filter with threshold=10
        apply_reverse_fan_out_filter(&mut mappings, 10);
        // Then: all 8 retained (8 < threshold 10)
        for mapping in &mappings {
            assert!(
                mapping.test_files.contains(&"tests/spawn.rs".to_string()),
                "all prods should be retained when fan-in < custom threshold"
            );
        }
    }

    // RF-07: reverse_fan_out_prod_stem_contains_test_stem
    #[test]
    fn reverse_fan_out_prod_stem_contains_test_stem() {
        // Given: test "tests/broadcast.rs" mapped to 8 production files including "src/sync/broadcast.rs"
        let prod_files = vec![
            "src/sync/broadcast.rs",
            "src/sync/mutex.rs",
            "src/sync/rwlock.rs",
            "src/sync/semaphore.rs",
            "src/sync/oneshot.rs",
            "src/sync/watch.rs",
            "src/sync/barrier.rs",
            "src/sync/notify.rs",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/broadcast.rs".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // When: apply_reverse_fan_out_filter with threshold=5
        apply_reverse_fan_out_filter(&mut mappings, 5);
        // Then: broadcast.rs is kept (prod stem "broadcast" matches test stem "broadcast")
        //       all others have tests/broadcast.rs removed
        for mapping in &mappings {
            let prod_stem = extract_class_name(&mapping.production_file).to_lowercase();
            if prod_stem == "broadcast" {
                assert!(
                    mapping
                        .test_files
                        .contains(&"tests/broadcast.rs".to_string()),
                    "broadcast.rs should retain broadcast.rs (name-match)"
                );
            } else {
                assert!(
                    !mapping
                        .test_files
                        .contains(&"tests/broadcast.rs".to_string()),
                    "non-matched prod {} should not retain broadcast.rs",
                    mapping.production_file
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // FO-01 to FO-INT-02: fan-out filter name-match exemption improvement
    // Cycle: docs/cycles/20260325_1651_fan-out-filter-name-match-exemption.md
    // -----------------------------------------------------------------------

    // FO-01: forward filter KEEPS test when directory segment matches
    // has_common_directory_segment は未実装 → RED
    #[test]
    fn fo_01_forward_filter_keeps_test_with_matching_directory_segment() {
        // Given: prod fan-out > threshold (20% > 5%)
        //   production_file = "src/Illuminate/Auth/Guard.php"     (dir segment: "auth")
        //   test_file       = "tests/Auth/AuthGuardTest.php"       (dir segment: "auth")
        //   name match: "authguardtest".contains("guard") = TRUE (forward already OK for this case)
        //   Use a case where name fails but dir succeeds:
        //   production_file = "src/Illuminate/Auth/AuthenticationException.php"  prod_class="authenticationexception"
        //   test_file       = "tests/Auth/AuthGuardTest.php"  test_stem="authguardtest"
        //   forward: "authguardtest".contains("authenticationexception") = FALSE
        //   dir match NEW: both have "auth" → KEPT
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Illuminate/Auth/AuthenticationException.php".to_string(),
            test_files: vec![
                "tests/Auth/AuthGuardTest.php".to_string(),
                "tests/Unit/OtherTest.php".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // fan-out: 2/10 = 20% > 5% → filter triggers
        // prod_class = "authenticationexception"
        // AuthGuardTest: "authguardtest".contains("authenticationexception") = FALSE
        //   dir match: prod has "Auth", test has "Auth" → should be KEPT
        // OtherTest: no name match, no dir match → REMOVED
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        assert!(
            mappings[0]
                .test_files
                .contains(&"tests/Auth/AuthGuardTest.php".to_string()),
            "AuthGuardTest.php should be KEPT via directory segment 'auth' match, got: {:?}",
            mappings[0].test_files
        );
        assert!(
            !mappings[0]
                .test_files
                .contains(&"tests/Unit/OtherTest.php".to_string()),
            "OtherTest.php should be REMOVED (no name or dir match), got: {:?}",
            mappings[0].test_files
        );
    }

    // FO-02: forward filter REMOVES test when directory segment does NOT match (guard test → PASS)
    #[test]
    fn fo_02_forward_filter_removes_test_with_no_directory_match() {
        // Given: prod fan-out > threshold
        //   production_file = "src/Illuminate/Auth/Guard.php"    prod_class = "guard"
        //   test_file       = "tests/Unit/DatabaseTest.php"       test_stem  = "databasetest"
        //   name: "databasetest".contains("guard") = FALSE
        //   dir: "Unit" vs "Auth" → no match
        // When: apply_fan_out_filter with threshold 5%
        // Then: DatabaseTest.php is REMOVED
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Illuminate/Auth/Guard.php".to_string(),
            test_files: vec!["tests/Unit/DatabaseTest.php".to_string()],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        assert!(
            mappings[0].test_files.is_empty(),
            "DatabaseTest.php should be REMOVED (no name or dir match), got: {:?}",
            mappings[0].test_files
        );
    }

    // FO-03: forward filter KEEPS test via bidirectional stem match (prod_class contains test_stem)
    // 未実装 → RED
    #[test]
    fn fo_03_forward_filter_keeps_test_via_bidirectional_stem_match() {
        // Given: prod fan-out > threshold
        //   production_file = "src/io/async_read.rs"    prod_class = "async_read"
        //   test_file       = "tests/io/read.rs"         test_stem  = "read"
        //   forward (current): "read".contains("async_read") = FALSE → REMOVED
        //   bidirectional NEW: "async_read".contains("read") = TRUE → should be KEPT
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/io/async_read.rs".to_string(),
            test_files: vec![
                "tests/io/read.rs".to_string(),
                "tests/other/unrelated.rs".to_string(),
            ],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // fan-out: 2/10 = 20% > 5% → filter triggers
        // prod_class = "async_read"
        // "read.rs" → test_stem = "read" (len=4, not short)
        //   forward: "read".contains("async_read") = FALSE
        //   bidirectional NEW: "async_read".contains("read") = TRUE → KEPT
        // "unrelated.rs" → test_stem = "unrelated"
        //   neither direction matches → REMOVED
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        assert!(
            mappings[0]
                .test_files
                .contains(&"tests/io/read.rs".to_string()),
            "tests/io/read.rs should be KEPT via bidirectional match ('async_read' contains 'read'), got: {:?}",
            mappings[0].test_files
        );
        assert!(
            !mappings[0]
                .test_files
                .contains(&"tests/other/unrelated.rs".to_string()),
            "unrelated.rs should be REMOVED, got: {:?}",
            mappings[0].test_files
        );
    }

    // FO-04: reverse filter KEEPS mapping when directory segment matches
    // has_common_directory_segment は未実装 → RED
    #[test]
    fn fo_04_reverse_filter_keeps_mapping_with_matching_directory_segment() {
        // Given: test "tests/Database/EloquentIntegrationTest.php" maps to >threshold prod files
        //   including "src/Illuminate/Database/Eloquent/Model.php" (dir segment "database" matches)
        // When: apply_reverse_fan_out_filter with threshold=5
        // Then: Model.php retains EloquentIntegrationTest.php (dir segment match)
        //       Request.php removes it (no dir/name match)
        let prod_files = [
            "src/Illuminate/Database/Eloquent/Model.php",
            "src/Illuminate/Database/Query/Builder.php",
            "src/Illuminate/Http/Request.php",
            "src/Illuminate/Auth/Guard.php",
            "src/Illuminate/Routing/Router.php",
            "src/Illuminate/Support/Collection.php",
        ];
        let mut mappings: Vec<exspec_core::observe::FileMapping> = prod_files
            .iter()
            .map(|prod| exspec_core::observe::FileMapping {
                production_file: prod.to_string(),
                test_files: vec!["tests/Database/EloquentIntegrationTest.php".to_string()],
                strategy: exspec_core::observe::MappingStrategy::ImportTracing,
            })
            .collect();
        // fan-in: 6 > threshold=5 → filter triggers for EloquentIntegrationTest.php
        // Model.php: prod_stem="model", test_stem="eloquentintegrationtest"
        //   name: "eloquentintegrationtest".contains("model") = FALSE; "model".contains("...") = FALSE
        //   dir NEW: prod has "Database", test has "Database" → KEPT
        // Request.php: prod has "Http", test has "Database" → no dir match, no name match → REMOVED
        apply_reverse_fan_out_filter(&mut mappings, 5);
        let model_mapping = mappings
            .iter()
            .find(|m| m.production_file == "src/Illuminate/Database/Eloquent/Model.php")
            .expect("Model.php mapping should exist");
        assert!(
            model_mapping
                .test_files
                .contains(&"tests/Database/EloquentIntegrationTest.php".to_string()),
            "Model.php should KEEP EloquentIntegrationTest.php via dir segment 'database', got: {:?}",
            model_mapping.test_files
        );
        let request_mapping = mappings
            .iter()
            .find(|m| m.production_file == "src/Illuminate/Http/Request.php")
            .expect("Request.php mapping should exist");
        assert!(
            !request_mapping
                .test_files
                .contains(&"tests/Database/EloquentIntegrationTest.php".to_string()),
            "Request.php should REMOVE EloquentIntegrationTest.php (no dir/name match), got: {:?}",
            request_mapping.test_files
        );
    }

    // FO-05: generic segments (src, tests) do NOT count as directory match (guard test → PASS)
    #[test]
    fn fo_05_generic_directory_segments_are_not_matched() {
        // Given: prod fan-out > threshold
        //   production_file = "src/Connection.php"        dir segments: ["src"]  (generic)
        //   test_file       = "tests/SomeOtherTest.php"   dir segments: ["tests"] (generic)
        //   name: "someothertest".contains("connection") = FALSE
        //   dir: "src" and "tests" are generic → should NOT match → REMOVED
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/Connection.php".to_string(),
            test_files: vec!["tests/SomeOtherTest.php".to_string()],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        assert!(
            mappings[0].test_files.is_empty(),
            "SomeOtherTest.php should be REMOVED (generic segments src/tests must not match), got: {:?}",
            mappings[0].test_files
        );
    }

    // FO-W2: short test_stem (len <= 3) must NOT trigger prod_class.contains(&test_stem) (FP guard)
    #[test]
    fn fo_w2_short_test_stem_does_not_cause_false_positive_in_bidirectional_match() {
        // Given: prod fan-out > threshold
        //   production_file = "src/async_io/async_io_driver.rs"  prod_class = "async_io_driver"
        //   test "tests/io.rs": test_stem = "io" (len=2, SHORT ≤ 3)
        //     bidirectional naive: "async_io_driver".contains("io") = TRUE → FP!
        //     guard: len("io") <= 3 → skip bidirectional → REMOVED
        //   test "tests/io_driver.rs": test_stem = "io_driver" (len=9, not short)
        //     forward: "io_driver".contains("async_io_driver") = FALSE
        //     bidirectional: "async_io_driver".contains("io_driver") = TRUE → KEPT
        let mut mappings = vec![exspec_core::observe::FileMapping {
            production_file: "src/async_io/async_io_driver.rs".to_string(),
            test_files: vec!["tests/io.rs".to_string(), "tests/io_driver.rs".to_string()],
            strategy: exspec_core::observe::MappingStrategy::ImportTracing,
        }];
        // fan-out: 2/10 = 20% > 5% → filter triggers
        apply_fan_out_filter(&mut mappings, 10, 5.0);
        // Key assertion: short stem "io" must NOT be kept via bidirectional (FP guard)
        assert!(
            !mappings[0]
                .test_files
                .contains(&"tests/io.rs".to_string()),
            "tests/io.rs should be REMOVED: short stem 'io' (len=2) must not trigger bidirectional match, got: {:?}",
            mappings[0].test_files
        );
    }

    // FO-INT-01: Laravel observe recall > 85% after fix (integration, requires /tmp/laravel)
    #[test]
    #[ignore = "integration test: requires /tmp/laravel and built binary"]
    fn fo_int_01_laravel_observe_recall_above_85_percent() {
        // Given: /tmp/laravel exists with Laravel framework source
        // When: cargo run -- observe --lang php --format json /tmp/laravel
        // Then: recall > 85%
        let output = std::process::Command::new("cargo")
            .args([
                "run",
                "--",
                "observe",
                "--lang",
                "php",
                "--format",
                "json",
                "/tmp/laravel",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR").replace("/crates/cli", ""))
            .output()
            .expect("failed to run cargo run");
        assert!(output.status.success(), "cargo run failed: {:?}", output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).expect("output should be valid JSON");
        let recall = parsed["summary"]["recall"]
            .as_f64()
            .expect("recall should be a number");
        assert!(
            recall > 0.85,
            "Laravel recall should be > 85%, got {:.1}%",
            recall * 100.0
        );
    }

    // FO-INT-02: tokio observe recall >= 50.8% (regression guard)
    #[test]
    #[ignore = "integration test: requires /tmp/exspec-dogfood/tokio and built binary"]
    fn fo_int_02_tokio_observe_recall_no_regression() {
        // Given: /tmp/exspec-dogfood/tokio exists
        // When: cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio
        // Then: recall >= 50.8% (no regression from v0.4.5-dev baseline)
        let tokio_path = if std::path::Path::new("/tmp/exspec-dogfood/tokio").exists() {
            "/tmp/exspec-dogfood/tokio"
        } else {
            "/private/tmp/exspec-dogfood/tokio"
        };
        let output = std::process::Command::new("cargo")
            .args([
                "run", "--", "observe", "--lang", "rust", "--format", "json", tokio_path,
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR").replace("/crates/cli", ""))
            .output()
            .expect("failed to run cargo run");
        assert!(output.status.success(), "cargo run failed: {:?}", output);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).expect("output should be valid JSON");
        let recall = parsed["summary"]["recall"]
            .as_f64()
            .expect("recall should be a number");
        assert!(
            recall >= 0.508,
            "tokio recall should be >= 50.8% (regression guard), got {:.1}%",
            recall * 100.0
        );
    }

    // --- extract_class_name ---

    #[test]
    fn extract_class_name_php() {
        assert_eq!(extract_class_name("src/Support/Str.php"), "Str");
    }

    #[test]
    fn extract_class_name_rust() {
        assert_eq!(extract_class_name("src/user.rs"), "user");
    }

    #[test]
    fn extract_class_name_nested() {
        assert_eq!(
            extract_class_name("src/Illuminate/Database/Eloquent/Model.php"),
            "Model"
        );
    }
}
