# exspec

Static analyzer for test design quality. Verifies that tests function as executable specifications -- fast, language-agnostic, zero LLM cost.

> **Public beta** (v0.1.1). Dogfooded across 10 projects / 4 languages / ~25,000 tests. Not production-ready -- rule IDs, severity levels, and config format may change.

## Why exspec?

| Tool | Focus | exspec's Niche |
|------|-------|----------------|
| SonarQube | Code coverage | Test **design** quality |
| Mutation testing | Fault detection (slow) | **Static** analysis (fast) |
| similarity | Duplicate detection | Specification quality |

exspec checks whether your tests are well-designed *specifications*, not just code that runs. It enforces 4 properties: **What not How**, **Living Documentation**, **Compositional**, **Single Source of Truth**. See [docs/philosophy.md](docs/philosophy.md) for the full rationale.

Validated against 10 real-world OSS projects (~25,000 tests across Python, TypeScript, PHP, Rust). See [Validation](#validation) below.

## Install

```bash
cargo install exspec
```

Or install from source:

```bash
cargo install --git https://github.com/morodomi/exspec.git
```

## Quick Start

```bash
exspec .                              # Analyze current directory
exspec init --lang python,typescript  # Generate .exspec.toml
exspec --lang python .                # Analyze specific language
exspec --strict .                     # WARN also fails
```

Example output:

```
exspec v0.1.1 -- 8 test files, 10 test functions
BLOCK tests/test_example.py:5 T001 assertion-free: test has no assertions
WARN  tests/test_example.py:20 T002 mock-overuse: 6 mocks (6 classes), threshold: 5 mocks / 3 classes
Score: BLOCK 1 | WARN 1 | INFO 0 | PASS 8
```

## Supported Languages

| Language | Test Frameworks | Since |
|----------|----------------|-------|
| Python | pytest | v0.1.0 |
| TypeScript | Jest, Vitest | v0.1.0 |
| PHP | PHPUnit, Pest | v0.1.0 |
| Rust | cargo test | v0.1.0 |

Each language has specific detection patterns and known gaps. See [docs/languages/](docs/languages/) for details.

## Check Rules

17 rules across 2 tiers. **Tier 1** catches structural issues (assertion-free tests, mock overuse, giant tests). **Tier 2** catches design smells (implementation coupling, fixture sprawl, undescriptive names).

| Tier | Rules | Levels |
|------|-------|--------|
| Tier 1 | T001-T008 | 1 BLOCK, 3 WARN, 4 INFO |
| Tier 2 | T101-T110 | 3 WARN, 6 INFO |

See [docs/SPEC.md](docs/SPEC.md) for the full rule reference.

## Gradual Adoption

Start with Tier 1 only. Disable Tier 2 until your codebase is clean:

```toml
# .exspec.toml
[rules]
disable = ["T101", "T102", "T103", "T105", "T106", "T107", "T108", "T109", "T110"]
```

Once Tier 1 passes, enable Tier 2 rules one at a time. Use inline suppression for known exceptions:

```python
# exspec-ignore: T002
def test_complex_integration():
    ...
```

For projects with custom assertion helpers, add them to avoid T001 false positives:

```toml
[assertions]
custom_patterns = ["assertJsonStructure", "self.assertValid"]
```

### Tuning Severity

Two independent mechanisms control what you see:

- **`[rules.severity]`** changes how a rule is *evaluated*. `T107 = "off"` disables the rule entirely; `T101 = "info"` downgrades it from WARN to INFO.
- **`--min-severity`** controls *display filtering*. `--min-severity warn` hides INFO diagnostics from the output but does not change evaluation or exit codes.

```toml
# .exspec.toml
[rules.severity]
T107 = "off"      # disable T107 entirely
T101 = "info"     # downgrade T101 to informational

[output]
min_severity = "warn"  # hide INFO in terminal/JSON output
```

```bash
exspec --min-severity warn .   # CLI equivalent of [output] min_severity
```

## CI Integration

```yaml
- run: cargo install exspec
- run: exspec .
```

exspec exits 1 on BLOCK violations, 0 otherwise. Use `--strict` to also fail on WARN. SARIF output is available for GitHub Code Scanning. See [docs/ci.md](docs/ci.md) for full examples.

## Known Constraints

- **Rust macro-generated tests**: Invisible to tree-sitter. Custom assertion macros need `custom_patterns`
- **TypeScript T107**: Intentionally disabled (high false positive rate in dogfooding)
- **Helper delegation**: Project-local assertion helpers need `custom_patterns` config

See [docs/known-constraints.md](docs/known-constraints.md) for details, workarounds, and dogfooding data.

## Validation

Dogfooded across 10 real-world projects:

| Project | Language | Tests | Result |
|---------|----------|-------|--------|
| exspec (self) | Rust | 51 | 0 FP |
| requests | Python | 339 | ~20% FP |
| fastapi | Python | 2,121 | 21% FP |
| pydantic | Python | ~2,500 | 43 TP (benchmark), 15 FP (helper/nested) |
| vitest | TypeScript | 3,120 | Remaining = project-local helpers |
| nestjs | TypeScript | 2,675 | 0% FP (17 remaining = all TP) |
| laravel | PHP | 10,790 | Remaining = helper delegation |
| ripgrep | Rust | ~346 | 330 tests in macros (not detected) |
| tokio | Rust | 1,582 | 33.8% FP (custom assert macros) |
| clap | Rust | 1,455 | 41.3% FP (assert_data_eq! macro + helper delegation) |

Full results: [docs/dogfooding-results.md](docs/dogfooding-results.md)

## Documentation

| Doc | Content |
|-----|---------|
| [docs/languages/](docs/languages/) | Language-specific detection, assertions, known gaps |
| [docs/known-constraints.md](docs/known-constraints.md) | Limitations, workarounds, dogfooding data |
| [docs/configuration.md](docs/configuration.md) | `.exspec.toml` reference, inline suppression |
| [docs/ci.md](docs/ci.md) | CI setup, SARIF, exit codes, score semantics |
| [docs/philosophy.md](docs/philosophy.md) | Design rationale, 4 properties |
| [docs/dogfooding-results.md](docs/dogfooding-results.md) | Full dogfooding results |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## Contributing

1. Fork the repository
2. Create a feature branch
3. Follow TDD: write tests first
4. Submit a pull request

## License

MIT
