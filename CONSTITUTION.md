# CONSTITUTION

## 1. One Sentence

Tests are executable specifications. exspec statically verifies whether tests are structurally sound as specifications -- fast, language-agnostic, zero LLM cost.

## 2. Goal / Non-Goals

### Goals
- Detect structural test smells that correlate with specification quality violations
- Provide actionable, low-noise lint output (BLOCK near-zero FP)
- Map test files to production files via static AST analysis (observe)
- Support 4 languages: Python, TypeScript, PHP, Rust

### Non-Goals
- Semantic validation (whether test name truly describes behavior)
- Code coverage measurement (use lcov/istanbul/coverage.py)
- LLM calls or AI-assisted analysis at runtime
- Cross-file semantic duplication detection (deferred to Tier 3)

## 3. Human vs AI Responsibilities

| Area | Human | AI |
|------|-------|----|
| Test architecture philosophy | Define 4 Properties | Implement as rules |
| Rule severity calibration | Final judgment on BLOCK/WARN/INFO | Propose based on FP rates |
| Ground truth creation | Audit and approve | Generate candidates |
| observe applicability scope | Decide ship criteria | Measure precision/recall |
| OSS release decisions | Approve version bumps | Prepare changelog |

## 4. Source of Truth (5-Layer)

| Layer | File | Role |
|-------|------|------|
| 0 | CONSTITUTION.md | Why (existence, principles) |
| 1 | AGENTS.md | What (tech stack, workflow) |
| 2 | CLAUDE.md | How (AI behavior, delegation) |
| 3 | docs/ | Detail (design, ADR, cycles) |
| 4 | Code | Truth (implementation is final) |

## 5. Change Policy

- CONSTITUTION.md changes require Human approval
- Record reasons in Git commit messages
- Upper layers take precedence when conflicts arise

## 6. Detection Philosophy

- exspec catches structural smells, not semantic quality
- Static AST analysis only -- no runtime, no LLM
- tree-sitter queries externalized (.scm files) for logic adjustment without recompilation
- observe uses multi-layer matching: filename convention (L1) + import tracing (L2)

## 7. Severity / Confidence Policy

| Level | Meaning | Confidence | FP Tolerance |
|-------|---------|------------|-------------|
| BLOCK | Almost certainly a test quality problem | High | Near-zero |
| WARN | Likely a problem, context-dependent | Medium | Acceptable |
| INFO | Worth considering, may be intentional | Lower | Tolerable |

exspec errs on the side of being quiet. A false positive at BLOCK level destroys trust.

## 8. Scope Boundaries

### Lint (exspec check)
- 4 languages, 17 rules, 3 severity levels
- Input: source files. Output: JSON/SARIF/terminal/ai-prompt
- No network, no LLM, no runtime dependency

### Observe (exspec observe)
- 4 languages: TypeScript, Python, Rust, PHP
- Static test-to-code mapping via AST (Layer 1: filename convention, Layer 2: import tracing)
- TypeScript: barrel/re-export resolution, tsconfig path alias, NestJS route extraction
- Python: dotted import resolution, `__init__.py` barrel
- Rust: `use crate::`/`use cratename::` resolution, workspace member aggregation, `pub mod` barrel
- PHP: PSR-4 namespace resolution
- Ship criteria (default): Precision >= 98%, Recall >= 90% (test file coverage)
- Ship criteria (PHP): Precision >= 98%, Recall >= 85% — structural ceiling at R=88.6% due to parent class inheritance, IoC resolution, and string literal patterns unreachable by static import tracing

### The 4 Properties

| Property | Definition |
|----------|-----------|
| What not How | Tests describe behavior, not implementation |
| Living Documentation | Tests are readable as specs without separate docs |
| Compositional | Each test verifies one responsibility |
| Single Source of Truth | One spec, one place |
