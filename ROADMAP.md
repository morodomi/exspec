# exspec Roadmap

## Design Principles

1. **exspec is a static lint.** Not a template generator or documentation generator
2. **Solo-dev scope constraint.** Don't pursue 2+ large features in parallel
3. **Ship then iterate.** Don't over-polish before release -- but don't ship broken lint
4. **AI separation.** exspec outputs data; humans/AI decide. exspec itself never calls LLMs

## Now

Phase 8a (WARN/INFO FP reduction + remaining BLOCK FP)

Goal: Establish lint reliability as the foundation for all future directions.

| Task | Status |
|------|--------|
| #62 (P0): Python `^assert_` -> `^assert` (pytest 148 FPs) | TODO |
| #63 (P1): PHP `addToAssertionCount()` assertion recognition (Symfony 91 FPs) | TODO |
| #64 (P1): Exclude skip-only tests from T001 (Symfony 91 FPs) | TODO |
| WARN/INFO FP survey: T101/T102/T109 dogfooding (currently only BLOCK verified) | TODO |
| Re-dogfooding: pytest, symfony, major projects | TODO |

## Next

Phase 8b (`exspec observe` PoC)

Goal: Validate feasibility of static AST-only test-to-code mapping in 1-2 weeks.

- **Why**: Zero competitors in static approach (all use dynamic instrumentation). Asymmetric risk.
- **Scope**: 1 language (TypeScript), 1 project (NestJS), route/method test density report
- **Success**: 70%+ of major routes correctly mapped
- **Failure**: <50% precision, or AST limitations make practical mapping impossible

Phase 8c (branch on PoC result)

| If observe PoC succeeds | If observe PoC fails |
|------------------------|---------------------|
| observe MVP (multi-language) | Go language support |
| `exspec init` enhancement (framework detection + custom_patterns auto-suggest) | `exspec init` enhancement |
| GitHub Action + marketplace | GitHub Action |
| Note.com article "AI-era test observability tool" | Tier 3 rules (T201 spec-quality etc.) |

## Backlog

| Priority | Task | Trigger |
|----------|------|---------|
| P2 | T001 FP: Python nested test functions (#41) | Deferred from Phase 6 |
| P2 | T001 FP: return-wrapped Chai property (#52) | Deferred from Phase 6 |
| P2 | T201 spec-quality (advisory mode) | "I want semantic quality checks" |
| P3 | T203 AST similarity duplicate detection | "I want duplicate test detection" |
| Rejected | LSP/VSCode extension | Too early (low user count) |
| Rejected | Go language (before FP cleanup) | FP残存状態での横展開はリスク |

## Non-goals

- **Semantic validator**: exspec does not judge whether test names are meaningful or properties are sound
- **Coverage tool**: use lcov/istanbul/coverage.py for that
- **AI reviewer**: no LLM calls, zero API cost
- **Framework-specific linter**: rules should be language-agnostic where possible

## Completed Phases

| Phase | Content |
|-------|---------|
| 0 | SPEC.md + naming |
| 1 | Rust + tree-sitter scaffolding |
| 2 | Python + Tier 1 (T001-T003) |
| 3A | TypeScript + inline suppression + output polish |
| 3B | T004-T008 + .exspec.toml parsing |
| 3C | SARIF output + ProjectMetrics (MVP) |
| 4 | PHP support (PHPUnit/Pest) + dev-crew integration |
| 5A | Rust language support (cargo test) |
| 5B | Tier 2 rules T101-T105 (Python + TypeScript) |
| 5C | Tier 2 PHP/Rust expansion (T101-T105, T104 removed) |
| 5.5 | Gap rules T106-T109 |
| 6 | Release Hardening: dogfooding 13 projects / 4 langs / ~45k tests, FP fixes (#25-#66), severity review, T110 |
| 7 | OSS Release: LICENSE, README (#26, #27), CHANGELOG, crates.io v0.1.2 publish, GitHub Release |

## Explore: Test Observability (`exspec observe`)

4-AI brainstorm (Grok/Gemini/GPT/Claude, 2026-03-11). Scheduled for Phase 8b PoC.

**Idea**: Route/method-level test density visualization. "What is tested, where are the gaps?" Not a lint (no FAIL), purely descriptive hints.

**OSS gap**: No tool does static test-to-code mapping (all competitors use dynamic instrumentation), automatic test classification (happy/error/validation), or OpenAPI-free route coverage. All three are wide open.

**PoC plan (Phase 8b)**: TypeScript/supertest on NestJS. 1-2 week timebox. Success = 70%+ route mapping precision.

**Narrative**: "AI-generated code -> exspec lint for quality -> exspec observe for gap discovery" completes the story.

**Fallback (if PoC fails)**: Deepen lint with Go support, Tier 3 rules, GitHub Action. Observe idea shelved.

## Key Design Decisions

### T104 removal (Phase 5.5)

"Hardcoded-only" rule penalized DAMP-style tests. Replaced by T106 (duplicate-literal-assertion).

### T001 FP strategy (Phase 6, 4-AI consensus)

- T001 = "oracle-free" detection, not "assert-free"
- Oracle shapes: root (expect/assert) -> modifier chain -> terminal (call or property)
- Bounded vocabulary approach (not ML)
- Custom helpers: `.exspec.toml` `[assertions] custom_patterns` as escape hatch

### Severity philosophy (Phase 6)

- BLOCK: near-zero false positives required
- WARN: heuristic-based, context-dependent
- INFO: opinionated, may be intentional
- T107 demoted WARN->INFO (36-48% FP rate in dogfooding)
