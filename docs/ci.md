# CI Integration

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No BLOCK violations (default) / No WARN+ (--strict) |
| 1 | BLOCK violations found (default) / WARN+ found (--strict) |

By default, only BLOCK violations (T001) cause a non-zero exit. Use `--strict` to also fail on WARN.

## GitHub Actions (SARIF)

Upload results to GitHub Code Scanning for inline PR annotations:

```yaml
# .github/workflows/exspec.yml
name: Test Quality
on: [pull_request]
jobs:
  exspec:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install --git https://github.com/morodomi/exspec.git
      - run: exspec --format sarif . > results.sarif
      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
```

## Simple CI (exit code)

For any CI system:

```yaml
- run: cargo install --git https://github.com/morodomi/exspec.git
- run: exspec .
```

Use `--strict` to also fail on WARN:

```yaml
- run: exspec --strict .
```

Use `--min-severity` to reduce noise in CI logs without changing exit behavior:

```yaml
- run: exspec --min-severity warn .   # hide INFO, still exit 1 on BLOCK
```

`--min-severity` is a display filter only. BLOCK violations still cause a non-zero exit regardless of the filter setting.

## Recommended: start without failing

For existing codebases, start by running exspec without failing the build. Review the output and configure `.exspec.toml` before enforcing:

```yaml
- run: exspec . || true   # observe, don't block
```

Once you've tuned thresholds and added `custom_patterns` for your assertion helpers, remove `|| true`.

## Output Formats

```bash
exspec .                      # Terminal (default)
exspec --format json .        # JSON (for programmatic consumption)
exspec --format sarif .       # SARIF (GitHub Code Scanning)
```

## Score Semantics

The `PASS` count in the output score line represents test functions without violations:

```
PASS = total_test_functions - unique_violated_functions
```

- A test function with **multiple violations** counts as **1** violated function, not 2
- **File-level diagnostics** (T004-T008) and **project-level diagnostics** (T007) do **not** reduce the PASS count
- Uniqueness is determined by `(file, line)` pair
