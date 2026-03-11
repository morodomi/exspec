# Cycle: --min-severity display filter (#59)

## Context
INFO diagnostics are 70-90% of output in dogfooding. Users need `--min-severity` to filter display without changing rule behavior or exit code.

## Scope
- `Severity::from_str` case-insensitive
- `Config.min_severity` field (default: Info)
- `OutputConfig` in ExspecConfig for `[output]` TOML section
- `filter_by_severity()` in output.rs
- `SummaryStats` for JSON unfiltered summary
- `--min-severity` CLI flag in main.rs
- Terminal/JSON/SARIF all respect filter

## Design Decisions
1. Filter in main.rs after evaluation, before formatting (centralized)
2. Terminal: score + diagnostics both filtered
3. JSON/SARIF: diagnostics filtered, JSON summary unfiltered
4. Exit code: uses unfiltered diagnostics (unchanged)
5. CLI overrides config file

## Test List
- [x] 1a-1e: Severity::from_str case insensitivity
- [x] 2a-2d: Config [output] parsing
- [x] 3a-3d: filter_by_severity
- [x] 4a: SummaryStats::from_diagnostics
- [x] 5a: Terminal score reflects filtered counts
- [x] 6a: JSON filtered array + unfiltered summary
- [x] 7a-7b: Exit code unaffected by filter
- [x] 8a-8b: CLI integration

## Phase Log
- KICKOFF: 2026-03-11
- RED: Tests written
- GREEN: Implementation complete
- REFACTOR: no significant refactoring needed
- REVIEW: PASS (security 5, correctness 42). Fixed: eprintln for invalid config, added fallback test, renamed parameter
- COMMIT: done
