# Dogfooding Results

Date: 2026-03-09
exspec version: 0.1.0 (commit 5957cd0)

## Summary

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
./target/release/exspec --lang python --format json /tmp/django/tests
./target/release/exspec --lang python --format json /tmp/pytest/testing
./target/release/exspec --lang php --format json /tmp/symfony
```
