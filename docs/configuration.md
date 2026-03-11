# Configuration

exspec is configured via `.exspec.toml` in your project root.

## Generate a starter config

```bash
exspec init --lang python,typescript
```

## Full example

```toml
[general]
lang = ["python", "typescript"]

[rules]
disable = ["T004"]

[rules.severity]
T107 = "off"
T101 = "info"

[thresholds]
mock_max = 5
mock_class_max = 3
test_max_lines = 50
parameterized_min_ratio = 0.1

[paths]
test_patterns = ["tests/**", "**/*_test.*", "**/*.test.*"]
ignore = ["node_modules", ".venv", "vendor"]

[assertions]
custom_patterns = ["assertJsonStructure", "self.assertValid"]
```

## Sections

### `[general]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `lang` | string[] | auto-detect | Languages to analyze |

### `[rules]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `disable` | string[] | `[]` | Rule IDs to disable |

### `[rules.severity]`

Override the default severity for specific rules.

```toml
[rules.severity]
T107 = "off"     # disable rule entirely (equivalent to adding to disable)
T101 = "info"    # downgrade from warn to info
T001 = "block"   # explicit default (no-op)
```

| Value | Effect |
|-------|--------|
| `"off"` | Skip evaluation entirely (same as `disable`) |
| `"info"` | Report as INFO |
| `"warn"` | Report as WARN |
| `"block"` | Report as BLOCK (exit code 1) |

This is orthogonal to `--min-severity` / `[output] min_severity`: severity overrides change *evaluation*, while `min_severity` controls *display filtering*.

> **Caution**: Downgrading BLOCK rules (e.g. T001) removes the safety net for assertion-free tests. Use with care.

### `[thresholds]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mock_max` | int | 5 | T002: max mocks per test |
| `mock_class_max` | int | 3 | T002: max distinct mock classes |
| `test_max_lines` | int | 50 | T003: max lines per test |
| `parameterized_min_ratio` | float | 0.1 | T004: min ratio of parameterized tests |

### `[paths]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `test_patterns` | string[] | language defaults | Glob patterns for test files |
| `ignore` | string[] | `[]` | Directories to skip |

### `[assertions]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `custom_patterns` | string[] | `[]` | Custom assertion helper patterns for T001 |

Custom patterns use substring matching. A test function containing any of these patterns in its body will not trigger T001 (assertion-free), even if no standard assertion is found.

## Inline Suppression

Suppress specific rules per function with a comment directly above the test:

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

### Limitation

**TypeScript `describe()` scope**: Inline suppression applies to the **next** `test()`/`it()` call only. Placing `// exspec-ignore` above a `describe()` block does **not** suppress rules for all tests inside it. Suppress each test individually.
