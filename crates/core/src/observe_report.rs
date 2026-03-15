use serde::Serialize;

/// A route with test coverage information.
#[derive(Debug, Clone, Serialize)]
pub struct ObserveRouteEntry {
    pub http_method: String,
    pub path: String,
    pub handler: String,
    pub file: String,
    pub test_files: Vec<String>,
}

/// A file mapping entry for the report.
#[derive(Debug, Clone, Serialize)]
pub struct ObserveFileEntry {
    pub production_file: String,
    pub test_files: Vec<String>,
    pub strategy: String,
}

/// Summary statistics for observe report.
#[derive(Debug, Clone, Serialize)]
pub struct ObserveSummary {
    pub production_files: usize,
    pub test_files: usize,
    pub mapped_files: usize,
    pub unmapped_files: usize,
    pub routes_total: usize,
    pub routes_covered: usize,
}

/// Full observe report.
#[derive(Debug, Clone, Serialize)]
pub struct ObserveReport {
    pub summary: ObserveSummary,
    pub file_mappings: Vec<ObserveFileEntry>,
    pub routes: Vec<ObserveRouteEntry>,
    pub unmapped_production_files: Vec<String>,
}

impl ObserveReport {
    /// Format as terminal-friendly Markdown.
    pub fn format_terminal(&self) -> String {
        let mut out = String::new();

        out.push_str("# exspec observe -- Test Coverage Map\n\n");

        // Summary
        out.push_str("## Summary\n");
        out.push_str(&format!(
            "- Production files: {}\n",
            self.summary.production_files
        ));
        out.push_str(&format!("- Test files: {}\n", self.summary.test_files));
        let pct = if self.summary.production_files > 0 {
            self.summary.mapped_files as f64 / self.summary.production_files as f64 * 100.0
        } else {
            0.0
        };
        out.push_str(&format!(
            "- Mapped: {} ({:.1}%)\n",
            self.summary.mapped_files, pct
        ));
        out.push_str(&format!("- Unmapped: {}\n", self.summary.unmapped_files));

        // Route Coverage
        if self.summary.routes_total > 0 {
            out.push_str(&format!(
                "\n## Route Coverage ({}/{})\n",
                self.summary.routes_covered, self.summary.routes_total
            ));
            out.push_str("| Route | Handler | Test File | Status |\n");
            out.push_str("|-------|---------|-----------|--------|\n");
            for route in &self.routes {
                let status = if route.test_files.is_empty() {
                    "Gap"
                } else {
                    "Covered"
                };
                let test_display = if route.test_files.is_empty() {
                    "\u{2014}".to_string()
                } else {
                    route.test_files.join(", ")
                };
                out.push_str(&format!(
                    "| {} {} | {} | {} | {} |\n",
                    route.http_method, route.path, route.handler, test_display, status
                ));
            }
        }

        // File Mappings
        if !self.file_mappings.is_empty() {
            out.push_str("\n## File Mappings\n");
            out.push_str("| Production File | Test File(s) | Strategy |\n");
            out.push_str("|----------------|-------------|----------|\n");
            for entry in &self.file_mappings {
                let tests = if entry.test_files.is_empty() {
                    "\u{2014}".to_string()
                } else {
                    entry.test_files.join(", ")
                };
                out.push_str(&format!(
                    "| {} | {} | {} |\n",
                    entry.production_file, tests, entry.strategy
                ));
            }
        }

        // Unmapped
        if !self.unmapped_production_files.is_empty() {
            out.push_str("\n## Unmapped Production Files\n");
            for f in &self.unmapped_production_files {
                out.push_str(&format!("- {f}\n"));
            }
        }

        out
    }

    /// Format as JSON.
    pub fn format_json(&self) -> String {
        #[derive(Serialize)]
        struct JsonOutput<'a> {
            version: &'a str,
            mode: &'a str,
            summary: &'a ObserveSummary,
            file_mappings: &'a [ObserveFileEntry],
            routes: &'a [ObserveRouteEntry],
            unmapped_production_files: &'a [String],
        }

