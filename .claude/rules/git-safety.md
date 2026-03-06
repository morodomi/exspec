# Git Safety Rules

## Prohibited Actions (without explicit user approval)

- `git push --force` (especially to main/master)
- `git reset --hard`
- `git checkout .` / `git restore .`
- `git clean -f`
- `git branch -D`
- `--no-verify` on any git command

## Required Before Commit

1. All tests pass: `cargo test`
2. Static analysis clean: `cargo clippy -- -D warnings`
3. Format check: `cargo fmt --check`

## Branch Strategy

- `main`: stable, always passing
- Feature branches: `feat/<description>`, `fix/<description>`
