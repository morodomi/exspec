use serde::Serialize;

use crate::extractor::FileAnalysis;

/// Project-wide metrics computed from all file analyses.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectMetrics {
    pub mock_density_avg: f64,
    pub mock_class_max: usize,
    pub parameterized_ratio: f64,
    pub pbt_ratio: f64,
    pub assertion_density_avg: f64,
    pub contract_coverage: f64,
    pub test_source_ratio: f64,
}

pub fn compute_metrics(analyses: &[FileAnalysis], source_file_count: usize) -> ProjectMetrics {
    let total_files = analyses.len();
    let total_functions: usize = analyses.iter().map(|a| a.functions.len()).sum();

    if total_functions == 0 {
        let pbt_ratio = if total_files > 0 {
            analyses.iter().filter(|a| a.has_pbt_import).count() as f64 / total_files as f64
        } else {
            0.0
        };
        let contract_coverage = if total_files > 0 {
            analyses.iter().filter(|a| a.has_contract_import).count() as f64 / total_files as f64
        } else {
            0.0
        };
        let test_source_ratio = if source_file_count > 0 {
            total_files as f64 / source_file_count as f64
        } else {
            0.0
        };
        return ProjectMetrics {
            pbt_ratio,
            contract_coverage,
            test_source_ratio,
            ..Default::default()
        };
    }

    let all_funcs: Vec<_> = analyses.iter().flat_map(|a| &a.functions).collect();

    let mock_density_avg = all_funcs
        .iter()
        .map(|f| f.analysis.mock_count)
        .sum::<usize>() as f64
        / total_functions as f64;
    let mock_class_max = all_funcs
        .iter()
        .map(|f| f.analysis.mock_classes.len())
        .max()
        .unwrap_or(0);
    let total_parameterized: usize = analyses.iter().map(|a| a.parameterized_count).sum();
    let parameterized_ratio = total_parameterized as f64 / total_functions as f64;
    let pbt_ratio =
        analyses.iter().filter(|a| a.has_pbt_import).count() as f64 / total_files as f64;
    let assertion_density_avg = all_funcs
        .iter()
        .map(|f| f.analysis.assertion_count)
        .sum::<usize>() as f64
        / total_functions as f64;
    let contract_coverage =
        analyses.iter().filter(|a| a.has_contract_import).count() as f64 / total_files as f64;
    let test_source_ratio = if source_file_count > 0 {
        total_files as f64 / source_file_count as f64
    } else {
        0.0
    };

    ProjectMetrics {
        mock_density_avg,
        mock_class_max,
        parameterized_ratio,
        pbt_ratio,
        assertion_density_avg,
        contract_coverage,
        test_source_ratio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::{TestAnalysis, TestFunction};

    fn make_func(
        name: &str,
        file: &str,
        assertion_count: usize,
        mock_count: usize,
        mock_classes: Vec<String>,
    ) -> TestFunction {
        TestFunction {
            name: name.to_string(),
            file: file.to_string(),
            line: 1,
            end_line: 10,
            analysis: TestAnalysis {
                assertion_count,
                mock_count,
                mock_classes,
                ..Default::default()
            },
        }
    }

    fn make_analysis(
        file: &str,
        functions: Vec<TestFunction>,
        has_pbt: bool,
        has_contract: bool,
        parameterized: usize,
    ) -> FileAnalysis {
        FileAnalysis {
            file: file.to_string(),
            functions,
            has_pbt_import: has_pbt,
            has_contract_import: has_contract,
            has_error_test: false,
            parameterized_count: parameterized,
        }
    }

    #[test]
    fn empty_analyses_returns_all_zeros() {
        let m = compute_metrics(&[], 0);
        assert_eq!(m.mock_density_avg, 0.0);
        assert_eq!(m.mock_class_max, 0);
        assert_eq!(m.parameterized_ratio, 0.0);
        assert_eq!(m.pbt_ratio, 0.0);
        assert_eq!(m.assertion_density_avg, 0.0);
        assert_eq!(m.contract_coverage, 0.0);
        assert_eq!(m.test_source_ratio, 0.0);
    }

    #[test]
    fn single_file_single_function_correct_values() {
        let funcs = vec![make_func("test_a", "test.py", 3, 2, vec!["Db".into()])];
        let analyses = vec![make_analysis("test.py", funcs, true, true, 1)];
        let m = compute_metrics(&analyses, 5);
        assert_eq!(m.mock_density_avg, 2.0);
        assert_eq!(m.mock_class_max, 1);
        assert_eq!(m.parameterized_ratio, 1.0); // 1/1
        assert_eq!(m.pbt_ratio, 1.0); // 1/1
        assert_eq!(m.assertion_density_avg, 3.0);
        assert_eq!(m.contract_coverage, 1.0); // 1/1
        assert!((m.test_source_ratio - 0.2).abs() < f64::EPSILON); // 1/5
    }

    #[test]
    fn multiple_files_proper_aggregation() {
        let funcs1 = vec![
            make_func("test_a", "a.py", 2, 4, vec!["Db".into(), "Api".into()]),
            make_func("test_b", "a.py", 1, 0, vec![]),
        ];
        let funcs2 = vec![make_func("test_c", "b.py", 3, 2, vec!["Cache".into()])];
        let analyses = vec![
            make_analysis("a.py", funcs1, true, false, 1),
            make_analysis("b.py", funcs2, false, true, 0),
        ];
        let m = compute_metrics(&analyses, 4);
        // mock_density_avg: (4+0+2)/3 = 2.0
        assert_eq!(m.mock_density_avg, 2.0);
        // mock_class_max: max(2, 0, 1) = 2
        assert_eq!(m.mock_class_max, 2);
        // parameterized_ratio: (1+0)/3 = 0.333...
        assert!((m.parameterized_ratio - 1.0 / 3.0).abs() < 0.001);
        // pbt_ratio: 1/2 = 0.5
        assert_eq!(m.pbt_ratio, 0.5);
        // assertion_density_avg: (2+1+3)/3 = 2.0
        assert_eq!(m.assertion_density_avg, 2.0);
        // contract_coverage: 1/2 = 0.5
        assert_eq!(m.contract_coverage, 0.5);
        // test_source_ratio: 2/4 = 0.5
        assert_eq!(m.test_source_ratio, 0.5);
    }

    #[test]
    fn zero_source_files_test_source_ratio_zero() {
        let funcs = vec![make_func("test_a", "test.py", 1, 0, vec![])];
        let analyses = vec![make_analysis("test.py", funcs, false, false, 0)];
        let m = compute_metrics(&analyses, 0);
        assert_eq!(m.test_source_ratio, 0.0);
    }

    #[test]
    fn zero_functions_all_ratios_zero() {
        let analyses = vec![make_analysis("test.py", vec![], false, false, 0)];
        let m = compute_metrics(&analyses, 5);
        assert_eq!(m.mock_density_avg, 0.0);
        assert_eq!(m.mock_class_max, 0);
        assert_eq!(m.parameterized_ratio, 0.0);
        assert_eq!(m.assertion_density_avg, 0.0);
        // pbt/contract are file-level
        assert_eq!(m.pbt_ratio, 0.0); // 0/1
        assert_eq!(m.contract_coverage, 0.0); // 0/1
    }

    #[test]
    fn mock_class_max_is_per_function_max() {
        let funcs = vec![
            make_func(
                "test_a",
                "test.py",
                1,
                3,
                vec!["A".into(), "B".into(), "C".into()],
            ),
            make_func("test_b", "test.py", 1, 1, vec!["D".into()]),
        ];
        let analyses = vec![make_analysis("test.py", funcs, false, false, 0)];
        let m = compute_metrics(&analyses, 1);
        assert_eq!(m.mock_class_max, 3); // max(3, 1)
    }

    #[test]
    fn parameterized_ratio_sums_across_files() {
        let funcs1 = vec![
            make_func("test_a", "a.py", 1, 0, vec![]),
            make_func("test_b", "a.py", 1, 0, vec![]),
        ];
        let funcs2 = vec![make_func("test_c", "b.py", 1, 0, vec![])];
        let analyses = vec![
            make_analysis("a.py", funcs1, false, false, 1),
            make_analysis("b.py", funcs2, false, false, 1),
        ];
        let m = compute_metrics(&analyses, 1);
        // (1+1)/3 = 0.666...
        assert!((m.parameterized_ratio - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn pbt_contract_count_files_with_import() {
        let analyses = vec![
            make_analysis(
                "a.py",
                vec![make_func("t", "a.py", 1, 0, vec![])],
                true,
                false,
                0,
            ),
            make_analysis(
                "b.py",
                vec![make_func("t", "b.py", 1, 0, vec![])],
                false,
                true,
                0,
            ),
            make_analysis(
                "c.py",
                vec![make_func("t", "c.py", 1, 0, vec![])],
                true,
                true,
                0,
            ),
        ];
        let m = compute_metrics(&analyses, 1);
        // pbt: 2/3
        assert!((m.pbt_ratio - 2.0 / 3.0).abs() < 0.001);
        // contract: 2/3
        assert!((m.contract_coverage - 2.0 / 3.0).abs() < 0.001);
    }
}
