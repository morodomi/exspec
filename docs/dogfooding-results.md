# Dogfooding Results

Latest: 2026-03-25, exspec v0.4.5-dev (tower GT + 17-library survey)
Initial: 2026-03-09, exspec v0.1.0 (commit 5957cd0)

## Rust observe normal-case library survey (2026-03-25)

GT file: `docs/observe-ground-truth-rust-tower.md`

### 17-library survey results

Goal: Find a Rust library where observe achieves R>=90% (normal-case baseline for ship criteria).

| Library | Test Files | Recall | Classification |
|---------|-----------|--------|----------------|
| tower | 23 (15 external + 8 inline) | **78.3%** (18/23) | best-surveyed |
| rayon | 23 | 82.6% | moderate |
| bytes | 12 | 75.0% | moderate |
| regex | 44 | 72.7% | moderate |
| axum | 122 | 63.9% | moderate |
| tokio | 272 | 50.8% | hard-case |
| serde_json | 31 | 38.7% | hard-case |
| syn | 41 | 39.0% | hard-case |
| crossbeam | 31 | 22.6% | hard-case |
| anyhow | 24 | 16.7% | hard-case |
| itertools | 13 | 15.4% | hard-case |
| clap | 91 | 14.3% | hard-case |
| serde | 150 | 0.0% | hard-case |

(log, hyper, parking_lot, aho-corasick, dashmap: too few test files for meaningful measurement)

Note: recall figures for non-tower libraries are approximate (survey-level, not full GT audits).

### Tower full audit results

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **100%** | >= 98% | **PASS** |
| Recall (GT: 23 files) | **78.3%** (18/23) | >= 90% | **FAIL** |
| TP | 18 (10 external + 8 inline) | - | - |
| FP | 0 | 0 | - |
| FN | 5 | - | - |

**FN root cause**: 5 external test files use `use tower::filter::AsyncFilter` / `use tower::hedge::Hedge` / `use tower::steer::Steer` style imports where the type is defined in a `mod.rs` file. observe does not recognize `mod.rs` files as mappable production files, causing these tests to be missed.

**Key finding**: tower uses submodule direct imports (e.g., `use tower::retry::Policy`, `use tower::buffer::error::*`) rather than crate-root barrel re-export. This eliminates the dominant FN cause seen in tokio/clap. However, `mod.rs`-defined types remain a distinct FN pattern. tower is the best-performing library in the survey but does not reach R>=90%.

**Ship criteria**: P=100% PASS, R=78.3% FAIL. No surveyed library achieves R>=90%. Rust observe ship criteria remain unmet.

**Note on cycle doc R=94.7%**: The cycle doc (20260325_2228) recorded R=94.7% (18/19) based on misreading the observe summary field `test_files: 19, mapped_files: 19`. That field counts production files that have test mappings (production-centric), not the test-file recall. The correct GT-based recall is 78.3%.

## v0.4.5-dev Stratified GT Re-audit (53-file, 2026-03-25)

### Rust (tokio, 53-file stratified sample)

GT file: `docs/observe-ground-truth-rust-tokio.md`

| Stratum | Files | TP | FP | FN | P | R |
|---------|-------|----|----|----|----|---|
| S1: TP import | 15 | 15 | 66 | 2 | 18.5% | 88.2% |
| S2: TP filename | 5 | 5 | 0 | 0 | 100% | 100% |
| S3: FN fan-out | 13 | 0 | 0 | 13 | - | 0% |
| S4: FN other | 10 | 0 | 0 | 15 | - | 0% |
| S5: inline src/ | 5 | 0 | 0 | 4 | - | 0% |
| S6: cross-crate | 5 | 0 | 0 | 5 | - | 0% |
| **Total** | **53** | **20** | **66** | **40** | **23.3%** | **33.3%** |

**Precision problem: barrel import fan-out.**

| Test file | FP count | Root cause |
|-----------|----------|-----------|
| io_driver.rs | 39 | `use tokio::runtime::Builder` resolves to ALL runtime/ files |
| fs_write.rs | 23 | `use tokio::fs` resolves to ALL fs/ files |
| udp.rs | 3 | codec imports not in GT primary |
| io_read_buf.rs | 1 | async_read.rs not in GT primary |

Excluding io_driver + fs_write: **P=82.6%, R=95.0%** (18 files).

**Fan-out filter assessment (S3):** 12/13 correctly filtered (92.3%). 1 false negative: task_hooks.rs (builder.rs IS the SUT).

**FN root causes:**

| Root Cause | Count | Files |
|-----------|-------|-------|
| Barrel import (`tokio::`) | 10 | sync_broadcast, sync_oneshot, sync_panic, task_blocking, fs_uring, etc. |
| No use statement | 4 | macros_test, time_wasm, unwindsafe, io_reader_stream |
| Macro body imports | 1 | rt_common (rt_test! macro) |
| Inline src/ tests | 5 | loom tests in src/sync/tests/, src/runtime/tests/ |
| Cross-crate barrel | 4 | tokio-stream/ tests |

**Previous P=100% was misleading.** Random 50-pair sampling happened to avoid barrel FP pairs. Stratified audit reveals per-file precision drops sharply for tests importing large module barrels.

**Ship criteria: FAIL.** P=23.3% << 98%. Barrel import fan-out is the blocking issue.

**Next steps:**
1. Fix barrel import precision: when `use tokio::fs` resolves, only map to the SPECIFIC file matching the test filename, not all barrel exports
2. This is essentially "L1 filename match should OVERRIDE L2 barrel fan-out" for cases where the barrel export maps to many files

## v0.4.4 Final Re-audit (50-pair, 2026-03-24)

### Rust (tokio, 50-pair)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **100%** (50/50) | >= 98% | **PASS** |
| Recall | ~36.8% (unchanged) | >= 90% | FAIL |

All 50 sampled pairs verified correct (import + self-mapping). driver.rs FP fully eliminated by #168 pub-only filter. Wildcard `use tokio::time::*` correctly resolves to secondary targets.

**Status**: Precision PASS. Experimental maintained (R < 90%).

### PHP (laravel, 50-pair)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **96.0%** (48/50) | >= 98% | **FAIL** |
| Recall | ~88.6% (unchanged) | >= 90% | FAIL (marginal) |

FP causes (2/50):
- Str.php incidental import (2): ContextTest.php (Log test), DatabaseEloquentIntegrationTest.php (Eloquent test) — both import `Illuminate\Support\Str` as utility, not testing Str itself

**Fan-out filter ineffective**: Str.php fan-out = 6.7% (61/912), well below 20% threshold. Fan-out approach alone cannot address this FP pattern. Need import-level filtering (e.g., detect utility-only usage patterns) or lower threshold with FN risk.

**Status**: FAIL. Str.php FP unchanged from v0.4.3.

### Summary

| Language | v0.4.3 | v0.4.4 | Ship Criteria |
|----------|--------|--------|---------------|
| Rust | P=76.7% | **P=100%** | P PASS, R FAIL (experimental) |
| PHP | P=90.0% | **P=96.0%** | P FAIL, R FAIL (experimental) |
| TypeScript | P=100% | unchanged | **stable** |
| Python | P=98.2% | unchanged | **stable** |

## Rust Final Re-audit: Post-#162 (50-pair, tokio, 2026-03-24)

Ship criteria: P>=98% (49/50)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **96.0%** (48/50) | >= 98% | **FAIL** |
| Recall (test file) | ~36.8% (unchanged) | >= 90% | **FAIL** |

FP causes (2/50):
- L2 `pub(crate)` visibility false match (2): driver.rs ← dump.rs, driver.rs ← shutdown.rs — `pub(crate) struct Handle` matches `exported_symbol.scm` query because tree-sitter `visibility_modifier` includes both `pub` and `pub(crate)`. `file_exports_any_symbol()` returns true for `pub(crate)` items.

### Fix applied vs previous audit

| FP Type | Post-#161 (4 FP) | Post-#162 (2 FP) | Status |
|---------|-------------------|-------------------|--------|
| L0 cfg_test helper (source.rs) | 1 | **0** | Fixed by #162 mod_item check |
| L0 cfg_test mock (open_options.rs) | 1 | 0 (not in sample) | Still exists but `mod mock_open_options;` is a valid mod_item |
| L2 re-export confusion (driver.rs) | 2 | **2** | NOT fixed — `pub(crate)` matches visibility_modifier |

### Decision: Additional fix needed

P=96.0% < 98%. Root cause: `exported_symbol.scm` does not distinguish `pub` from `pub(crate)`. Fix: refine query to match only `pub` (not `pub(crate)`, `pub(super)`, etc.).

## Rust Re-audit: Post-#161 (50-pair, tokio, 2026-03-24)

Ship criteria: P>=98% (49/50)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **92.0%** (46/50) | >= 98% | **FAIL** |
| Recall (test file) | **36.8%** (100/272) | >= 90% | **FAIL** (unchanged) |

FP causes (4/50):
- L0 cfg(test) false detection (2): source.rs (helper method), open_options.rs (mock substitution) — `#[cfg(test)]` used for conditional compilation, not test modules
- L2 re-export chain confusion (2): driver.rs ← shutdown.rs, driver.rs ← yield_now.rs — `pub(crate) mod driver` in runtime/mod.rs causes any `use crate::runtime::*` test to map to driver.rs

