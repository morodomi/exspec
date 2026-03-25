# Ground Truth: Rust observe -- clap

Repository: clap-rs/clap
Commit: 70f3bb3
Auditor: Human + AI (stratified audit)
Date: 2026-03-25

## Methodology

1. exspec observe output collected (`observe --lang rust --format json`)
2. 91-file GT scope defined across 6 test strata
3. 40-file stratified sample selected for detailed audit (S1-S5)
4. Each test file audited: use statements, test function names, assertion targets

## Scope Exclusions

- tests/builder/utils.rs, tests/builder/main.rs: helper files, not test files
- tests/derive/utils.rs, tests/derive/main.rs: helper files
- tests/derive_ui/*.rs (27 files): trybuild compile-check tests -- no behavioral mapping
- tests/derive_ui.rs, tests/examples.rs, tests/ui.rs: test runner stubs (0 test functions)
- clap_lex/tests/testsuite/common.rs, clap_complete/tests/testsuite/common.rs, clap_mangen/tests/testsuite/common.rs: helper files
- examples/, src/_cookbook/, src/_derive/, src/_tutorial.rs: documentation modules
- clap_bench/: benchmarks, not tests

## GT Scope Summary

| Stratum | Files | Description |
|---------|-------|-------------|
| tests/builder/ (excl. utils.rs, main.rs) | 45 | Builder API integration tests |
| tests/derive/ (excl. utils.rs, main.rs) | 32 | Derive macro integration tests |
| top-level tests/ | 3 | examples.rs, macros.rs, ui.rs (runner stubs) |
| clap_lex/tests/testsuite/ | 3 | lexer.rs, parsed.rs, shorts.rs |
| clap_complete/tests/testsuite/ | 7 | bash.rs, elvish.rs, engine.rs, fish.rs, general.rs, powershell.rs, zsh.rs |
| clap_mangen/tests/testsuite/ | 1 | roff.rs |
| **Total** | **91** | |

## Rust-Specific Decisions

- **Crate root barrel re-export**: `use clap::Arg` resolves through `clap/src/lib.rs` -> re-exports -> `clap_builder/src/lib.rs` -> re-exports -> `clap_builder/src/builder/arg.rs`. observe cannot trace this chain; these are FNs.
- **Derive macro tests**: `use clap::Parser` routes through `clap_derive` proc-macro crate. observe cannot cross crate boundaries; these are FNs.
- **automod::dir!()**: clap_lex/tests/testsuite/ uses automod for dynamic module discovery; static AST cannot resolve.
- **Inline #[cfg(test)] modules**: 18 src files contain inline tests. Recorded as inline self-matches (TP by definition).
- **Secondary mappings**: When a test file explicitly imports from multiple production modules (e.g., `use clap::builder::ArgPredicate` AND `use clap::error::ErrorKind`), both are recorded as TP.

## FN Root Cause Analysis

| Root Cause | Count | Example Files |
|-----------|-------|---------------|
| Crate root barrel re-export (`use clap::`) | ~65 | tests/builder/conflicts.rs, groups.rs, help.rs, opts.rs, positionals.rs, etc. |
| Derive macro barrel (`use clap::Parser`) | ~25 | tests/derive/basic.rs, flatten.rs, subcommands.rs, value_enum.rs, etc. |
| automod::dir!() | 3 | clap_lex/tests/testsuite/lexer.rs, parsed.rs, shorts.rs |
| Local module via common.rs | ~8 | clap_complete/tests/testsuite/bash.rs, clap_mangen/tests/testsuite/roff.rs |

## P/R Metrics

Based on GT scope of 91 test files and observe output:

- **Mapped test files (external)**: 13 unique test files
- **Inline self-matches**: 18 files (not counted in external P/R)
- **TP (correctly mapped)**: 13
- **FP**: 0
- **FN**: 78
- **Precision** = 13 / (13 + 0) = **100%** (meets >= 98% ship criterion)
- **Recall** = 13 / (13 + 78) = **14.3%** (does NOT meet >= 90% ship criterion)
- **Conclusion**: clap is NOT a normal-case library for Rust observe. The dominant import pattern (`use clap::Arg` through crate root barrel) is a hard case equivalent to tokio.

## Ground Truth

```json
{
  "metadata": {
    "repository": "clap-rs/clap",
    "commit": "70f3bb3",
    "language": "rust",
    "auditor": "human+ai",
    "audit_coverage": "40-file stratified sample (S1-S5) + full scope classification",
    "date": "2026-03-25"
  },
  "file_mappings": {
    "tests/builder/action.rs": {
      "primary_targets": [
        "clap_builder/src/builder/action.rs"
      ],
      "secondary_targets": [
        "clap_builder/src/builder/arg_predicate.rs",
        "clap_builder/src/error/kind.rs"
      ],
      "evidence": {
        "clap_builder/src/builder/action.rs": [
          "filename_match",
          "direct_import"
        ],
        "clap_builder/src/builder/arg_predicate.rs": [
          "direct_import"
        ],
        "clap_builder/src/error/kind.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/builder/app_settings.rs": {
      "primary_targets": [
        "clap_builder/src/builder/app_settings.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/app_settings.rs": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/builder/command.rs": {
      "primary_targets": [
        "clap_builder/src/builder/command.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/command.rs": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/builder/require.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [
        "clap_builder/src/builder/arg_predicate.rs"
      ],
      "evidence": {
        "clap_builder/src/builder/arg_predicate.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "Primary target arg.rs is FN (crate root barrel `use clap::Arg`). ArgPredicate secondary correctly mapped."
    },
    "tests/builder/default_vals.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [
        "clap_builder/src/builder/arg_predicate.rs",
        "clap_builder/src/error/kind.rs"
      ],
      "evidence": {
        "clap_builder/src/builder/arg_predicate.rs": [
          "direct_import"
        ],
        "clap_builder/src/error/kind.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "Primary target arg.rs is FN (crate root barrel). Secondaries correctly mapped."
    },
    "tests/builder/tests.rs": {
      "primary_targets": [
        "clap_builder/src/builder/tests.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/tests.rs": [
          "filename_match"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/derive/flags.rs": {
      "primary_targets": [
        "clap_builder/src/builder/value_parser.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/value_parser.rs": [
          "direct_import",
          "symbol_assertion"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/derive/utf8.rs": {
      "primary_targets": [
        "clap_builder/src/error/kind.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/error/kind.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/derive/non_literal_attributes.rs": {
      "primary_targets": [
        "clap_builder/src/error/kind.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/error/kind.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/derive/custom_string_parsers.rs": {
      "primary_targets": [
        "clap_builder/src/parser/parser.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/parser/parser.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/derive/doc_comments_help.rs": {
      "primary_targets": [
        "clap_derive/src/utils/doc_comments.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_derive/src/utils/doc_comments.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/derive_ui/value_parser_unsupported.rs": {
      "primary_targets": [
        "clap_builder/src/parser/parser.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/parser/parser.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "trybuild compile-check test; excluded from GT scope count but mapped correctly"
    },
    "clap_complete/tests/testsuite/engine.rs": {
      "primary_targets": [
        "clap_complete/src/engine/candidate.rs",
        "clap_complete/src/engine/custom.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_complete/src/engine/candidate.rs": [
          "direct_import"
        ],
        "clap_complete/src/engine/custom.rs": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/builder/conflicts.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/arg.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/groups.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg_group.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/arg_group.rs": [
          "symbol_assertion",
          "filename_match"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/help.rs": {
      "primary_targets": [
        "clap_builder/src/output/help_template.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/output/help_template.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/subcommands.rs": {
      "primary_targets": [
        "clap_builder/src/builder/command.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/command.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/positionals.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/arg.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/opts.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/arg.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/multiple_values.rs": {
      "primary_targets": [
        "clap_builder/src/builder/arg.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/arg.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/global_args.rs": {
      "primary_targets": [
        "clap_builder/src/builder/command.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/command.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/builder/flag_subcommands.rs": {
      "primary_targets": [
        "clap_builder/src/builder/command.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_builder/src/builder/command.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "crate_root_barrel"
    },
    "tests/derive/basic.rs": {
      "primary_targets": [
        "clap_derive/src/derives/parser.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_derive/src/derives/parser.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "derive_macro_barrel"
    },
    "tests/derive/flatten.rs": {
      "primary_targets": [
        "clap_derive/src/derives/args.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_derive/src/derives/args.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "derive_macro_barrel"
    },
    "tests/derive/subcommands.rs": {
      "primary_targets": [
        "clap_derive/src/derives/subcommand.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_derive/src/derives/subcommand.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "derive_macro_barrel"
    },
    "tests/derive/value_enum.rs": {
      "primary_targets": [
        "clap_derive/src/derives/value_enum.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_derive/src/derives/value_enum.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "derive_macro_barrel"
    },
    "clap_lex/tests/testsuite/lexer.rs": {
      "primary_targets": [
        "clap_lex/src/lib.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_lex/src/lib.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "automod_dir",
      "note": "automod::dir!() prevents static test file discovery"
    },
    "clap_lex/tests/testsuite/shorts.rs": {
      "primary_targets": [
        "clap_lex/src/lib.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_lex/src/lib.rs": [
          "direct_import"
        ]
      },
      "observe_result": "FN",
      "root_cause": "automod_dir",
      "note": "uses `use clap_lex::RawArgs` but not discovered due to automod"
    },
    "clap_lex/tests/testsuite/parsed.rs": {
      "primary_targets": [
        "clap_lex/src/lib.rs"
      ],
      "secondary_targets": [],
      "evidence": {
        "clap_lex/src/lib.rs": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "automod_dir"
    }
  }
}
```
