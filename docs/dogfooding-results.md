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
| nestjs (post-#51) | TypeScript | 2675 | ~17 (est.) | -- | return wrapper, helper delegation, done() callback |

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
```