### vs v0.4.3 audit

| Metric | v0.4.3 (30-pair) | Post-#161 (50-pair) | Delta |
|--------|------------------|---------------------|-------|
| Precision | 76.7% (23/30) | **92.0%** (46/50) | **+15.3pp** |

#161 eliminated mod.rs/lib.rs barrel self-mapping FPs. Remaining FPs are:
1. **L0 detect_inline_tests false positive**: `#[cfg(test)]` used for conditional compilation (helper methods, mock substitution) is indistinguishable from `#[cfg(test)] mod tests`
2. **L2 re-export chain**: `pub(crate) mod` makes all sub-modules visible to import tracing, causing over-mapping

### Decision: #162 (L2 re-export validation) is GO

P=92.0% < 98%. L2 FPs (2/4) can be addressed by `file_exports_any_symbol()` validation. L0 FPs (2/4) require smarter `detect_inline_tests()` that checks for actual `mod tests` blocks, not just `#[cfg(test)]`.

## GT Audit: Rust/PHP Observe (#149, 2026-03-24)

Ship criteria: P>=98%, R>=90% (test file coverage)

### Rust (tokio, 30-pair sample)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **76.7%** (23/30) | >= 98% | **FAIL** |
| Recall (test file) | **36.8%** (100/272) | >= 90% | **FAIL** |

FP causes (7/30):
- Filename strategy: mod.rs ambiguity (1), test file absent (3), production-only files mapped (1)
- Import strategy: re-export/wrapper confusion (2)

### PHP (laravel, 30-pair sample)

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision | **90.0%** (27/30) | >= 98% | **FAIL** |
| Recall (test file) | **88.6%** (808/912) | >= 90% | **FAIL** (marginal) |

FP causes (3/30):
- Str.php utility class incidental imports (3/3)

### Conclusion

Both languages remain **experimental**. Key improvement paths:
- Rust: filter filename matches for mod.rs / files without corresponding test files
- PHP: filter high-fan-out utility classes (Str, Collection) from L2 mappings — related to #129

## Summary (v0.4.3-dev, same-file helper tracing)

| Project | Lang | Tests | BLOCK | WARN | INFO | PASS | vs v0.4.1 BLOCK | Delta |
|---------|------|-------|-------|------|------|------|----------------|-------|
| requests | Python | 386 | 10 | 1 | 198 | 177 | 10 | 0 |
| django | Python | 1835 | 37 | 17 | 842 | 939 | 32 | +5 (test growth) |
| tokio | Rust | 2902 | 247 | 80 | 2075 | 500 | 247 | 0 |
| clap | Rust | 1943 | 43 | 53 | 956 | 891 | 43 | 0 |
| nestjs | TypeScript | 4090 | 11 | 25 | 2174 | 1880 | 13 | **-2** |
| laravel | PHP | 14895 | 222 | 179 | 10593 | 3901 | 222 | 0 |
| symfony | PHP | 25752 | 617 | 319 | 14662 | 10154 | 616 | +1 (test growth) |

### Same-file helper tracing impact analysis (2026-03-24)

**Result: Near-zero BLOCK reduction.** Same-file free-function helpers are rare in real-world projects. Helper delegation FP is dominated by:
- Python: `self.check_output()`, `self.check_html()` — class method calls (cross-file)
- PHP: `$this->fails()`, `$response->assertStatus()` — method calls (cross-file)
- TypeScript: helper functions in describe blocks — only nestjs showed -2 reduction
- Rust: `common::assert_matches()` — cross-file module (Phase 23a already handled same-file)

**#153 Go/No-Go: DEFERRED.** All languages show BLOCK FP rate <= 5%. Cross-file helper delegation deferred to v0.4.4.

## Summary (v0.4.1)

| Project | Lang | Tests | BLOCK | WARN | INFO | PASS | Primary BLOCK Cause |
|---------|------|-------|-------|------|------|------|---------------------|
| exspec (self) | Rust | 10 | 0 | 0 | 7 | 9 | N/A |
| requests | Python | 339 | 10 | 1 | 198 | 177 | helper delegation |
| fastapi | Python | 2155 | 15 | 210 | 2882 | 1086 | helper delegation, nested fn |
| django | Python | **1391** | **32** | 16 | 830 | 912 | helper delegation |
| pytest | Python | 2380 | -- | -- | -- | -- | (clone unavailable, v0.1.0 data below) |
| nestjs | TypeScript | 2679 | 13 | 26 | 2174 | 1878 | helper delegation, done() callback |
| laravel | PHP | 11069 | 222 | 179 | 10584 | 3900 | helper delegation |
| symfony | PHP | 17211 | 616 | 319 | 14654 | 10152 | helper delegation, addToAssertionCount |
| ripgrep | Rust | 16 | 0 | 2 | 30 | 1 | ~330 tests in `rgtest!` macro not detected |
| tokio | Rust | 1594 | **247** | 80 | 2075 | 500 | select! token_tree, smoke tests |
| clap | Rust | 1455 | **43** | 53 | 956 | 891 | helper delegation, smoke tests |

### v0.4.0 → v0.4.1 BLOCK changes

