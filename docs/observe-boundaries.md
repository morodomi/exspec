# observe: Failure Boundaries

This document catalogs the known failure boundaries of `exspec observe` -- cases where the static test-to-code mapping produces false negatives (FN). Each boundary is verified by a boundary specification test in `observe.rs` (`boundary_b{N}_*` tests).

## Summary

| # | Boundary | Root Cause | Impact | Fixability |
|---|----------|-----------|--------|------------|
| B1 | Namespace re-export | `re_export.scm` lacks `namespace_export` pattern | FN | Medium (query addition) |
| B2 | Cross-package barrel import | ~~Non-relative paths excluded from import tracing~~ **Resolved in B2** (yarn/pnpm symlink follow) | ~~FN~~ Resolved | node_modules symlink resolution (Layer 2c) |
| B3 | tsconfig path alias | ~~Same as B2~~ **Resolved in 8c-3** | ~~FN~~ Resolved | tsconfig.json paths parsing |
| B4 | Interface/enum filter side-effect | ~~`is_non_sut_helper` filters primary test targets~~ **Resolved in 8c-4** (direct import only) | ~~FN~~ Partially resolved | Context-aware filtering with `is_known_production` |
| B5 | Dynamic import | `import_mapping.scm` only captures static `import` statements | FN | Low (rare in test code) |
| B6 | Monorepo scan_root boundary | Resolved paths outside scan_root have no production file match | FN | By design |

## B1: Namespace re-export (`export * as Ns from`)

**Syntax**: `export * as Validators from './validators'`

**Why it fails**: `re_export.scm` handles two patterns:
1. Named re-export: `export { Foo } from './module'`
2. Wildcard re-export: `export * from './module'`

Namespace re-export (`export * as Ns from`) produces a `namespace_export` AST node, which neither pattern matches.

**Impact**: When a barrel file uses namespace grouping, all symbols behind that namespace become invisible to import tracing. This is uncommon in NestJS but appears in some utility packages.

**Tests**: `boundary_b1_ns_reexport_captured_as_wildcard`, `boundary_b1_ns_reexport_mapping_miss` (both inverted to TP after fix)

**Fix path**: Add a third pattern to `re_export.scm` targeting `namespace_export` nodes. **Decision**: Treat as opaque wildcard (`wildcard: true`). Ns.Foo -> Foo resolution is out of scope — wildcard covers all public symbols in the target module, avoiding FN with minimal FP risk (barrel precision 99.4%).

## B2: Cross-package barrel import (non-relative path) -- Resolved in B2

**Syntax**: `import { Foo } from '@org/common'`

**Root cause**: `extract_imports` filters out any module specifier that does not start with `./` or `../`. Package-scoped imports (`@org/common`, `@nestjs/common`) are indistinguishable from third-party dependencies without `node_modules` resolution.

**Resolution (Phase B2)**: Layer 2c follows yarn/pnpm workspace symlinks in `node_modules`. When `scan_root/node_modules/@org/common` is a symlink (as created by yarn/pnpm workspaces), it is canonicalized to the real package directory. All production files under that directory that appear in `production_files` are then mapped to the test file. tsconfig aliases (Layer 2b) take priority -- if a specifier is already resolved by tsconfig, Layer 2c is skipped.

**Scope constraint**: `production_files` must include the cross-package file for this to work. If only scan_root files are passed, B6 applies.

**Remaining limitations**:
- npm (real directories in node_modules, not symlinks) is not resolved -- only symlinks
- package.json `main`/`exports` parsing is not supported (barrel fallback only)
- Windows not supported (symlink creation requires Unix APIs)

**Tests**: `boundary_b2_non_relative_import_skipped` (unchanged -- extract_imports still filters non-relative), `boundary_b2_cross_pkg_symlink_resolved` (TP), `b2_sym_01_symlink_followed`, `b2_sym_02_real_directory_returns_none`, `b2_sym_03_nonexistent_returns_none`, `b2_map_02_tsconfig_alias_priority`, `b2_multi_01_two_test_files_both_mapped`

## B3: tsconfig path alias (`@app/*`) -- Resolved in 8c-3

**Syntax**: `import { FooService } from '@app/services/foo.service'`

**Root cause**: `@app/` does not start with `./` or `../`, so `extract_imports` skips it. The path alias is defined in `tsconfig.json` (e.g., `"@app/*": ["src/*"]`).

