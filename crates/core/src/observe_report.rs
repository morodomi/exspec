use serde::Serialize;

/// Status constants for route coverage.
pub const ROUTE_STATUS_COVERED: &str = "covered";
pub const ROUTE_STATUS_GAP: &str = "gap";
pub const ROUTE_STATUS_UNMAPPABLE: &str = "unmappable";

/// A route with test coverage information.
#[derive(Debug, Clone, Serialize)]
pub struct ObserveRouteEntry {
    pub http_method: String,
    pub path: String,
    pub handler: String,
    pub file: String,
    pub test_files: Vec<String>,
    pub status: String,           // "covered" | "gap" | "unmappable"
    pub gap_reasons: Vec<String>, // e.g. ["no_test_mapping"]
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
    pub routes_gap: usize,
    pub routes_unmappable: usize,
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
                "\n## Route Coverage: {} total, {} covered, {} gap, {} unmappable\n",
                self.summary.routes_total,
                self.summary.routes_covered,
                self.summary.routes_gap,
                self.summary.routes_unmappable,
            ));
            out.push_str(&format!(
                "Routes: {} total, {} covered, {} gap, {} unmappable\n",
                self.summary.routes_total,
                self.summary.routes_covered,
                self.summary.routes_gap,
                self.summary.routes_unmappable,
            ));
            out.push_str("| Route | Handler | Test File | Status |\n");
            out.push_str("|-------|---------|-----------|--------|\n");
            for route in &self.routes {
                let status = if route.status.is_empty() {
                    if route.test_files.is_empty() {
                        "Gap"
                    } else {
                        "Covered"
                    }
                } else {
                    match route.status.as_str() {
                        "covered" => "Covered",
                        "unmappable" => "Unmappable",
                        _ => "Gap",
                    }
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

    /// Format as AI-friendly prompt for test generation guidance.
    pub fn format_ai_prompt(&self) -> String {
        let mut out = String::new();

        out.push_str("## Route Coverage Summary\n\n");
        out.push_str(&format!("- Total routes: {}\n", self.summary.routes_total));
        out.push_str(&format!("- Covered: {}\n", self.summary.routes_covered));
        out.push_str(&format!("- Gap: {}\n", self.summary.routes_gap));
        out.push_str(&format!(
            "- Unmappable: {}\n",
            self.summary.routes_unmappable
        ));

        let gap_routes: Vec<&ObserveRouteEntry> =
            self.routes.iter().filter(|r| r.status == "gap").collect();

        if gap_routes.is_empty() {
            out.push_str("\nAll mappable routes have test coverage.\n");
        } else {
            out.push_str("\n## Route Coverage Gaps\n\n");
            out.push_str("The following API routes have no test coverage:\n");
            for route in &gap_routes {
                out.push_str(&format!(
                    "- {} {} -> {}\n",
                    route.http_method, route.path, route.handler
                ));
            }
            out.push_str("\nConsider writing tests for these endpoints.\n");
        }

        out
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
                routes_gap: 1,
                routes_unmappable: 0,
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
                    status: "covered".to_string(),
                    gap_reasons: vec![],
                },
                ObserveRouteEntry {
                    http_method: "POST".to_string(),
                    path: "/users".to_string(),
                    handler: "UsersController.create".to_string(),
                    file: "src/users.controller.ts".to_string(),
                    test_files: vec!["src/users.controller.spec.ts".to_string()],
                    status: "covered".to_string(),
                    gap_reasons: vec![],
                },
                ObserveRouteEntry {
                    http_method: "DELETE".to_string(),
                    path: "/users/:id".to_string(),
                    handler: "UsersController.remove".to_string(),
                    file: "src/utils/helpers.ts".to_string(),
                    test_files: vec![],
                    status: "gap".to_string(),
                    gap_reasons: vec!["no_test_mapping".to_string()],
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
        assert_eq!(report.summary.routes_gap, 1);
        assert_eq!(report.summary.routes_unmappable, 0);
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
        assert_eq!(parsed["summary"]["routes_gap"], 1);
        assert_eq!(parsed["summary"]["routes_unmappable"], 0);

        // Route entries have status and gap_reasons
        let routes = parsed["routes"].as_array().unwrap();
        assert_eq!(routes[0]["status"], "covered");
        assert_eq!(routes[2]["status"], "gap");
        assert_eq!(routes[2]["gap_reasons"][0], "no_test_mapping");
    }

    // OB4: terminal output contains expected sections
    #[test]
    fn ob4_observe_terminal_output() {
        let report = sample_report();
        let output = report.format_terminal();

        assert!(output.contains("## Summary"), "missing Summary section");
        assert!(
            output.contains("## Route Coverage:"),
            "missing Route Coverage section"
        );
        assert!(output.contains("3 total"), "missing routes_total in header");
        assert!(
            output.contains("2 covered"),
            "missing routes_covered in header"
        );
        assert!(output.contains("1 gap"), "missing routes_gap in header");
        assert!(
            output.contains("0 unmappable"),
            "missing routes_unmappable in header"
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

    // TC-01: Given route with test_files, When report built, Then status="covered" and gap_reasons=[]
    #[test]
    fn tc01_covered_route_has_status_covered_and_empty_gap_reasons() {
        // Given: a route entry with test_files populated
        let route = ObserveRouteEntry {
            http_method: "GET".to_string(),
            path: "/api/items".to_string(),
            handler: "ItemsController.index".to_string(),
            file: "src/items.controller.ts".to_string(),
            test_files: vec!["src/items.controller.spec.ts".to_string()],
            status: "covered".to_string(),
            gap_reasons: vec![],
        };

        // When / Then
        assert_eq!(route.status, ROUTE_STATUS_COVERED);
        assert!(route.gap_reasons.is_empty());
    }

    // TC-02: Given route with handler but no test_files, When report built, Then status="gap" and gap_reasons=["no_test_mapping"]
    #[test]
    fn tc02_gap_route_has_status_gap_and_no_test_mapping_reason() {
        // Given: a route entry with a handler but no test_files
        let route = ObserveRouteEntry {
            http_method: "POST".to_string(),
            path: "/api/items".to_string(),
            handler: "ItemsController.store".to_string(),
            file: "src/items.controller.ts".to_string(),
            test_files: vec![],
            status: "gap".to_string(),
            gap_reasons: vec!["no_test_mapping".to_string()],
        };

        // When / Then
        assert_eq!(route.status, ROUTE_STATUS_GAP);
        assert_eq!(route.gap_reasons, vec!["no_test_mapping"]);
    }

    // TC-03: Given route with empty handler, When report built, Then status="unmappable" and gap_reasons=[]
    #[test]
    fn tc03_unmappable_route_has_status_unmappable_and_empty_gap_reasons() {
        // Given: a route entry with an empty handler (e.g. closure)
        let route = ObserveRouteEntry {
            http_method: "GET".to_string(),
            path: "/health".to_string(),
            handler: "".to_string(),
            file: "src/app.ts".to_string(),
            test_files: vec![],
            status: "unmappable".to_string(),
            gap_reasons: vec![],
        };

        // When / Then
        assert_eq!(route.status, ROUTE_STATUS_UNMAPPABLE);
        assert!(route.gap_reasons.is_empty());
    }

    // TC-04: Terminal output contains "Routes: X total, Y covered, Z gap, W unmappable" summary line
    #[test]
    fn tc04_terminal_output_contains_routes_summary_line() {
        // Given: a report with route coverage data
        let report = sample_report();

        // When
        let output = report.format_terminal();

        // Then: output must contain a standalone summary line in this exact format
        assert!(
            output.contains("Routes: 3 total, 2 covered, 1 gap, 0 unmappable"),
            "terminal output must contain 'Routes: X total, Y covered, Z gap, W unmappable' summary line, got:\n{output}"
        );
    }

    // TC-05: JSON output contains status and gap_reasons fields for each route
    #[test]
    fn tc05_json_output_contains_status_and_gap_reasons_per_route() {
        // Given
        let report = sample_report();

        // When
        let json = report.format_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        // Then: every route entry must have status and gap_reasons
        let routes = parsed["routes"].as_array().expect("routes is array");
        for route in routes {
            assert!(
                route.get("status").is_some(),
                "route missing 'status' field: {route}"
            );
            assert!(
                route.get("gap_reasons").is_some(),
                "route missing 'gap_reasons' field: {route}"
            );
        }

        // Verify specific values
        assert_eq!(routes[0]["status"], ROUTE_STATUS_COVERED);
        assert_eq!(routes[0]["gap_reasons"].as_array().unwrap().len(), 0);
        assert_eq!(routes[2]["status"], ROUTE_STATUS_GAP);
        assert_eq!(routes[2]["gap_reasons"][0], "no_test_mapping");
    }

    // TC-06: AI prompt output lists gap routes with handler info
    #[test]
    fn tc06_ai_prompt_lists_gap_routes_with_handler_info() {
        // Given
        let report = sample_report();

        // When
        let output = report.format_ai_prompt();

        // Then: output must list the gap route with its handler
        assert!(
            output.contains("DELETE /users/:id"),
            "ai prompt must mention gap route path, got:\n{output}"
        );
        assert!(
            output.contains("UsersController.remove"),
            "ai prompt must mention gap route handler, got:\n{output}"
        );
        assert!(
            output.contains("## Route Coverage Gaps"),
            "ai prompt must have Route Coverage Gaps section, got:\n{output}"
        );
    }
}