        let output = JsonOutput {
            version: env!("CARGO_PKG_VERSION"),
            mode: "observe",
            summary: &self.summary,
            file_mappings: &self.file_mappings,
            routes: &self.routes,
            unmapped_production_files: &self.unmapped_production_files,
        };

        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> ObserveReport {
        ObserveReport {
            summary: ObserveSummary {
                production_files: 3,
                test_files: 2,
                mapped_files: 2,
                unmapped_files: 1,
                routes_total: 3,
                routes_covered: 2,
            },
            file_mappings: vec![
                ObserveFileEntry {
                    production_file: "src/users.controller.ts".to_string(),
                    test_files: vec!["src/users.controller.spec.ts".to_string()],
                    strategy: "import".to_string(),
                },
                ObserveFileEntry {
                    production_file: "src/users.service.ts".to_string(),
                    test_files: vec!["src/users.service.spec.ts".to_string()],
                    strategy: "filename".to_string(),
                },
            ],
            routes: vec![
                ObserveRouteEntry {
                    http_method: "GET".to_string(),
                    path: "/users".to_string(),
                    handler: "UsersController.findAll".to_string(),
                    file: "src/users.controller.ts".to_string(),
                    test_files: vec!["src/users.controller.spec.ts".to_string()],
                },
                ObserveRouteEntry {
                    http_method: "POST".to_string(),
                    path: "/users".to_string(),
                    handler: "UsersController.create".to_string(),
                    file: "src/users.controller.ts".to_string(),
                    test_files: vec!["src/users.controller.spec.ts".to_string()],
                },
                ObserveRouteEntry {
                    http_method: "DELETE".to_string(),
                    path: "/users/:id".to_string(),
                    handler: "UsersController.remove".to_string(),
                    file: "src/utils/helpers.ts".to_string(),
                    test_files: vec![],
                },
            ],
            unmapped_production_files: vec!["src/utils/helpers.ts".to_string()],
        }
    }

    // OB2: summary counts are accurate
    #[test]
    fn ob2_observe_report_summary() {
        let report = sample_report();
        assert_eq!(report.summary.production_files, 3);
        assert_eq!(report.summary.test_files, 2);
        assert_eq!(report.summary.mapped_files, 2);
        assert_eq!(report.summary.unmapped_files, 1);
        assert_eq!(report.summary.routes_total, 3);
        assert_eq!(report.summary.routes_covered, 2);
    }

    // OB3: JSON output is valid and has required fields
    #[test]
    fn ob3_observe_json_output() {
        let report = sample_report();
        let json = report.format_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        assert_eq!(parsed["mode"], "observe");
        assert!(parsed["version"].is_string());
        assert!(parsed["summary"].is_object());
        assert!(parsed["file_mappings"].is_array());
        assert!(parsed["routes"].is_array());
        assert!(parsed["unmapped_production_files"].is_array());

        assert_eq!(parsed["summary"]["production_files"], 3);
        assert_eq!(parsed["summary"]["routes_covered"], 2);
    }

    // OB4: terminal output contains expected sections
    #[test]
    fn ob4_observe_terminal_output() {
        let report = sample_report();
        let output = report.format_terminal();

        assert!(output.contains("## Summary"), "missing Summary section");
        assert!(
            output.contains("## Route Coverage"),
            "missing Route Coverage section"
        );
        assert!(
            output.contains("## File Mappings"),
            "missing File Mappings section"
        );
        assert!(
            output.contains("## Unmapped Production Files"),
            "missing Unmapped section"
        );
    }

    // OB5: covered route shows "Covered"
    #[test]
    fn ob5_route_coverage_covered() {
        let report = sample_report();
        let output = report.format_terminal();
        // GET /users route has test files -> Covered
        assert!(output.contains("| GET /users | UsersController.findAll |"));
        assert!(output.contains("| Covered |"));
    }

    // OB6: gap route shows "Gap"
    #[test]
    fn ob6_route_coverage_gap() {
        let report = sample_report();
        let output = report.format_terminal();
        // DELETE /users/:id has no test files -> Gap
        assert!(output.contains("| DELETE /users/:id |"));
        assert!(output.contains("| Gap |"));
    }

    // OB7: unmapped files are listed
    #[test]
    fn ob7_unmapped_files_listed() {
        let report = sample_report();
        let output = report.format_terminal();
        assert!(output.contains("- src/utils/helpers.ts"));
    }
}
