# exspec

Static analyzer for test design quality. Verifies that tests function as executable specifications -- fast, language-agnostic, zero LLM cost.

## Why exspec?

| Tool | Focus | exspec's Niche |
|------|-------|----------------|
| SonarQube | Code coverage | Test **design** quality |
| Mutation testing | Fault detection (slow) | **Static** analysis (fast) |
| similarity | Duplicate detection | Specification quality |

exspec checks whether your tests are well-designed *specifications*, not just code that runs.

## Install

```bash
cargo install exspec
```

## Quick Start

```bash
# Analyze current directory
exspec .

# Initialize config
exspec init --lang python,typescript

# Analyze with specific language
exspec --lang python .

# Strict mode (WARN also fails)
exspec --strict .
```

## Output

```
exspec v0.1.0 -- 42 test files, 187 test functions

BLOCK tests/test_api.py:78       T001 assertion-free
WARN  tests/test_predict.py:45   T002 mock-overuse (6 mocks across 4 classes)
WARN  tests/test_feature.py:120  T003 giant-test (73 lines)
INFO  tests/test_transform.py    T008 no-contract (no Pydantic/Pandera)

Metrics:
  Mock density:      2.3/test (avg), 4 distinct classes/test (max)
  Parameterized:     15% (28/187)
  PBT usage:         12% (23/187)
  Assertion density: 1.8/test (avg)
  Contract coverage: 8% (15/187)

Score: BLOCK 1 | WARN 2 | INFO 1 | PASS 183
```

## Output Formats

```bash
exspec .                      # Terminal (default)
exspec --format json .        # JSON
exspec --format sarif .       # SARIF (GitHub Code Scanning)
exspec --format ai-prompt .   # AI review prompts (Tier 3)
```

## Supported Languages

| Language | Test Frameworks | Phase |
|----------|----------------|-------|
| Python | pytest | MVP |
| TypeScript | Jest, Vitest | MVP |
| PHP | PHPUnit, Pest | v0.2 |
| Rust | cargo test | v0.2 |
| Dart | flutter_test | v1.0 (best-effort) |

## Check Rules

### Tier 1 (MVP)

| ID | Rule | Level | Description |
|----|------|-------|-------------|
| T001 | assertion-free | BLOCK | Test has no assertions |
| T002 | mock-overuse | WARN | Too many mocks/stubs/spies |
| T003 | giant-test | WARN | Test function exceeds line limit |
| T004 | no-parameterized | INFO | Low parameterized test ratio |
| T005 | pbt-missing | INFO | No property-based testing |
| T006 | low-assertion-density | WARN | assertions/tests < 1.0 |
| T007 | test-source-ratio | INFO | Test file to source file ratio |
| T008 | no-contract | INFO | No schema validation in tests |

### Tier 2 (v0.2)

| ID | Rule | Level |
|----|------|-------|
| T101 | how-not-what | WARN |
| T102 | fixture-sprawl | WARN |
| T103 | missing-error-test | INFO |
| T104 | hardcoded-only | INFO |
| T105 | deterministic-no-metamorphic | WARN |

### Tier 3 (v1.0) -- AI Prompt Generation

For semantic checks that require LLM reasoning, exspec generates review prompts instead of making judgments.

## Configuration

Create `.exspec.toml` in your project root:

```toml
[general]
lang = ["python", "typescript"]

[rules]
disable = ["T004"]

[thresholds]
mock_max = 5
mock_class_max = 3
test_max_lines = 50
parameterized_min_ratio = 0.1

[paths]
test_patterns = ["tests/**", "**/*_test.*", "**/*.test.*"]
ignore = ["node_modules", ".venv", "vendor"]
```

Or generate one:

```bash
exspec init --lang python,typescript
```

## Inline Suppression

Suppress specific rules per function:

```python
# exspec-ignore: T002
def test_complex_integration():
    ...
```

```typescript
// exspec-ignore: T002, T003
test('complex integration', () => {
  ...
});
```

## Architecture

Built with Rust + tree-sitter for fast, language-agnostic AST analysis. Detection queries are externalized as `.scm` files, allowing logic adjustments without recompilation.

```
crates/
  core/           Language-independent analysis engine
  lang-python/    Python queries/*.scm
  lang-typescript/ TypeScript queries/*.scm
  cli/            CLI entry point
```

## dev-crew Integration

exspec runs as a zero-cost quality gate in the TDD RED phase:

```
RED Phase (test written)
  └── exspec --format json --strict {test_files}
      ├── exit 0 → proceed to GREEN
      └── exit 1 → feedback to fix tests
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No BLOCK violations (default) / No WARN+ (--strict) |
| 1 | BLOCK violations found (default) / WARN+ found (--strict) |

## Contributing

1. Fork the repository
2. Create a feature branch
3. Follow TDD: write tests first
4. Submit a pull request

## License

MIT
