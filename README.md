# exspec

Static analyzer for test design quality. Verifies that tests function as executable specifications -- fast, language-agnostic, zero LLM cost.

> **Public beta** (v0.4.4). Dogfooded across 11 projects / 4 languages / ~40,000 tests. Not production-ready -- rule IDs, severity levels, and config format may change.

## Why exspec?

| Tool | Focus | exspec's Niche |
|------|-------|----------------|
| SonarQube | Code coverage | Test **design** quality |
| Mutation testing | Fault detection (slow) | **Static** analysis (fast) |
| similarity | Duplicate detection | Specification quality |

exspec checks whether your tests are well-designed *specifications*, not just code that runs. It enforces 4 properties: **What not How**, **Living Documentation**, **Compositional**, **Single Source of Truth**. See [docs/philosophy.md](docs/philosophy.md) for the full rationale.

Validated against 11 real-world OSS projects (~40,000 tests across Python, TypeScript, PHP, Rust). See [Validation](#validation) below.

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

Default output (`ai-prompt` format) includes fix guidance for AI agents:

```markdown
# exspec -- Test Quality Report

## BLOCK (must fix)

### tests/test_example.py:5

**T001**: assertion-free: test has no assertions

> This test does not express a specification -- it only verifies "no crash."
> Ask: what observable outcome should this function guarantee?
> Assert the return value, state change, or side effect instead.

## WARN (should fix)

### tests/test_example.py:20

**T002**: mock-overuse: 6 mocks (6 classes), threshold: 5 mocks / 3 classes

> Too many mocks can make the test fragile and coupled to implementation.
> Consider using fewer mocks and testing through real collaborators where possible.
> Extract the core logic into a pure function that can be tested without mocks.

Score: BLOCK 1 | WARN 1 | INFO 0 | PASS 8
```

Use `--format terminal` for concise human-readable output:

```
exspec v0.1.2 -- 8 test files, 10 test functions
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
| Tier 2 | T101-T110 | 0 WARN, 8 INFO, 1 OFF |

See [docs/SPEC.md](docs/SPEC.md) for the full rule reference.

## Observe

Static test-to-code mapping. Answers "what is tested, where are the gaps?" without running tests.

Supports **TypeScript, Python, Rust, PHP**.

```bash
exspec observe --lang typescript .    # TypeScript (NestJS, barrel resolution)
exspec observe --lang python .        # Python (dotted imports)
exspec observe --lang rust .          # Rust (use crate::, workspace aggregation)
exspec observe --lang php .           # PHP (PSR-4 namespace resolution)
exspec observe --lang rust --format json .  # JSON for CI
```

### What it does

1. **File mapping**: Maps test files to production files via filename convention (Layer 1) and import tracing (Layer 2)
2. **Route coverage** (TypeScript/NestJS): Detects controller routes and shows which have test coverage
3. **Gap detection**: Lists unmapped production files (potential test gaps)

### Observe flags

| Flag | Default | Description |
|------|---------|-------------|
| `--l1-exclusive` | off | Suppress L2 for L1-matched test files |
| `--no-fan-out-filter` | off | Disable fan-out threshold filter |
| `--format json` | terminal | JSON output for CI |

### Dogfooding results

| Project | Lang | Prod | Mapped | Precision | Status |
|---------|------|------|--------|-----------|--------|
| NestJS | TypeScript | 1279 | 466 (36%) | 100% | stable |
| FastAPI | Python | 620 | 122 (20%) | ~100% | stable |
| Django | Python | 2266 | 381 (17%) | ~100% | stable |
| tokio | Rust | 495 | 50 (10%) | 100% | experimental (R < 90%) |
| Laravel | PHP | 1951 | 973 (50%) | ~100% | stable (R=88.6%) |
| Symfony | PHP | 7937 | 4117 (52%) | ~96% | stable |

See [docs/dogfooding-results.md](docs/dogfooding-results.md) for full details.

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

## Output Formats

| Format | Default | Description |
|--------|---------|-------------|
| `ai-prompt` | Yes | Structured markdown with fix guidance per rule. Designed for AI agents (Claude Code, Copilot, etc.) |
| `terminal` | | Concise one-line-per-diagnostic. For humans |
| `json` | | Machine-readable with full metadata |
| `sarif` | | SARIF v2.1.0 for GitHub Code Scanning |

```bash
exspec .                        # ai-prompt (default)
exspec --format terminal .      # human-readable
exspec --format json .          # machine-readable
exspec --format sarif .         # GitHub Code Scanning
```

## Known Constraints

- **Rust macro-generated tests**: Invisible to tree-sitter. `assert_*!` macros are auto-detected; other custom macros need `custom_patterns`
- **TypeScript T107**: Intentionally disabled (high false positive rate in dogfooding)
- **Helper delegation**: Project-local assertion helpers need `custom_patterns` config

See [docs/known-constraints.md](docs/known-constraints.md) for details, workarounds, and dogfooding data.

## Validation

Dogfooded across 11 projects (~40k tests):

| Project | Language | Tests | BLOCK | Primary Cause |
|---------|----------|-------|-------|---------------|
| exspec (self) | Rust | 10 | 0 | N/A |
| requests | Python | 339 | 10 | helper delegation |
| fastapi | Python | 2,155 | 15 | helper delegation |
| nestjs | TypeScript | 2,679 | 13 | helper delegation |
| laravel | PHP | 11,044 | 222 | helper delegation |
| symfony | PHP | 17,204 | 616 | helper delegation |
| ripgrep | Rust | 16 | 0 | ~330 tests in macros (not detected) |
| tokio | Rust | 1,594 | 257 | select! token_tree |
| clap | Rust | 1,455 | 71 | helper delegation |
| django | Python | 1,048 | 22 | helper delegation |

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