**Resolution (Phase 8c-3)**: `tsconfig.rs` module parses `tsconfig.json` `compilerOptions.paths` + `baseUrl`, resolves aliases to absolute paths, and feeds them into the existing file resolution pipeline. Supports `extends` chains (relative paths only, max 3 levels) and auto-discovers `tsconfig.json` by walking up from scan_root.

**Remaining limitations**:
- JSON5 tsconfig (comments, trailing commas) not supported -- standard JSON only
- `extends` referencing npm packages (`@tsconfig/node18`) ignored
- `baseUrl`-only resolution (without `paths`) not supported
- B2 (cross-package barrel via `node_modules`) remains unresolved

**Tests**: `boundary_b3_tsconfig_alias_not_resolved` (without tsconfig -- FN by design), `boundary_b3_tsconfig_alias_resolved` (with tsconfig -- resolved)

## B4: Interface/enum filter side-effect -- Partially resolved in 8c-4

**Syntax**: `import { RouteParamtypes } from './route-paramtypes.enum'`

**Original problem**: `is_non_sut_helper` filters files matching `*.enum.*`, `*.interface.*`, and `*.exception.*`. When a test directly imports an enum or interface listed in `production_files`, the filter created a false negative.

**Resolution (Phase 8c-4)**: `is_non_sut_helper` now accepts `is_known_production: bool`. When the resolved file is a known production file (`canonical_to_idx.contains_key()`), the suffix filter is bypassed. The `is_type_definition_file` function was extracted to encapsulate the suffix check.

**Remaining limitation**: Barrel resolution path (`resolve_barrel_exports_inner`) passes `is_known_production=false` because it lacks access to the production file index. Enum/interface files re-exported through barrels may still be filtered before reaching `collect_matches`. Phase 11 analysis showed this affects only 2 FN (http.exception.spec.ts). Barrel fix was rejected because `export *` barrels would resolve 20+ files per barrel, likely increasing FP more than reducing FN. The dominant FN source is B2 (cross-package), not B4.

**Tests**: `boundary_b4_enum_primary_target_filtered` (TP), `boundary_b4_interface_primary_target_filtered` (TP)

**Fix path adopted**: Production file membership check (`canonical_to_idx.contains_key`). Test name context analysis was not adopted (outside static analysis scope).

## B5: Dynamic import (`import()`)

**Syntax**: `const m = await import('./user.service')`

**Why it fails**: `import_mapping.scm` only captures static `import { ... } from '...'` statements. Dynamic `import()` expressions produce a `call_expression` AST node with `import` as the function, which the query does not match.

**Impact**: Rare in test files. Most test frameworks use static imports. Dynamic imports appear occasionally in lazy-loading tests or module isolation patterns.

**Tests**: `boundary_b5_dynamic_import_not_extracted`

**Fix path**: Add a `call_expression` pattern to `import_mapping.scm` targeting `import(specifier)`. Low priority due to rarity.

## B6: Monorepo scan_root boundary

**Syntax**: `import { Shared } from '../../common/src/shared'` (where `shared.ts` is outside scan_root)

**Why it fails**: `map_test_files_with_imports` only considers files within `production_files` (which are collected from scan_root). A relative import that resolves to a file outside scan_root will resolve successfully at the filesystem level, but there is no matching entry in `canonical_to_idx` to map it to.

**Impact**: By design. scan_root defines the analysis boundary. Files outside it are not part of the production codebase being analyzed.

**Tests**: `boundary_b6_import_outside_scan_root`

**Fix path**: None needed. This is intentional scoping. Users should set scan_root to the monorepo root if they want cross-package visibility (at the cost of including all packages).

## Applicability Scope

Based on these boundaries, observe is most reliable for:

- **Single-package TypeScript projects** (no B2/B3/B6 impact)
- **Projects using relative imports or tsconfig path aliases** (B2 resolved, B3 resolved)
- **Projects with standard barrel patterns** (`export { X } from` or `export * from`, no B1 impact)

- **yarn/pnpm monorepo workspaces** with cross-package imports (B2 resolved via symlink follow)

Observe is **less reliable** for:
- **npm monorepo workspaces** with real-directory node_modules (not symlinks) for cross-package deps (B6)
- **Projects heavy on namespace re-exports** (B1)
- **Projects where enums/interfaces are re-exported through barrels** (B4 partial)
