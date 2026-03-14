# State Ownership Rules

Defines who owns which state and when mutations are permitted.

## Source of Truth Domains

### Plan File — IMMUTABLE after approve

Once the plan is approved, the plan file must not be modified.
It is a read-only contract for the rest of the cycle.

### Cycle Doc — APPEND-ONLY log + structured frontmatter

Body text (Progress Log, Test List transitions) is append-only.
Never rewrite or delete existing log entries.
Frontmatter fields may be updated per the permissions table below.

### Source Files — SINGLE SOURCE OF TRUTH for implementation

All implementation decisions are reflected in source files.
Cycle doc records what was done; source files define what is.

## Frontmatter Update Permissions

| Phase | Allowed Updates |
|-------|----------------|
| sync-plan | Initialize all frontmatter fields (feature, phase, complexity, test_count, risk_level, created, updated) |
| red | complexity, test_count, phase, updated |
| green | phase, updated |
| refactor | phase, updated |
| review | Body log only (no frontmatter changes except phase, updated) |
| commit | phase, updated |
