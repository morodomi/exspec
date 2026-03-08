use crate::rules::RuleId;

#[derive(Debug, Clone, Default)]
pub struct TestAnalysis {
    pub assertion_count: usize,
    pub mock_count: usize,
    pub mock_classes: Vec<String>,
    pub line_count: usize,
    pub how_not_what_count: usize,
    pub fixture_count: usize,
    pub has_wait: bool,
    pub assertion_message_count: usize,
    pub duplicate_literal_count: usize,
    pub suppressed_rules: Vec<RuleId>,
}

#[derive(Debug, Clone)]
pub struct TestFunction {
    pub name: String,
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub analysis: TestAnalysis,
}

/// File-level analysis result for rules that operate at file scope (T004-T008).
///
/// Language extractors MUST override `extract_file_analysis()` to provide
/// accurate `has_pbt_import`, `has_contract_import`, `has_error_test`,
/// and `parameterized_count`.
/// The default impl returns false/0 for these fields.
#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub file: String,
    pub functions: Vec<TestFunction>,
    pub has_pbt_import: bool,
    pub has_contract_import: bool,
    pub has_error_test: bool,
    pub has_relational_assertion: bool,
    pub parameterized_count: usize,
}

pub trait LanguageExtractor {
    fn extract_test_functions(&self, source: &str, file_path: &str) -> Vec<TestFunction>;

    /// Extract file-level analysis including imports and parameterized test counts.
    /// Default impl delegates to `extract_test_functions` with file-level fields as false/0.
    /// Language extractors MUST override this to provide accurate detection.
    fn extract_file_analysis(&self, source: &str, file_path: &str) -> FileAnalysis {
        let functions = self.extract_test_functions(source, file_path);
        FileAnalysis {
            file: file_path.to_string(),
            functions,
            has_pbt_import: false,
            has_contract_import: false,
            has_error_test: false,
            has_relational_assertion: false,
            parameterized_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_default_all_zero_or_empty() {
        let analysis = TestAnalysis::default();
        assert_eq!(analysis.assertion_count, 0);
        assert_eq!(analysis.mock_count, 0);
        assert!(analysis.mock_classes.is_empty());
        assert_eq!(analysis.line_count, 0);
        assert_eq!(analysis.how_not_what_count, 0);
        assert_eq!(analysis.fixture_count, 0);
        assert!(!analysis.has_wait);
        assert_eq!(analysis.assertion_message_count, 0);
        assert_eq!(analysis.duplicate_literal_count, 0);
        assert!(analysis.suppressed_rules.is_empty());
    }

    #[test]
    fn file_analysis_fields_accessible() {
        let fa = FileAnalysis {
            file: "test.py".to_string(),
            functions: vec![],
            has_pbt_import: true,
            has_contract_import: false,
            has_error_test: true,
            has_relational_assertion: false,
            parameterized_count: 3,
        };
        assert_eq!(fa.file, "test.py");
        assert!(fa.functions.is_empty());
        assert!(fa.has_pbt_import);
        assert!(!fa.has_contract_import);
        assert!(fa.has_error_test);
        assert!(!fa.has_relational_assertion);
        assert_eq!(fa.parameterized_count, 3);
    }

    struct DummyExtractor;
    impl LanguageExtractor for DummyExtractor {
        fn extract_test_functions(&self, _source: &str, file_path: &str) -> Vec<TestFunction> {
            vec![TestFunction {
                name: "test_dummy".to_string(),
                file: file_path.to_string(),
                line: 1,
                end_line: 3,
                analysis: TestAnalysis::default(),
            }]
        }
    }

    #[test]
    fn default_extract_file_analysis_delegates_to_extract_test_functions() {
        let extractor = DummyExtractor;
        let fa = extractor.extract_file_analysis("x = 1", "test.py");
        assert_eq!(fa.functions.len(), 1);
        assert!(!fa.has_pbt_import);
        assert!(!fa.has_contract_import);
        assert!(!fa.has_error_test);
        assert!(!fa.has_relational_assertion);
        assert_eq!(fa.parameterized_count, 0);
    }
}