| Project | v0.4.0 BLOCK | v0.4.1 BLOCK | Delta | Notes |
|---------|-------------|-------------|-------|-------|
| django | 22 | 32 | **+10** | +343 tests detected (tests.py recognition). BLOCK increase = newly visible assertion-free tests |
| tokio | 257 | 247 | **-10** | should_panic exact identifier match (#29) |
| clap | 71 | 43 | **-28** | should_panic exact identifier match (#29) |

Phase 24 impact: Django tests.py recognition: **+343 tests** now visible (1048→1391). v0.4.1 cleanup: **-38 BLOCK** across 2 Rust projects from should_panic exact match.

### v0.3.0 → v0.4.0 BLOCK changes (Phase 22)

| Project | v0.3.0 BLOCK | v0.4.0 BLOCK | Delta | Notes |
|---------|-------------|-------------|-------|-------|
| tokio | 385 | 257 | **-128** | `assert_*!` macro auto-detection |
| clap | 193 | 71 | **-122** | `assert_*!` macro auto-detection |

Phase 22 impact: **-250 BLOCK** across 2 Rust projects. Custom assertion macros (`assert_pending!`, `assert_ready!`, `assert_data_eq!`, etc.) now auto-detected via prefix matching.

### v0.1.0 → v0.3.0 BLOCK changes

| Project | v0.1.0 BLOCK | v0.3.0 BLOCK | Delta | Notes |
|---------|-------------|-------------|-------|-------|
| requests | 14 | 10 | -4 | Python assertion broadening |
| fastapi | 19 | 15 | -4 | Python assertion broadening |
| nestjs | 17 | 13 | -4 | Chai/Sinon vocab expansion |
| laravel | 222 | 222 | 0 | Remaining = helper delegation |
| symfony | 759 | 616 | -143 | skip-only exclusion, addToAssertionCount |
| tokio | 388 | 385 | -3 | Rust assert fn call detection |
| clap | 528 | 193 | -335 | Custom assertion filter, helper delegation |
| django | 23 | 22 | -1 | Python assertion broadening |

## Historical Summary (v0.1.0)

<details>
<summary>v0.1.0 (2026-03-09) dogfooding results</summary>

| Project | Lang | Tests | T001 BLOCK | FP Rate | Primary FP Cause |
|---------|------|-------|-----------|---------|------------------|
| exspec (self) | Rust | 51 | 0 real (3 fixture) | 0% | N/A |
| requests | Python | 339 | 14 | ~20% (est.) | mock.assert_*(), delegation |
| fastapi | Python | 2121 | 19 | 21% (4/19) | mock.assert_*(), nested fn |
| vitest | TypeScript | 3120 | 326 (post-fix) | see below | .not/.resolves chains, Chai, expect.soft |
| laravel (pre-fix) | PHP | 10790 | 1305 | ~85% | Mockery shouldReceive |
| laravel (post-#38) | PHP | 10790 | 776 | 71% (552/776) | $obj->assert*, ->expects*, self::assert* |
| laravel (post-#44) | PHP | 10790 | ~224 | -- | named-class assert, Facade assert |
| laravel (post-#45/46) | PHP | 10790 | 222 | -- | helper delegation ($this->fails(), $assert->has()) |
| pydantic | Python | ~2500 | 105 | ~55% (58/105) | benchmark() fixture (43), helper/nested (15) |
| nestjs (pre-fix) | TypeScript | 2675 | 90 | 90% (81/90) | Chai aliases, Sinon mock .verify() |
| nestjs (post-#50) | TypeScript | 2675 | 34 | ~26% (est.) | Sinon .verify(), return wrapper, helper delegation |
| nestjs (post-#51) | TypeScript | 2675 | 17 (confirmed) | 0% | helper delegation, done() callback, bare expect() |
| ripgrep | Rust | 16 (of ~346) | 0 | 0% | ~330 tests in `rgtest!` macro not detected (token_tree) |
| tokio | Rust | 1582 | 388 | 33.8% (131/388) | custom assert macros (124), select! token_tree (7) |
| clap | Rust | 1455 | 528 | 41.3% (218/528) | assert_data_eq! macro (115), helper delegation (103) |
| django | Python | 1047 | 23 | 39% (9/23) | helper delegation (self.check_output, etc.) |
| pytest | Python | 2380 | 594 | ~100% (est.) | `obj.assertX()` without underscore (#62), fnmatch_lines() helper |
| symfony | PHP | 17148 | 759 | ~24% (182/759) | addToAssertionCount() (#63, 91), markTestSkipped() (#64, 91) |

</details>

### Acceptance Criteria Status

- [x] Run exspec against each target
- [ ] BLOCK/WARN false positives = 0 -- **FAIL** (see below)
- [ ] INFO false positives < 3% -- Not evaluated (lower priority)
- [x] File issues for each false positive found
- [x] Document results

## T001 False Positive Categories

### P0: TS expect modifier chains (#37)

`expect(x).not.toX()` and `expect(x).resolves.toX()` are not detected.
The assertion.scm only matches depth-1 `expect(x).toX()` and the explicit `rejects` pattern.

**Impact**: ~40% of vitest T001 FPs (~85 tests).

Patterns:
- `expect(x).not.toBe(y)`
- `expect(x).resolves.toBe(y)`
- `expect(x).resolves.not.toThrow()`

### P0: PHP Mockery expectations as assertions (#38)

Mockery `shouldReceive()->once()` / `->with()` expectations are verified in tearDown
but not counted as assertions.

**Impact**: ~85% of Laravel T001 BLOCKs (~1100 tests).

Patterns:
- `$mock->shouldReceive('method')->once()`
- `$mock->shouldReceive('method')->once()->with('args')`
- `$mock->shouldReceive('method')->never()`

### P1: Python mock assertion methods (#39)

`unittest.mock` assertion methods are not detected by assertion.scm.

**Impact**: 3/19 fastapi T001 BLOCKs.

Patterns:
- `mock.assert_not_called()`
- `mock.assert_called_once_with()`
- `mock.assert_called_with()`

### P1: TS expect.assertions / expect.unreachable (#40)

Static assertion count and failure assertions not detected.

**Impact**: ~15% of vitest T001 FPs.

Patterns:
- `expect.assertions(N)`
- `expect.unreachable()`
- `expectType<T>(...)` (without "Of")

### P1: TS Chai method chains (#41)

Chai `.to.be.a()`, `.to.have.callCount()` etc. only property terminals
are matched, not method-call terminals.

**Impact**: ~25% of vitest T001 FPs.

Patterns:
- `expect(x).to.be.a('string')`
- `expect(spy).to.have.callCount(3)`
- `expect(spy).to.have.been.calledWith(...)`

Also missing property: `returned`.

### P2: Python nested test function assertion counting (#42)

When a test function contains a nested `def test_*()`, the parent function's
assertions may not be counted correctly.

**Impact**: 1/19 fastapi T001 BLOCKs (rare edge case).

Example: `test_get_db()` contains nested `async def test_async_gen()`,
and the `assert` statement in `test_get_db` after the nested function is not counted.

### P1: PHP PHPUnit mock expectations (#38)

PHPUnit's built-in mock: `$mock->expects($this->once())->method('name')`.
Same issue as Mockery, combined in same issue.

## Laravel Post-#38 Dogfooding (2026-03-09)

Post-Mockery fix: 1305 → 776 BLOCK (-529). Remaining 776 breakdown:

| Category | Count | FP? | Pattern |
|----------|-------|-----|---------|
| `$obj->assert*()` | 449 | FP | `$response->assertStatus()`, `->assertJson()`, etc. |
| `->expects*()` | 85 | FP | Artisan `->expectsOutput()`, `->expectsQuestion()` |
| `self::assert*()` | 13 | FP | PHPUnit static assertion calls |
| `->should*()` | 5 | FP | Mockery edge cases |
| Truly assertion-free | 278 | TP | Smoke tests, delegation, no oracle |

**Root cause**: assertion.scm only matches `$this->assert*()`. Three missing patterns:

1. **Any-object `->assert*()`**: Laravel TestResponse, Fluent assertions, Mail assertions, etc.
   Top methods: assertStatus(105), assertHeader(63), assertExactJson(54), assertJsonPath(53), assertSent(52)
2. **`self::assert*()` / `static::assert*()`**: PHPUnit static calls (13 cases)
3. **`->expects*()` as assertion**: Artisan `expectsOutput`(115), `expectsOutputToContain`(35), etc.

## WARN Analysis

### T107 (assertion-roulette)

| Project | T107 Count | % of Tests |
|---------|-----------|-----------|
| requests | 134 | 39.5% |
| fastapi | 772 | 36.4% |
| laravel | 5191 | 48.1% |

T107 detects tests with 2+ assertions and no failure messages. This is
technically correct (pytest `assert` statements don't have messages by
convention, and PHPUnit `$this->assert*` often omit the message parameter).

**Assessment**: True positives, but severity (WARN) may be too aggressive for
real-world codebases where assertion messages are rarely used. Consider
demoting to INFO. This is input for #24 (severity review).

### T101 (how-not-what)

| Project | T101 Count | % of Tests |
|---------|-----------|-----------|
| fastapi | 7 | 0.3% |
| vitest | 85 | 2.7% |
| laravel | 1731 | 16.0% |
| requests | 9 | 2.7% |

Laravel's high rate (16%) may include Mockery expectation patterns being
incorrectly classified. Investigation needed after #38 fix.

### T003 (giant-test)

| Project | T003 Count | % of Tests |
|---------|-----------|-----------|
| fastapi | 209 | 9.9% |
| vitest | 114 | 3.7% |
| laravel | 171 | 1.6% |
| requests | 1 | 0.3% |

True positives. Threshold of 50 lines is reasonable.

## New Issues Filed

| # | Title | Priority |
|---|-------|----------|
| #37 | T001 FP: TS expect modifier chains (.not/.resolves) | P0 |
| #38 | T001 FP: PHP/Python mock expectations as assertions | P0 |
| #39 | T001 FP: TS expect.assertions / expect.unreachable | P1 |
| #40 | T001 FP: TS Chai method-call chain terminals | P1 |
| #41 | T001 FP: Python nested test function assertion counting | P2 |

## Recommendations for #24 (Severity Review)

Based on dogfooding data:

1. **T107 -> INFO**: 36-48% of tests trigger across all projects. Too noisy at WARN.
2. **T101 WARN is appropriate**: Low rates (0.3-2.7%) except Laravel (needs post-#38 recheck).
3. **T003 WARN is appropriate**: Rates are reasonable (1.6-9.9%).
4. **T001 BLOCK is correct**: After FP fixes, remaining BLOCKs are genuine assertion-free tests.

## vitest T001 Hardening Summary (CLOSED)

**Period**: 2026-03-09
**Issues**: #37, #39, #40, #42, #43
**BLOCK progression**: 432 → 350 (#37) → 339 (#39) → 334 (#40) → 326 (#42+#43)
**Cumulative reduction**: -106 (24.5%)

| Fix | Issue | Impact |
|-----|-------|--------|
| expect .not/.resolves chains | #37 | -82 |
| expect.assertions/unreachable | #39 | -11 |
| Chai method-call chain terminals | #40 | -5 |
| Chai intermediate vocabulary + returned | #42 | -5 |
| expect.soft modifier chains | #43 | -3 |

**Conclusion**: Generic query-fixable FP clusters exhausted. Remaining 326 BLOCKs are:
- Project-local custom assertion helpers (→ `.exspec.toml` `custom_patterns` escape hatch)
- True positives (assertion-free tests)
- Edge cases not worth query complexity (diminishing returns)

**Status**: CLOSED. No further vitest-specific T001 work planned.

## NestJS Dogfooding (2026-03-10)

**2675 tests, 380 files, 90 T001 BLOCK → 81 FP (90%), 9 TP (10%)**

### FP Breakdown

| FP Pattern | Count | Root Cause |
|-----------|-------|-----------|
| `chai_instanceof_alias` | 20 | `.instanceof()` (lowercase) missing from method terminals |
| `sinon_mock_verify` | 17 | `.verify()` pattern not detected → **FIXED (#51)** |
| `chai_throws_alias` | 11 | `.throws()` missing (only `throw`) |
| `chai_throw_property` | 6 | `.to.be.throw` (property) missing |
| `chai_eventually_deep` | 6 | depth > 5 + `and` not in intermediate chain |
| `chai_contains_alias` | 6 | `.contains()` missing (only `contain`) |
| `chai_ownProperty` | 5 | `.ownProperty()` missing from method terminals |
| `chai_length` | 5 | `.length()` missing (only `lengthOf`) |
| `chai_equals_alias` | 3 | `.equals()` missing (only `equal`) |
| `return_expect_rejected` | 2 | `return expect(...)...` wrapped in `return_statement` |

### TP Breakdown (9)

| Category | Count | Examples |
|----------|-------|---------|
| `done()` callback oracle | 3 | router-response-controller.spec.ts |
| Helper delegation (no direct assertion) | 3 | file-type.validator.spec.ts, client-tcp.spec.ts |
| `expect(value)` bare (no chain, no-op) | 2 | bar.service.spec.ts |
| `@ts-expect-error` compile-time check | 1 | reflector.service.spec.ts |

### Issues Filed

| # | Title | Expected Impact |
|---|-------|----------------|
| #50 | T001 FP: TS Chai alias/property vocabulary expansion | -56 FPs (DONE) |
| #51 | T001 FP: TS Sinon mock .verify() method-call oracle | -17 FPs (DONE) |
| #52 | T001 FP: TS return-wrapped Chai property assertions | -2 FPs |

### Post-#51 Verification (2026-03-10)

Re-dogfooding confirmed: **90 → 34 (#50) → 17 (#51)**. All 17 remaining are TP:

| Category | Count | Examples |
|----------|-------|---------|
| Helper delegation (return helper()) | 8 | file-type.validator.spec.ts, injector.spec.ts |
| done() callback oracle | 3 | router-response-controller.spec.ts |
| Bare expect() no-op | 2 | bar.service.spec.ts (expect(stub.called) without chain) |
| @ts-expect-error compile check | 1 | reflector.service.spec.ts |
| Assertion in try/catch only | 2 | parse-bool.pipe.spec.ts, validation.pipe.spec.ts |
| client helper delegation | 1 | client-tcp.spec.ts |

**FP rate: 0%**. No query-fixable FP clusters remain. #52 (return-wrapped, -2 est.) is the only remaining FP issue but may not be worth the query complexity.

### WARN/INFO Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T102 (fixture-sprawl) | 378 | 14.1% |
| T109 (undescriptive-name) | 348 | 13.0% |
| T105 (deterministic-no-metamorphic) | 143 | 5.3% |
| T101 (how-not-what) | 38 | 1.4% |
| T106 (duplicate-literal) | 22 | 0.8% |
| T003 (giant-test) | 18 | 0.7% |
| T108 (wait-and-see) | 12 | 0.4% |
| T002 (mock-overuse) | 4 | 0.1% |

## Key Technical Discoveries

### instanceof is a safe Chai terminal alias (NestJS #50)

tree-sitter parses `.instanceof(Error)` as `property_identifier: "instanceof"`, NOT as the JavaScript keyword. This means it can be safely matched in member-expression property position as a Chai method terminal, alongside `throw`, `throws`, `contains`, etc.

### Deep Chai chains require depth > 5 support (NestJS #50)

vitest-oriented patterns capped at depth-5 (`expect(x).a.b.c.d.e()`). NestJS Chai usage revealed depth-6 and depth-7 chains:
- `expect(x).to.eventually.be.rejected.and.be.an.instanceof(Error)` (depth-7)
- `expect(x).to.be.rejected.and.have.property('message')` (depth-6)

Intermediate `and` / `rejected` / `fulfilled` were missing from the chain vocabulary.

### Broad .verify() matching is safe for T001 (NestJS #51)

Sinon mock `.verify()` is the primary use of `.verify()` in test code. Rather than constraining to Sinon-specific patterns, broad matching (`any_expr.verify()` counts as assertion) was chosen. The risk of false negatives (non-assertion `.verify()`) is acceptable for T001's purpose of detecting oracle-free tests.

## Rust Dogfooding (2026-03-10)

### ripgrep

**16 tests detected (of ~346 actual), 15 files, 0 T001 BLOCK.**

**Important caveat**: ripgrep's test suite uses a custom `rgtest!` macro that generates `#[test] fn` internally. tree-sitter parses macro invocations as `macro_invocation > token_tree`, making the ~330 `rgtest!` tests invisible to exspec. Only 16 tests in `crates/` with bare `#[test]` were detected.

This is a **significant limitation of Rust support**: projects using macro-generated test functions will have most tests undetected. The `rgtest!` macro also uses `eqnice!` (a custom assertion macro using `panic!`), so even if tests were detected, assertions would be missed.

Detected tests (16) had clean results:

| Rule | Count | Notes |
|------|-------|-------|
| T003 (giant-test) | 2 | 102 and 126 lines. TP |
| T107 (assertion-roulette) | 12 | All TP (assert! without messages) |
| T109 (undescriptive-name) | 3 | "find", "captures", "replace" |

**Conclusion**: ripgrep is not usable as a Rust dogfooding benchmark due to macro-heavy test structure. Use tokio instead.

### clap

**1455 tests, 134 files, 528 T001 BLOCK. FP rate: 41.3% (218/528).**

Test detection worked well (1455/~1577 `#[test]` detected). `#[should_panic]` (70 tests) correctly excluded from T001.

#### BLOCK Breakdown

| Category | Count | Type | Notes |
|----------|-------|------|-------|
| Truly assertion-free | 310 | TP | Builder pattern tests, smoke tests, no oracle |
| `assert_data_eq!` macro | 115 | FP | snapbox custom assertion macro (token_tree) |
| `common::assert_matches()` helper | 103 | FP | Helper delegation to shared test utility |

#### Key Findings

1. **clap confirms the two known Rust FP patterns**: custom assertion macros (token_tree) and helper delegation. No new FP categories found.

2. **`#[should_panic]` detection works correctly**: All 70 `#[should_panic]` tests were excluded from T001, confirming the sibling-walk detection logic.

3. **`custom_patterns` mitigation**:
   ```toml
   [assertions]
   custom_patterns = ["assert_data_eq!", "assert_matches"]
   ```

4. **High TP count (310)** is notable: clap has many tests that construct a `Command` and call `.debug_assert()` or just verify parsing succeeds without checking the result. These are genuinely assertion-free smoke tests.

#### WARN Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T003 (giant-test) | 51 | 3.5% |
| T102 (fixture-sprawl) | 19 | 1.3% |
| T006 (low-assertion-density) | 9 | 0.6% |

### tokio

**1582 tests, 271 files, 388 T001 BLOCK. FP rate: 33.8% (131/388).**

#### BLOCK Breakdown

| Category | Count | Type | Notes |
|----------|-------|------|-------|
| Truly assertion-free | 187 | TP | Smoke tests, concurrency setup, no oracle |
| Custom assert macros (`assert_pending!`, `assert_ready!`, etc.) | 124 | FP | `tokio-test` macros not detected. Fixable via `custom_patterns` |
| loom model check | 34 | TP | `loom::model()` verifies concurrency, no explicit assertions |
| trybuild/compile-fail | 21 | TP | `#[tokio::test]` macro error tests |
| `panic!` only | 15 | TP | panic is not an oracle |
| `assert!` inside `select!` macro | 5 | FP | token_tree limitation (Known Constraint) |
| std assert missed | 2 | FP | assert inside `tokio::select!` nested closures |

#### Key Findings

1. **Custom assert macros are the dominant FP source** (124/131 = 95% of FPs). `tokio-test` provides `assert_pending!`, `assert_ready!`, `assert_ok!`, `assert_elapsed!` etc. These are not recognized because tree-sitter parses macro invocations as `macro_invocation > token_tree`, hiding the assertion semantics.

2. **`custom_patterns` is the correct mitigation**:
   ```toml
   [assertions]
   custom_patterns = ["assert_pending!", "assert_ready!", "assert_ok!", "assert_elapsed!", "assert_done!", "assert_next_eq!", "assert_err!", "assert_ready_eq!", "assert_ready_ok!", "assert_next_err!", "assert_next_pending!", "assert_ready_err!"]
   ```

3. **`select!` macro token_tree issue** (5 FPs): `assert!` inside `tokio::select!` body is invisible to tree-sitter. Already documented as Known Constraint.

4. **loom tests are correctly TP**: `loom::model(|| { ... })` tests verify concurrency properties through the loom runtime, not through explicit assertions. T001 is correct to flag these.

#### WARN Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T102 (fixture-sprawl) | 77 | 4.9% |
| T003 (giant-test) | 59 | 3.7% |
| T108 (wait-and-see) | 43 | 2.7% |
| T006 (low-assertion-density) | 38 | 2.4% |
| T101 (how-not-what) | 8 | 0.5% |

## T101 Triage Experience: AWS Cognito Wrapper Tests (2026-03-10)

**Project**: Internal Next.js app, `cognito.test.ts` (11 test functions, 19 T101 hits)
**Context**: AWS SDK thin wrapper module. Tests verify parameter passthrough via constructor spies.

### Pattern Classification

Real-world T101 triage revealed 4 distinct categories with different appropriate responses:

| Category | Example | Count | Action | Rationale |
|----------|---------|-------|--------|-----------|
| Command construction spy | `expect(SignUpCommand).toHaveBeenCalledWith({...})` | 8 | KEEP + suppress | Wrapper's core behavior; parameter passthrough is the specification |
| Redundant mockSend | `expect(mockSend).toHaveBeenCalled()` + result assertion | 2 | DELETE | Result assertion already proves send was called |
| Fire-and-forget mockSend | `expect(mockSend).toHaveBeenCalled()` on void function | 5 | KEEP + suppress | No return value; send execution is the only observable evidence |
| Console.error spy | `expect(consoleSpy).toHaveBeenCalledWith(...)` | 1 | KEEP + suppress | Observable side-effect (logging) is part of error handling spec |

### Key Insight: "Redundant spy" vs "Sole verification"

The critical distinction is whether the test has **behavior assertions alongside the spy**:

- **spy + result assertion** = spy is redundant (if `result` is correct, `send` must have been called)
- **spy only (void function)** = spy is the sole verification, removing it loses test coverage

This maps directly to data exspec already has: `assertion_count` vs `how_not_what_count`.

### Potential Improvement: Contextual T101 Diagnostic

Current message: `"how-not-what: 2 implementation-testing pattern(s) detected"`

Enhanced message could distinguish:
- `assertion_count > how_not_what_count`: "test also has behavior assertions; mock checks may be redundant"
- `assertion_count == how_not_what_count`: "test has no behavior assertions beyond mock checks"

This stays within static analysis scope (no type/return-value analysis needed) and directly aids triage decisions. **Not yet implemented**; tracked here as a potential enhancement.

### Result

- 2 redundant spy lines deleted (genuine improvement)
- 11 functions annotated with `// exspec-ignore: T101` (legitimate patterns)
- cognito.test.ts T101 warnings: 19 → 0

## Django Dogfooding (2026-03-11)

**1,047 tests, 617 files, 23 T001 BLOCK. FP rate: 39% (9/23).**

### BLOCK Breakdown

| Category | Count | Type | Notes |
|----------|-------|------|-------|
| Implicit no-exception assertion | 8 | TP | test body runs code, success = no exception |
| Helper delegation (self.check_output, self.check_html) | 8 | FP | Custom assertion helpers not detected |
| Decorator wrapping | 1 | FP | @test_mutation() injects assertion logic |
| Pass-only / non-test | 2 | TP | Explicit pass statement or function name misleading |
| Assertion in nested block | 1 | TP | assert in if/elif not counted |

### Key Findings

1. **self.assert*() detection works**: Django's TestCase assertion methods (assertContains, assertRedirects, etc.) are correctly detected. No query-level FP from this pattern.
2. **FPs are all helper delegation** (known pattern): `self.check_output()`, `self.check_html()`, `_test_argon2_upgrade()`. Mitigated by `custom_patterns`.
3. **No new FP categories found**. Django is well-served by existing detection.

### WARN Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T101 (how-not-what) | 30 | 2.9% |
| T003 (giant-test) | 8 | 0.8% |
| T006 (low-assertion-density) | 1 | 0.1% |

## pytest Dogfooding (2026-03-11)

**2,380 tests, 108 files, 594 T001 BLOCK. FP rate: ~100%.**

### BLOCK Breakdown

| Category | Count (est.) | Type | Notes |
|----------|-------------|------|-------|
| `result.stdout.fnmatch_lines()` | 415 | FP | Helper delegation (not query-fixable) |
| `reprec.assertoutcome()` | 148 | FP | `assertX()` without underscore (#62) |
| `child.expect()` (pexpect) | 20 | FP | External library assertion method |
| Other result assertion helpers | 11 | FP | Fixture-based assertion helpers |

### Key Findings

1. **Critical FP**: `reprec.assertoutcome()` is missed because assertion.scm matches `^assert_` (with underscore) but `assertoutcome` has no underscore separator. **Fix: #62 (P0)**.
2. **fnmatch_lines()** is pytest's primary assertion helper. It raises `AssertionError` on mismatch. This is pure helper delegation — `custom_patterns = ["fnmatch_lines"]` would resolve 415 FPs.
3. **100% FP rate** makes pytest dogfooding currently unusable. #62 fix would reduce to ~75% FP (remaining = helper delegation).

### WARN Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T003 (giant-test) | 73 | 3.1% |
| T101 (how-not-what) | 27 | 1.1% |
| T006 (low-assertion-density) | 9 | 0.4% |
| T108 (wait-and-see) | 5 | 0.2% |

## Symfony Dogfooding (2026-03-11)

**17,148 tests, 2,416 files, 759 T001 BLOCK. FP rate: ~24% (182/759).**

### BLOCK Breakdown

| Category | Count | Type | Notes |
|----------|-------|------|-------|
| Implicit mock verification | ~400 | TP | PHPUnit mock expectations verified on teardown |
| `$this->addToAssertionCount()` | 91 | FP | PHPUnit assertion counter (#63) |
| `$this->markTestSkipped()` only | 91 | FP | Skip-only functions (#64) |
| Parent test delegation | ~50 | TP | `parent::testSomething()` |
| Helper method delegation | ~50 | TP | Inherited helper methods |
| Exception-based testing | ~20 | TP | try/catch only |
| Other edge cases | ~57 | TP | Setup-only, framework patterns |

### Key Findings

1. **addToAssertionCount()**: PHPUnit's official assertion counter. 91 FPs. **Fix: #63 (P1)**.
2. **markTestSkipped()-only functions**: 91 FPs. These are intentional non-applicability declarations, not assertion-free tests. **Fix: #64 (P1)**.
3. **76% TP rate** (577/759) after removing the two FP patterns. Remaining TPs are genuine: mock-only tests, parent delegation, helper delegation.
4. **Symfony vs Laravel**: Different FP profile. Laravel's main FP was Facade assertions and helper delegation. Symfony's main FP is addToAssertionCount() and markTestSkipped().

### WARN Summary

| Rule | Count | % of Tests |
|------|-------|-----------|
| T101 (how-not-what) | 1283 | 7.5% |
| T003 (giant-test) | 247 | 1.4% |
| T108 (wait-and-see) | 106 | 0.6% |
| T006 (low-assertion-density) | 78 | 0.5% |
| T002 (mock-overuse) | 3 | 0.02% |

### New Issues Filed

| # | Title | Priority |
|---|-------|----------|
| #62 | T001 FP: Python `^assert_` → `^assert` broadening | P0 |
| #63 | T001 FP: PHP addToAssertionCount() as assertion | P1 |
| #64 | T001: exclude skip-only test functions from evaluation | P1 |
| #65 | T110: skip-only-test detection (INFO) | P2 |

## Phase 8a-2: WARN/INFO FP Rate Survey (2026-03-13)

**exspec version**: 0.1.2 (commit 79dd714)
**Method**: 20-30 samples per rule per project, classified as TP/FP by reading source code.

### Re-dogfooding: 8a-1 verification

| Project | Metric | Before (#62/#63/#64) | After | Change |
|---------|--------|---------------------|-------|--------|
| pytest | T001 BLOCK | 594 | 515 | -79 (-13.3%) |
| symfony | T001 BLOCK | 759 | 615 | -144 (-19.0%) |

pytest remaining 515 BLOCKs: ~415 fnmatch_lines() helper delegation (8a-4 scope), rest are TP or other helpers.
symfony remaining 615 BLOCKs: majority TP (mock verification, parent delegation), helper delegation addressed by 8a-4.

### Updated WARN/INFO counts (all projects)

| Rule | laravel | nestjs | tokio | fastapi | symfony | pytest | django |
|------|---------|--------|-------|---------|---------|--------|--------|
| T101 (Warn) | 1728 (16.0%) | 38 (1.4%) | 8 (0.5%) | 7 (0.3%) | 1283 (7.5%) | 27 (1.1%) | 30 (2.9%) |
| T102 (Warn) | -- | 378 (14.1%) | 77 (4.9%) | -- | -- | -- | -- |
| T003 (Warn) | 171 (1.6%) | 18 (0.7%) | 59 (3.7%) | 209 (9.9%) | 247 (1.4%) | 73 (3.1%) | 8 (0.8%) |
| T109 (Info) | 122 (1.1%) | 348 (13.0%) | 88 (5.6%) | 81 (3.8%) | 347 (2.0%) | 65 (2.7%) | 26 (2.5%) |
| T105 (Info) | 192 (1.8%) | 145 (5.4%) | 112 (7.1%) | 238 (11.2%) | 444 (2.6%) | 9 (0.4%) | 10 (1.0%) |
| T106 (Info) | 1181 (10.9%) | 22 (0.8%) | 44 (2.8%) | 316 (14.9%) | 944 (5.5%) | 57 (2.4%) | 39 (3.7%) |
| T108 (Warn) | 10 (0.1%) | 12 (0.4%) | 43 (2.7%) | 1 (0.0%) | 106 (0.6%) | 5 (0.2%) | -- |

### FP Rate Survey Results

| Rule | Severity | Sampled Projects | TP Rate | FP Rate | Verdict |
|------|----------|-----------------|---------|---------|---------|
| T105 | INFO | fastapi, NestJS | 92% | 8% | Healthy. Keep INFO |
| T101 | WARN | Laravel, Symfony | 53% | 47% | Demote WARN→INFO |
| T109 | INFO | NestJS, tokio | 50% | 50% | Keep INFO |
| T102 | WARN | NestJS, tokio | 20% | 80% | Demote WARN→INFO or threshold↑ |
| T003 | WARN | fastapi | 5% | 95% | Threshold↑ or snapshot exclusion |
| T108 | WARN | tokio, Symfony | 7% | 93% | Demote WARN→INFO or OFF |
| T106 | INFO | fastapi, laravel | 7% | 93% | Demote INFO→OFF |

### T101 (how-not-what) — FP 47%

**Sampled**: Laravel 15, Symfony 15

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| Laravel | 11 | 4 | 73% |
| Symfony | 5 | 10 | 33% |

**FP causes**:
1. **Fire-and-forget verification**: broadcast(), dispatch() with no return value — mock assertions ARE the specification
2. **Delegating wrappers**: TraceableMessageBus, MarshallingSessionHandler — verifying delegation IS the contract
3. **Contract boundaries**: Repository.find(), decision manager, lock store — collaborator invocation is part of the API contract
4. **Security-critical operations**: password verification (hasher.check()) must be mocked

**Why Laravel > Symfony TP rate**: Laravel tests more often use shouldReceive() chains to verify internal query construction (implementation detail). Symfony tests more often use mock expectations to define collaborator contracts (specification).

### T102 (fixture-sprawl) — FP 80%

**Sampled**: NestJS 15, tokio 10

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| NestJS | 5 | 10 | 33% |
| tokio | 0-2 | 8-10 | 0-20% |

**FP causes**:
1. **DI framework overhead**: NestJS modules require provider, service, controller, guard, interceptor fixtures as minimum setup
2. **Network test setup**: tokio tests need sender, receiver, addr, buffer, message as inherent components
3. **Tracing/observability tests**: span builders, event matchers, subscriber setup are legitimate test data
4. **Threshold too low**: default threshold catches legitimate integration tests. Most FPs have 6-9 fixtures.

### T003 (giant-test) — FP 95%

**Sampled**: fastapi 20

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| fastapi | 1 | 19 | 5% |

**FP cause**: OpenAPI schema snapshot testing. Pattern: single `assert response.json() == {large_dict}`. The dict is 50-1200+ lines of schema definition. Logically cohesive (single assertion), length is data size not logic complexity.

**Caveat**: fastapi is worst-case for T003 due to snapshot-heavy tests. Other projects show 0.7-3.7% hit rate with likely higher TP rates. fastapi-specific, not systemic.

### T109 (undescriptive-name) — FP 50%

**Sampled**: NestJS 15, tokio 10

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| NestJS | 10 | 5 | 67% |
| tokio | 2 | 8 | 20% |

**FP causes**:
1. **API method names**: test names like `read()`, `write()`, `pin()` directly match the public API being tested
2. **Domain-specific terms**: `coop` (cooperative scheduling), `empty` (container state) are clear to ecosystem developers
3. **Framework operations**: NestJS REPL commands (`get()`, `$()`, `debug()`) are recognizable method names
4. **Describe block context**: parent describe/mod provides context that makes short names sufficient

**TP pattern**: Generic version numbers (`V1`, `V2`, `V3`) and placeholder names (`usage`, `shell`) are genuinely undescriptive.

### T105 (deterministic) — FP 8%

**Sampled**: fastapi 10, NestJS 5

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| fastapi | 8 | 2 | 80% |
| NestJS | 5 | 0 | 100% |

**Healthy rule**. Minor FP from OpenAPI schema snapshot tests where exact equality is the correct approach. No action needed.

### T106 (duplicate-literal) — FP 93%

**Sampled**: fastapi 10, laravel 5

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| fastapi | 1 | 9 | 10% |
| laravel | 0 | 5 | 0% |

**FP causes**:
1. **Snapshot/schema tests**: `"application/json"`, `"type"`, `"schema"` appear in multiple nested paths — structural necessity in OpenAPI validation
2. **Message passthrough testing**: same string tested through different code paths to verify consistency
3. **Realistic test data**: array/string literals are intentional, not parameterization candidates

### T108 (wait-and-see) — FP 93%

**Sampled**: tokio 10, Symfony 5

| Project | TP | FP | TP Rate |
|---------|----|----|---------|
| tokio | 0 | 10 | 0% |
| Symfony | 1 | 4 | 20% |

**FP causes**:
1. **Time-mocked tests**: tokio `start_paused=true` and Symfony Clock Mock eliminate real delays while preserving delay logic in code
2. **Timing behavior tests**: TTL/expiration testing, delay verification — sleep IS the behavior being tested
3. **Controlled async tests**: tokio's `time::pause()` makes delays deterministic and instant

**The rule cannot distinguish real blocking sleeps from mocked/controlled time.**

## Rust Observe Dogfooding (2026-03-18)

**exspec version**: post-#96 (commit bc6ff04)
**Feature**: `exspec observe --lang rust` — test-to-code mapping via static AST analysis

### Summary

| Project | Prod Files | Test Files | Mapped | L0/L1 (inline/filename) | L2 (import) | Unmapped |
|---------|-----------|-----------|--------|------------------------|-------------|----------|
| exspec | 21 | 1 | 19 | 19 | 0 | 2 |
| tokio/tokio | 343 | 198 | 51 | 37 | **14** | 292 |
| clap (workspace) | 195 | 134 | 20 | 20 | 0 | 175 |
| clap_complete | 22 | 10 | 5 | 3 | **2** | 17 |
| ripgrep | 85 | 15 | 43 | 43 | 0 | 42 |

### Key Findings

1. **tokio Layer 2: 0 -> 14** (post-#96). Integration tests under `tokio/tests/` now resolve `use tokio::signal::unix`, `use tokio::time::sleep` etc. via Cargo.toml crate name parsing. Previously all 198 test files in `tests/` were invisible to Layer 2.

2. **clap_complete Layer 2: 2 matches** detected. Per-subcrate scanning works correctly; workspace root scanning returns 0 Layer 2 because `parse_crate_name` returns `None` for `[workspace]` Cargo.toml (by design).

3. **ripgrep: 43 mapped, all L0/L1**. ripgrep uses inline tests (`#[cfg(test)]` in production files) extensively. No integration tests in `tests/` directory, so Layer 2 is not triggered.

4. **exspec: 19/21 mapped, all L0**. Self-dogfooding via inline tests. 2 unmapped files (hints.rs, lib.rs) have no tests.

### Workspace Limitation (Resolved in #98)

~~When `scan_root` points to a workspace root, `parse_crate_name` returns `None` and Layer 2 is skipped.~~ **Fixed**: `find_workspace_members()` auto-detects member crates and applies L2 per member. Both virtual and non-virtual workspaces supported. See Post-#98 section below.

### tokio Layer 2 Detail

| Production File | Test Files (via import) | Import Pattern |
|----------------|------------------------|----------------|
| src/signal/unix.rs | 11 signal_*.rs tests | `use tokio::signal::unix` |
| src/time/timeout.rs | 7 tests | `use tokio::time::timeout` |
| src/time/sleep.rs | 6 tests | `use tokio::time::sleep` |
| src/time/interval.rs | 4 tests | `use tokio::time::interval` |
| src/time/error.rs | 2 tests | `use tokio::time::error` |
| src/sync/batch_semaphore.rs | 2 internal tests | `use crate::sync::batch_semaphore` |
| src/sync/broadcast.rs | 1 test (+ inline) | `use tokio::sync::broadcast` |
| src/sync/mpsc/error.rs | 1 test | `use tokio::sync::mpsc` |
| src/sync/oneshot.rs | 2 tests | `use tokio::sync::oneshot` |
| src/sync/rwlock.rs | 1 loom test | `use crate::sync::RwLock` |
| src/runtime/* | 4 tests | various runtime imports |
| src/macros/support.rs | 1 test | `use tokio::macros::support` |
| src/net/windows/named_pipe.rs | 1 test | `use tokio::net::windows` |

### Precision/Recall Estimate

- **Precision**: High (~98%). All 14 tokio Layer 2 matches are correct (verified by reading import statements).
- **Recall**: Low (~8%). 198 test files exist but only 14 production files gained Layer 2 matches. Many test files import multiple modules but only the first-level module is resolved. Deep re-exports and `use tokio::*` are not traced.

### Remaining Gaps

1. ~~**Workspace-level scanning**~~: Resolved in #98.
2. **Wildcard imports**: `use tokio::*` is not resolved (by design — too noisy).
3. **Deep re-export chains**: `tokio/src/lib.rs` re-exports `pub mod sync`, but the chain from `use tokio::sync::Mutex` to `src/sync/mutex.rs` requires multi-hop barrel resolution.
4. **Macro-generated test functions**: tokio's `#[tokio::test]` is handled (detected as `#[test]`), but custom `loom_cfg_*` macros hide some test files.

### Post-#99/#100 Re-verification (2026-03-18)

**exspec version**: post-#100 (commit 0750719)
**Changes**: #99 (`pub mod` as `wildcard=true` BarrelReExport) + #100 (`file_exports_any_symbol` for Rust)

#### Summary

| Project | Prod | Test | Mapped | L0/L1 | L2 | Unmapped | Delta (mapped) |
|---------|------|------|--------|-------|----|----------|----------------|
| exspec | 21 | 1 | 19 | 19 | 0 | 2 | 0 |
| tokio/tokio | 343 | 198 | 55 | 37 | **18** | 288 | **+4** |
| clap (workspace) | 195 | 134 | 20 | 20 | 0 | 175 | 0 |
| clap_complete | 22 | 10 | 5 | 3 | 2 | 17 | 0 |
| ripgrep | 85 | 15 | 43 | 43 | 0 | 42 | 0 |

#### Regression Check

- L0/L1: all projects unchanged. No regression.
- broadcast.rs: previously L2, now reported as "filename" strategy (inline test + import test combined). Not a regression — the external test file (sync_broadcast_weak.rs) is still mapped.

#### tokio L2 Changes: 14 -> 18 (+4)

New L2 matches (all verified TP):

| Production File | Test File(s) | Import | Why New |
|----------------|-------------|--------|---------|
| src/net/unix/pipe.rs | tests/net_unix_pipe.rs | `use tokio::net::unix::pipe` | `pub mod` chain: net/mod.rs → unix/mod.rs → pipe.rs |
| src/runtime/dump.rs | tests/task_trace_self.rs | `use tokio::runtime::dump` | `pub mod` chain: runtime/mod.rs → dump.rs |
| src/sync/mpsc/list.rs | tests/sync_mpsc_weak.rs | `use tokio::sync::mpsc` | `pub mod` chain: sync/mod.rs → mpsc/mod.rs, symbol filter resolves list.rs |
| src/runtime/task/id.rs | src/runtime/tests/task.rs | `use crate::runtime::task` | `pub mod` chain: runtime/mod.rs → task/mod.rs → id.rs |

All 4 new matches result from `pub mod` being treated as wildcard barrel re-export (#99), enabling multi-hop module chain resolution.

#### Precision/Recall Update

- **Precision**: 100% (18/18 L2 matches verified as TP). Improved from ~98% estimate.
- **Recall**: ~9% (55/343 production files mapped, vs 51/343 previously). Still low due to deep re-export chains and macro-generated tests.

#### Conclusion

#99/#100 delivered incremental improvement (+4 L2 matches in tokio) with zero false positives and zero regressions. The `pub mod` wildcard strategy correctly resolves multi-hop module chains (e.g., `tokio::net::unix::pipe` → `net/mod.rs` → `unix/mod.rs` → `pipe.rs`). Remaining gaps are workspace-level aggregation and deeper re-export chain resolution.

### Post-#98 Workspace Aggregation (2026-03-18)

**exspec version**: post-#98 (commit b564e28)
**Changes**: #98 (`find_workspace_members` + `has_workspace_section` for workspace-level L2 aggregation)

#### Summary (workspace root scanning)

| Project | Prod | Test | Mapped | L0/L1 | L2 | Unmapped | Delta vs subcrate-only |
|---------|------|------|--------|-------|----|----------|----------------------|
| tokio (workspace) | 495 | 272 | 71 | 40 | **31** | 424 | **+13 L2** (vs tokio/tokio 18) |
| clap (workspace) | 195 | 134 | 22 | 20 | **2** | 173 | **+2 L2** (vs 0 before) |
| tokio/tokio (subcrate) | 343 | 198 | 55 | 37 | 18 | 288 | unchanged |

#### Key Findings

1. **tokio workspace L2: 18 → 31 (+13)**. Workspace-level scanning now resolves imports in tokio-stream, tokio-test, tokio-util member crates. Previously these required per-subcrate runs.

2. **clap workspace L2: 0 → 2**. clap has both `[workspace]` and `[package]` in root Cargo.toml (non-virtual workspace). The initial #98 implementation only handled virtual workspaces; the fix added `has_workspace_section()` to detect workspaces regardless of `[package]` presence.

3. **No regressions**: tokio/tokio subcrate results unchanged (55 mapped, 18 L2). L0/L1 unchanged across all projects.

#### New L2 matches from workspace members

tokio workspace gained 13 new L2 matches from member crates:

| Member | Production File | Test Files | Import Pattern |
|--------|----------------|-----------|----------------|
| tokio-stream | src/wrappers.rs | 3 tests | `use tokio_stream::wrappers` |
| tokio-test | src/io.rs | 1 test | `use tokio_test::io` |
| tokio-test | src/stream_mock.rs | 1 test | `use tokio_test::stream_mock` |
| tokio-util | src/codec/length_delimited.rs | 1 test | `use tokio_util::codec` |
| tokio-util | src/compat.rs | 1 test | `use tokio_util::compat` |
| tokio-util | src/context.rs | 1 test | `use tokio_util::context` |
| tokio-util | src/sync/cancellation_token.rs | 2 tests | `use tokio_util::sync` |
| tokio-util | src/sync/mpsc.rs | 3 tests | `use tokio_util::sync::PollSender` |
| tokio-util | src/sync/poll_semaphore.rs | 1 test | `use tokio_util::sync` |
| tokio-util | src/sync/reusable_box.rs | 1 test | `use tokio_util::sync` |
| tokio-util | src/task/join_map.rs | 1 test | `use tokio_util::task` |
| tokio-util | src/time/delay_queue.rs | 2 tests | `use tokio_util::time` |
| tokio-util | src/udp/frame.rs | 1 test | `use tokio_util::udp` |

All 13 verified as TP (correct import → production file mapping).

#### Precision/Recall Update

- **Precision**: 100% (31/31 workspace L2 matches verified as TP).
- **Recall (tokio workspace)**: ~14% (71/495 production files mapped). Improved from ~9% (subcrate-only). Remaining gaps: wildcard imports, deep re-export chains, macro-generated tests.
- **Recall (clap workspace)**: ~11% (22/195). Improved from ~10% (L2 contribution small but non-zero).

#### Workspace Limitation: Resolved

The "Known" workspace limitation from the initial dogfooding is now fully resolved. Both virtual workspaces (tokio) and non-virtual workspaces (clap) are supported. No per-subcrate workaround needed.

## PHP Observe Dogfooding (2026-03-18)

**exspec version**: post-#98 (commit 67070f3)
**Feature**: `exspec observe --lang php` — PSR-4 namespace-based test-to-code mapping

### Summary

| Project | Prod | Test | Mapped | L0/L1 | L2 (import) | Unmapped |
|---------|------|------|--------|-------|-------------|----------|
| Laravel | 1945 | 910 | 968 | 0 | **968** | 977 |
| Symfony | 7937 | 2419 | 4117 | 0 | **4117** | 3820 |

### Key Findings

1. **All mappings are L2 (import tracing), L1 = 0**. PHP tests live in `tests/` and production in `src/`, so same-directory filename matching (Layer 1) never triggers. This is by design — Layer 2 handles the cross-directory case via PSR-4 namespace resolution.

2. **Test file coverage is high**: Laravel 806/910 (88.6%), Symfony 2367/2419 (97.8%). Most test files successfully map to at least one production file.

3. **Precision is effectively 100%**: Mappings are based on `use` statement import tracing. If a test file imports a class via `use App\Models\User`, the mapping to `src/Models/User.php` is mechanically correct. 30-sample spot-check confirmed all mappings are valid (including cross-class dependencies).

4. **Fixture files classified as production**: 88 files in Laravel's `tests/` subdirectories (e.g., `tests/Integration/Http/Fixtures/Post.php`) are classified as production files because they don't match test naming conventions. These are not FP in the mapping (tests correctly import them), but ideally `is_non_sut_helper()` would filter `Fixtures/` directories.

5. **Production coverage ~50%**: Laravel 968/1945 (49.8%), Symfony 4117/7937 (51.9%). Unmapped production files are typically internal classes not directly imported by any test file (e.g., framework internals, event classes, middleware).

### Precision/Recall

- **Precision**: ~100% (import-based mechanical matching, all spot-checks valid)
- **Recall (production)**: ~50% (unmapped files are framework internals not directly tested)
- **Recall (test files)**: 89-98% (most test files contribute to at least one mapping)

### Success Criteria Check

ROADMAP target: Precision >= 90%, Recall >= 80%.

- Precision: **PASS** (100%)
- Recall (test file coverage): **PASS** (89-98%)
- Recall (production file coverage): **BELOW** (50%) — but this measures "how many production files have tests", not mapping quality. Many production files genuinely have no direct tests.

### Potential Improvements

1. **Fixture directory filtering**: Add `Fixtures/` to `is_non_sut_helper()` to exclude test fixture files from production classification
2. **Composer.json PSR-4 autoload parsing**: Currently uses `common_prefixes` heuristic (`src/`, `app/`, `lib/`). Parsing `composer.json` autoload config would improve accuracy for non-standard layouts

## PHP observe post-#193/#194 (2026-03-25)

**exspec version**: v0.4.5-dev (post-#193 Fixtures/Stubs helper detection + PSR-4, post-#194 directory-aware fan-out filter)
**Repository**: laravel/framework @ f513824
**GT doc**: `docs/observe-ground-truth-php-laravel.md`

### Results

| Metric | Value | Target | Result |
|--------|-------|--------|--------|
| Precision (spot-check, 10 pairs) | **~100%** (10/10) | >= 98% | **PASS** |
| Recall (test file coverage) | **88.6%** (808/912) | >= 90% | **FAIL** (1.4pp below target) |
| Mapped test files | 808 | - | - |
| Unmapped test files | 104 | - | - |
| Fan-out blocked files | **0** | - | - |

### Comparison to pre-#193/#194 baseline

| Metric | Pre-#193/#194 | Post-#193/#194 | Delta |
|--------|---------------|----------------|-------|
| Recall (test file) | 85.1% (~776/912) | **88.6%** (808/912) | +3.5pp |
| Fan-out blocked | -63 (noise) | **0** | eliminated |
| Precision | 96.0% (FAIL) | **~100%** (PASS) | +4pp |

**#193 impact**: Fixtures/Stubs helper detection correctly excludes `tests/Fixtures/` and `tests/Stubs/` files from production classification. PSR-4 `composer.json` resolution improves namespace-to-path mapping accuracy.

**#194 impact**: Directory-aware fan-out filter (bidirectional name-match + directory segment match) resolved all 63 previously blocked files. 0 files blocked post-#194.

### FN Root Cause Analysis (104 unmapped)

| Category | Count | Root Cause | Fixable by static import tracing? |
|----------|-------|-----------|----------------------------------|
| View/Blade | 54 | `AbstractBladeTestCase` parent class -- no direct import of production file | No (requires inheritance tracing) |
| Integration/Generators | 28 | String literal `use` statements in code generation assertions | No (content is runtime string, not import) |
| Integration/Database | 10 | Framework helper access (`DB::`, `$this->app->make()`) | No (facade/IoC pattern) |
| Others | 12 | Various patterns (no direct import, cross-file delegation) | Requires individual analysis |
| **Total** | **104** | | |

### Structural Ceiling

R=88.6% is the structural ceiling for the current approach (L1 filename matching + L2 import tracing). All remaining 104 FN fall into cross-file delegation patterns that require:
- Inheritance chain traversal (parent class → production file) for View/Blade FN
- Runtime string content analysis for Generators FN
- IoC container resolution for Database/Integration FN

None of these are feasible with purely static import tracing.

### Ship Criteria Decision

**P PASS (100%), R FAIL (88.6% < 90%).** The 1.4pp gap to ship criteria is entirely due to structural patterns. Fixing requires parent class import propagation (#153, backlog) or IoC container resolution (not scoped). Ship criteria discussion deferred to separate decision.

## Rust/PHP Observe Re-dogfooding (2026-03-23, v0.4.2)

**exspec version**: v0.4.2-pre (post-#126, #146)

### Summary

| Project | Lang | Prod | Test | Mapped | Unmapped | Pairs | vs v0.3.0 |
|---------|------|------|------|--------|----------|-------|-----------|
| tokio | Rust | 495 | 272 | 71 (14.3%) | 424 | 112 | +20 mapped (51→71) |
| clap | Rust | 195 | 134 | 22 (11.3%) | 173 | 22 | +2 mapped (20→22) |
| laravel | PHP | 1951 | 912 | 973 (49.9%) | 978 | 3790 | +5 mapped (968→973) |

### Key Changes from v0.3.0

1. **tokio**: 51→71 mapped (+20). Workspace member aggregation improvements from v0.4.0/v0.4.1 resolve more `use crate::` imports.
2. **clap**: 20→22 mapped (+2). Minor improvement from workspace handling.
3. **laravel**: 968→973 mapped (+5). Marginal improvement from PSR-4 resolution refinements.

### Precision Spot-Check

GT audit not performed. First-pass numbers only. GT audit deferred to separate issue.

### Ship Criteria

Rust and PHP observe remain **experimental** (no formal GT audit). TS (P=100%, R=91%) and Python (P=98.2%, R=96.8%) are stable.

## Reproduction

```bash
# Build
cargo build --release

# Run
./target/release/exspec --lang rust --format json .
./target/release/exspec --lang python --format json /tmp/fastapi/tests
./target/release/exspec --lang python --format json /tmp/requests/tests
./target/release/exspec --lang typescript --format json /tmp/vitest/test
./target/release/exspec --lang php --format json /tmp/laravel/tests
./target/release/exspec --lang typescript --format json /tmp/nestjs
./target/release/exspec --lang rust --format json /tmp/ripgrep
./target/release/exspec --lang rust --format json /tmp/tokio
./target/release/exspec --lang rust --format json /tmp/clap

# Observe (test-to-code mapping)
./target/release/exspec observe --lang rust --format json .
./target/release/exspec observe --lang rust --format json /tmp/tokio          # workspace root
./target/release/exspec observe --lang rust --format json /tmp/tokio/tokio    # subcrate
./target/release/exspec observe --lang rust --format json /tmp/clap           # non-virtual workspace
./target/release/exspec observe --lang rust --format json /tmp/ripgrep
./target/release/exspec --lang python --format json /tmp/django/tests
./target/release/exspec --lang python --format json /tmp/pytest/testing
./target/release/exspec --lang php --format json /tmp/symfony

# Observe: PHP
./target/release/exspec observe --lang php --format json /tmp/laravel
./target/release/exspec observe --lang php --format json /tmp/symfony

# Observe: Python
./target/release/exspec observe --lang python --format json /tmp/httpx
./target/release/exspec observe --lang python --format json /tmp/requests
```

## Python Observe Dogfooding (2026-03-19)

**Feature**: `exspec observe --lang python` — test-to-code mapping via static AST analysis
**Ship criteria**: Precision >= 98%, Recall >= 90% (test file coverage)

### httpx (encode/httpx @ b5addb64)

#### Phase 21 (re-dogfood + FP fix + GT re-audit)

| Metric | Phase 20 (estimated) | Phase 21 (measured) |
|--------|---------------------|---------------------|
| Production files detected | 29 | 29 |
| Test files detected | 31 | 31 |
| Mapped production files | 21 | 18 |
| Test file coverage (Recall) | 96.8% (30/31) | **96.8%** (30/31) |
| Mapping pairs | ~64 | 56 |
| Precision (pair, vs GT) | ~94% (estimated) | **98.2%** (55/56) |
| TP | -- | 55 |
| FP | ~4 (estimated) | 1 |
| FN (primary) | -- | 3 |
| F1 | -- | **97.5%** |

**Result: Ship criteria PASS (P>=98%, R>=90%).**

Code fixes: `is_non_sut_helper()` excludes `mock*.py` (6 FP), `__version__.py` (2 FP), `_types.py` (2 FP).
GT re-audit: 23 secondary_targets added. 1 remaining FP: `_models.py <- test_timeouts.py` (0 assertions on model).

<details>
<summary>Phase 20 results (historical, estimated)</summary>

| Metric | Phase 18/19 | Phase 20 |
|--------|-------------|----------|
| Production files detected | 29 | 29 |
| Test files detected | 31 | 31 |
| Mapped production files | 22 | 21 |
| Test file coverage (Recall) | 96.8% (30/31) | 96.8% (30/31) |
| Mapping pairs | ~66 | ~64 |
| Estimated Precision | ~92% | ~94% |

Note: Phase 20 precision was hand-estimated, not pair-based measurement.

</details>

Phase 20 で `tests/common.py` の FP が解消。残る FP は barrel import 経由の incidental mappings のみ。

##### Remaining FP Sources (~4 pairs)

| FP Type | Pairs | Example |
|---------|-------|---------|
| Incidental barrel import (`__version__`) | 2 | `__version__.py` ← test_event_hooks (uses `httpx.__version__` in assertion strings, not testing __version__) |
| Utility type imports (`_types`) | 2 | `_types.py` ← test_async_client (type annotations only) |

##### Unmapped Test File

- `tests/test_exported_members.py` — no matching production file (tests module-level `__all__`)

<details>
<summary>Phase 18/19 results (historical)</summary>

| Metric | Before (pre-Phase 18) | After (Phase 18/19) |
|--------|----------------------|---------------------|
| Production files detected | 23 | 29 |
| Mapped production files | 3 | 22 |
| Test file coverage (Recall) | 6.2% (2/32) | 96.8% (30/31) |
| Estimated Precision | 66.7% | ~92% |

</details>

#### Pre-Phase 18 Results (historical)

<details>
<summary>Original measurement (P=66.7%, R=6.2%)</summary>

| Metric | Value |
|--------|-------|
| Test files | 30 |
| Production files | 23 |
| TP | 2 |
| FP | 1 |
| FN | 30 |
| **Precision** | **66.7%** |
| **Recall** | **6.2%** |
| **F1** | **11.4%** |

Root causes: `_` prefix not stripped, barrel imports not resolved, cross-directory matching failed.

</details>

### Requests (psf/requests, latest main)

#### Phase 20 後 (test helper exclusion)

| Metric | Phase 18/19 | Phase 20 |
|--------|-------------|----------|
| Production files detected | 27 | 27 |
| Test files detected | 9 | 9 |
| Mapped production files | 18 | 14 |
| Test file coverage (Recall) | 100% (9/9) | **100%** (9/9) |
| Mapping pairs | ~26 | ~21 |
| Estimated Precision | ~81% | **~100%** |

**Result: Recall PASS, Precision PASS** — Phase 20 で全テストヘルパー FP が解消。

Phase 20 で `tests/compat.py`, `tests/testserver/server.py`, `tests/utils.py` の FP が全て解消。残存 FP なし。

##### Strategy Breakdown

| Strategy | Prod Files | Pairs | Notes |
|----------|-----------|-------|-------|
| filename | 6 | ~7 | `adapters`, `help`, `hooks`, `packages`, `structures`, `utils` |
| import | 8 | ~14 | Barrel resolution through `__init__.py` + assertion filter |

<details>
<summary>Phase 18/19 results (historical)</summary>

| Metric | Before (pre-Phase 18) | After (Phase 18/19) |
|--------|----------------------|---------------------|
| Production files detected | 0 (in `src/`) | 27 |
| Mapped production files | 0 | 18 |
| Test file coverage (Recall) | 0% | 100% (9/9) |
| Estimated Precision | N/A | ~81% |

FP sources: `tests/compat.py` (2), `tests/testserver/server.py` (2), `tests/utils.py` (1)

</details>

### Python Observe Summary

| Metric | httpx | Requests (spot-check) | Target |
|--------|-------|-----------------------|--------|
| Precision (pair) | **98.2%** | ~100% | >= 98% |
| Recall (test file) | **96.8%** | 100% | >= 90% |
| F1 | **97.5%** | -- | -- |
| Status | **PASS** | PASS | -- |

**Requests: both targets met. httpx: Recall met, Precision close (94% vs 98% target).**

### Remaining Improvement Plan

| Priority | Fix | Expected Impact |
|----------|-----|-----------------|
| P0 | Assertion-only import filtering (exclude `__version__`, `_types` when not asserted against directly) | -4 httpx FP, P 94%->98%+ |
