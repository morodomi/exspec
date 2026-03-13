---
feature: lint-reliability
cycle: 8a-3-warn-info-fp-fixes
phase: DONE
complexity: standard
test_count: 12
risk_level: low
created: 2026-03-13 15:41
updated: 2026-03-13 16:45
---

# Phase 8a-3: WARN/INFO FP Fixes

## Scope Definition

### In Scope
- [ ] T101 default severity WARN->INFO (#69)
- [ ] T102 default severity WARN->INFO (#70)
- [ ] T108 default severity WARN->INFO (#72)
- [ ] T106 default OFF via Config::default().disabled_rules (#73)
- [ ] Config merge: defaults -> disable -> severity (precedence fix)
- [ ] T003 threshold: maintain 50 lines, document rationale (#71)
- [ ] Update tests for severity changes + precedence
- [ ] Update docs (dogfooding-results.md, configuration.md)
- [ ] Update ROADMAP.md with 8a-3 completion

### Out of Scope
- T105 (8% FP - no action needed)
- T109 (50% FP - no action needed per survey)
- T003 threshold change (maintaining 50 lines; projects use .exspec.toml override)
- T106 rule deletion (keeping for future improvement)
- 矛盾設定時のwarning出力 (別Issue)

### Files to Change (target: 10 or less)
- crates/core/src/rules.rs (edit) - T101/T102/T108 severity defaults, T106 default OFF
- crates/core/src/config.rs (edit) - disabled_rules merge logic (defaults -> disable -> severity)
- tests/ (edit) - severity expectation updates + precedence tests
- docs/dogfooding-results.md (edit) - results reflection
- docs/configuration.md (edit) - document changes
- ROADMAP.md (edit) - 8a-3 completion record

## Environment

### Scope
- Layer: Backend
- Plugin: N/A (Rust native)
- Risk: 15 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: existing
- serde/toml: existing

## Context & Dependencies

### Reference Documents
- [docs/dogfooding-results.md] - FP rate data from 8a-2 survey
- [docs/SPEC.md] - Rule specifications
- [docs/configuration.md] - Config escape hatch documentation

### Dependent Features
- Phase 8a-2 survey results (completed)

### Related Issues/PRs
- Issue #69: T101 WARN->INFO
- Issue #70: T102 WARN->INFO
- Issue #71: T003 threshold decision
- Issue #72: T108 WARN->INFO
- Issue #73: T106 default OFF decision

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: T101 default severity is Info (not Warn)
- [x] TC-02: T102 default severity is Info (not Warn)
- [x] TC-03: T108 default severity is Info (not Warn)
- [x] TC-04: T106 is disabled by default in Config::default()
- [x] TC-05: T106 re-enabled via `severity = "info"` (removes from disabled_rules + adds override)
- [x] TC-06: T101/T102/T108 can still be overridden to WARN/BLOCK via config
- [x] TC-07: Existing severity override tests still pass (backward compat)
- [x] TC-08: Self-dogfooding: cargo run -- --lang rust . produces BLOCK 0
- [x] TC-09: default disabled T106 + `rules.disable = ["T106"]` -> appears exactly once
- [x] TC-10: default disabled T106 + `severity = "off"` -> appears exactly once
- [x] TC-11: `disable = ["T106"]` + `severity = "info"` -> enabled INFO (severity wins)
- [x] TC-12: default OFF + `disable = ["T106"]` + `severity = "off"` -> disabled (single entry)

## Implementation Notes

### Goal
Reduce false positive noise by demoting 3 rules (T101/T102/T108) from WARN to INFO and disabling T106 by default. T003 threshold maintained at 50 lines.

### Background
Phase 8a-2 survey measured FP rates across 13 real projects (~45k tests). Results:
- T101 (how-not-what): 47% FP - too noisy for WARN
- T102 (fixture-sprawl): 80% FP - too noisy for WARN
- T108 (wait-and-see): 93% FP - too noisy for WARN
- T106 (duplicate-literal): 93% FP at INFO - disable by default
- T003 (test-too-long): 95% FP in fastapi only; 1-4% in other projects - keep 50 lines

### Design Approach

**T101/T102/T108**: Change the third argument of `effective_severity()` from `Severity::Warn` to `Severity::Info`.

**T106 default OFF**: Add "T106" to `Config::default().disabled_rules`.

**T003**: No code change. fastapi's high FP is project-specific (`.exspec.toml`で対応可能).

**Config merge precedence**:
1. Start from built-in defaults (`Config::default().disabled_rules`)
2. Apply `[rules.disable]` (additive, dedup)
3. Apply `[rules.severity]`
4. `severity = "off"` -> add to disabled (dedup)
5. `severity = "info" | "warn" | "block"` -> remove from disabled + record override

Why: 現在の `From<ExspecConfig> for Config` は `disabled_rules` を user config からのみ構築し、`defaults.disabled_rules` を無視している。他フィールド (`mock_max` 等) は全て defaults をフォールバックに使っており、`disabled_rules` だけが不整合。severity が最終決定権を持つのは `"off"` = 無効化の既存動作と対称的。

**Re-enable via severity**: `T106 = "info"` と書いたユーザーがT106を無効のままにしたいはずがない。`rules.enable` は `severity` と概念的に冗長 (Option B却下)。

### Key Code Changes

`crates/core/src/config.rs` - `From<ExspecConfig> for Config`:
```rust
// Before:
let mut disabled_rules: Vec<RuleId> =
    ec.rules.disable.iter().map(|s| RuleId::new(s)).collect();

// After:
let mut disabled_rules = defaults.disabled_rules;
for rule_id in &ec.rules.disable {
    if !disabled_rules.iter().any(|r| r.0 == *rule_id) {
        disabled_rules.push(RuleId::new(rule_id));
    }
}
```

severity loop に re-enable 追加:
```rust
Ok(sev) => {
    severity_overrides.insert(rule_id.clone(), sev);
    disabled_rules.retain(|r| r.0 != *rule_id);  // re-enable
}
```

`crates/core/src/rules.rs` - 3箇所の severity default 変更:
- T101: `effective_severity(config, "T101", Severity::Warn)` -> `Severity::Info`
- T102: `effective_severity(config, "T102", Severity::Warn)` -> `Severity::Info`
- T108: `effective_severity(config, "T108", Severity::Warn)` -> `Severity::Info`

`crates/core/src/rules.rs` - `Config::default()`:
```rust
disabled_rules: vec![RuleId::new("T106")],
```

## Progress Log

### 2026-03-13 15:41 - KICKOFF
- Cycle doc created
- Decisions: T003 maintain 50 lines, T106 default OFF, Option A (severity as final arbiter)
- 12 test cases identified
- Review findings (5件) resolved: config merge precedence defined, dedup strategy agreed, doc sweep deferred

### 2026-03-13 16:24 - REFACTOR
- Extracted `count_disabled()` test helper (5 call sites deduplicated)
- Added comment on T106 default disabled rationale
- Fixed failing `load_config_valid_file` test (disabled_rules count 2->3)
- Verification Gate: 706 tests PASS, clippy 0, fmt OK, self-dogfooding BLOCK 0

### 2026-03-13 16:32 - REVIEW
- review(code) score:25 verdict:PASS
- Security: PASS, Correctness: PASS (2 WARN non-blocking)
- WARN-1: rules.disable not validated against KNOWN_RULE_IDS (pre-existing)
- WARN-2: disabled_rules.len() == 3 in main.rs fragile (low priority)
- Phase completed

### 2026-03-13 16:45 - COMMIT
- Phase 8a-3 complete: T101/T102/T108 WARN->INFO, T106 default OFF, config merge precedence
- Public docs updated: README, STATUS, SPEC, severity-review, configuration
- Phase completed

---

## Next Steps

1. [Done] KICKOFF
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT <- Complete
