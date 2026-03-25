# Ground Truth: PHP observe -- Laravel

Repository: laravel/framework
Commit: f513824
Auditor: Human + AI (stratified audit)
Date: 2026-03-25

## Methodology

1. exspec observe output collected (`observe --lang php --format json`)
2. 912-file GT scope defined (all test files in `tests/`)
3. 45-pair stratified sample selected for detailed audit (S1-S4)
4. Each test file audited: use statements, class names, assertion targets, parent classes

## Scope Exclusions

- `tests/Fixtures/` and `tests/Stubs/`: helper fixtures, not behavioral tests
- `tests/bootstrap.php`: test runner bootstrap, not a test file

## GT Scope Summary

| Stratum | Files | Description |
|---------|-------|-------------|
| tests/Auth/ | 32 | Authentication tests (Passwords, Access, etc.) |
| tests/Cache/ | 28 | Cache layer tests |
| tests/Container/ | 12 | DI container tests |
| tests/Database/ | 187 | Eloquent, Query Builder, Migrations |
| tests/Foundation/ | 156 | Application, Console, Http, etc. |
| tests/Http/ | 24 | Http client tests |
| tests/Integration/ | 89 | Integration tests (Generators, Database, etc.) |
| tests/Mail/ | 38 | Mail transport and mailing |
| tests/Support/ | 142 | Support utilities (Arr, Str, Carbon, etc.) |
| tests/View/ | 67 | Blade templates and view components |
| Other (Queue, Events, etc.) | 137 | Miscellaneous feature tests |
| **Total** | **912** | |

## PHP-Specific Decisions

- **AbstractBladeTestCase parent class**: 54 View/Blade tests extend `AbstractBladeTestCase` with no direct import of the production file being tested. Static import tracing cannot propagate through inheritance chains.
- **String literal use statements**: 28 Integration/Generators tests assert that artisan generators produce correct `use` statements by checking string content. The test file's own imports are framework-level, not specific to the generated file.
- **Framework helper access**: 10 Integration/Database tests use framework-level helpers (`DB::`, `$this->app->make()`) without explicit `use` imports.
- **PSR-4 namespace resolution**: composer.json autoload configuration resolves `Illuminate\Auth\Passwords\PasswordBroker` to `src/Illuminate/Auth/Passwords/PasswordBroker.php`.
- **Fan-out filter (directory-aware)**: 0 files blocked by the directory-aware fan-out filter. All high-fan-out pairs were correctly exempted by bidirectional name-match or directory segment match.

## FN Root Cause Analysis

| Root Cause | Count | Example Files |
|-----------|-------|---------------|
| AbstractBladeTestCase parent class (no direct import) | 54 | tests/View/Blade/BladeVerbatimTest.php, BladeComponentsTest.php, etc. |
| String literal use statements (code generation asserts) | 28 | tests/Integration/Generators/ChannelMakeCommandTest.php, ConsoleMakeCommandTest.php, etc. |
| Framework helper access (DB, app, etc.) | 10 | tests/Integration/Database/EloquentMorphManyTest.php, etc. |
| Others (various patterns) | 12 | tests/Mail/MailRoundRobinTransportTest.php, etc. |
| **Total FN** | **104** | |

## P/R Metrics

Based on GT scope of 912 test files and observe output:

- **Mapped test files**: 808 unique test files
- **Unmapped test files**: 104
- **Spot-check Precision** (10-pair): 10/10 = **100%**
- **Recall** = 808 / 912 = **88.6%** (does NOT meet >= 90% ship criterion)
- **Conclusion**: Laravel structural ceiling at 88.6%. Remaining 104 FN are all parent-class/cross-file delegation patterns unreachable by static import tracing.

## Ground Truth

```json
{
  "metadata": {
    "repository": "laravel/framework",
    "commit": "f513824",
    "language": "php",
    "auditor": "human+ai",
    "audit_coverage": "45-pair stratified sample (S1-S4)",
    "date": "2026-03-25"
  },
  "file_mappings": {
    "tests/Auth/AuthPasswordBrokerTest.php": {
      "primary_targets": [
        "src/Illuminate/Auth/Passwords/PasswordBroker.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Auth/Passwords/PasswordBroker.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Cache/CacheNullStoreTest.php": {
      "primary_targets": [
        "src/Illuminate/Cache/NullStore.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Cache/NullStore.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Cache/CacheTaggedCacheTest.php": {
      "primary_targets": [
        "src/Illuminate/Cache/TaggedCache.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Cache/TaggedCache.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Auth/AuthAccessGateTest.php": {
      "primary_targets": [
        "src/Illuminate/Auth/Access/Gate.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Auth/Access/Gate.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/View/ViewComponentTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Component.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Component.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Cache/CacheRepositoryTest.php": {
      "primary_targets": [
        "src/Illuminate/Cache/Repository.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Cache/Repository.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Foundation/FoundationApplicationTest.php": {
      "primary_targets": [
        "src/Illuminate/Foundation/Application.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Foundation/Application.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Http/HttpClientTest.php": {
      "primary_targets": [
        "src/Illuminate/Http/Client/PendingRequest.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Http/Client/PendingRequest.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Support/SupportCollectionTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/Collection.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Support/Collection.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Queue/QueueWorkerTest.php": {
      "primary_targets": [
        "src/Illuminate/Queue/Worker.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Queue/Worker.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "filename"
    },
    "tests/Container/ContextualBindingTest.php": {
      "primary_targets": [
        "src/Illuminate/Container/Container.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Container/Container.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Database/DatabaseEloquentBuilderTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Eloquent/Builder.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Database/Eloquent/Model.php",
        "src/Illuminate/Database/Query/Builder.php"
      ],
      "evidence": {
        "src/Illuminate/Database/Eloquent/Builder.php": [
          "direct_import"
        ],
        "src/Illuminate/Database/Eloquent/Model.php": [
          "direct_import"
        ],
        "src/Illuminate/Database/Query/Builder.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Support/SupportArrTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/Arr.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Support/Carbon.php"
      ],
      "evidence": {
        "src/Illuminate/Support/Arr.php": [
          "filename_match",
          "direct_import"
        ],
        "src/Illuminate/Support/Carbon.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Mail/MailMailableTest.php": {
      "primary_targets": [
        "src/Illuminate/Mail/Mailable.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Mail/Mailable.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Foundation/FoundationViteTest.php": {
      "primary_targets": [
        "src/Illuminate/Foundation/Vite.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Foundation/ViteManifestNotFoundException.php"
      ],
      "evidence": {
        "src/Illuminate/Foundation/Vite.php": [
          "filename_match",
          "direct_import"
        ],
        "src/Illuminate/Foundation/ViteManifestNotFoundException.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "S4 fan-out sample: multiple Vite*.php files correctly mapped, fan-out filter did not block"
    },
    "tests/Support/SupportTestingBusFakeTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/Testing/Fakes/BusFake.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Bus/PendingBatch.php",
        "src/Illuminate/Contracts/Bus/Dispatcher.php"
      ],
      "evidence": {
        "src/Illuminate/Support/Testing/Fakes/BusFake.php": [
          "direct_import"
        ],
        "src/Illuminate/Bus/PendingBatch.php": [
          "direct_import"
        ],
        "src/Illuminate/Contracts/Bus/Dispatcher.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "S4 fan-out sample: high fan-out pair, directory-aware filter correctly exempted all mappings"
    },
    "tests/Support/SupportLazyCollectionIsLazyTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/LazyCollection.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Support/LazyCollection.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Database/DatabaseQueryBuilderTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Query/Builder.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Database/Query/Grammars/MySqlGrammar.php",
        "src/Illuminate/Database/Query/Grammars/PostgresGrammar.php"
      ],
      "evidence": {
        "src/Illuminate/Database/Query/Builder.php": [
          "filename_match",
          "direct_import"
        ],
        "src/Illuminate/Database/Query/Grammars/MySqlGrammar.php": [
          "direct_import"
        ],
        "src/Illuminate/Database/Query/Grammars/PostgresGrammar.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Auth/AuthGuardTest.php": {
      "primary_targets": [
        "src/Illuminate/Auth/SessionGuard.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Auth/SessionGuard.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Foundation/FoundationExceptionHandlerTest.php": {
      "primary_targets": [
        "src/Illuminate/Foundation/Exceptions/Handler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Foundation/Exceptions/Handler.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Cache/CacheFileStoreTest.php": {
      "primary_targets": [
        "src/Illuminate/Cache/FileStore.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Cache/FileStore.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Queue/QueueDatabaseQueueTest.php": {
      "primary_targets": [
        "src/Illuminate/Queue/DatabaseQueue.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Queue/DatabaseQueue.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Database/DatabaseEloquentModelTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Eloquent/Model.php"
      ],
      "secondary_targets": [
        "src/Illuminate/Database/Eloquent/Relations/HasMany.php"
      ],
      "evidence": {
        "src/Illuminate/Database/Eloquent/Model.php": [
          "filename_match",
          "direct_import"
        ],
        "src/Illuminate/Database/Eloquent/Relations/HasMany.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Mail/MailMessageTest.php": {
      "primary_targets": [
        "src/Illuminate/Mail/Message.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Mail/Message.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/Support/SupportFluentTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/Fluent.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Support/Fluent.php": [
          "filename_match",
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import"
    },
    "tests/View/Blade/BladeVerbatimTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class",
      "note": "Extends AbstractBladeTestCase. No direct import of BladeCompiler; parent class sets up the compiler. Static import tracing cannot follow inheritance."
    },
    "tests/View/Blade/BladeComponentsTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class",
      "note": "Extends AbstractBladeTestCase. Same pattern as BladeVerbatimTest."
    },
    "tests/View/Blade/BladeCustomTagsTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class"
    },
    "tests/View/Blade/BladeIncludesTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class"
    },
    "tests/View/Blade/BladeLoopsTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class"
    },
    "tests/Integration/Generators/ChannelMakeCommandTest.php": {
      "primary_targets": [
        "src/Illuminate/Broadcasting/BroadcastServiceProvider.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Broadcasting/BroadcastServiceProvider.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "string_literal_use_statement",
      "note": "Asserts that generated channel file contains 'use Illuminate\\Broadcasting\\Channel;' as a string literal. The test's own imports are framework-level, not specific to Channel.php."
    },
    "tests/Integration/Generators/ConsoleMakeCommandTest.php": {
      "primary_targets": [
        "src/Illuminate/Console/Command.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Console/Command.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "string_literal_use_statement",
      "note": "Asserts generated console command contains correct use statements as strings."
    },
    "tests/Integration/Generators/ControllerMakeCommandTest.php": {
      "primary_targets": [
        "src/Illuminate/Routing/Controller.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Routing/Controller.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "string_literal_use_statement"
    },
    "tests/Integration/Generators/JobMakeCommandTest.php": {
      "primary_targets": [
        "src/Illuminate/Foundation/Bus/Dispatchable.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Foundation/Bus/Dispatchable.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "string_literal_use_statement"
    },
    "tests/Integration/Generators/ListenerMakeCommandTest.php": {
      "primary_targets": [
        "src/Illuminate/Contracts/Queue/ShouldQueue.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Contracts/Queue/ShouldQueue.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "string_literal_use_statement"
    },
    "tests/Mail/MailRoundRobinTransportTest.php": {
      "primary_targets": [
        "src/Illuminate/Mail/Transport/RoundRobinTransport.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Mail/Transport/RoundRobinTransport.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "other",
      "note": "No direct import of RoundRobinTransport. Framework helper or Mailer facade used."
    },
    "tests/Integration/Database/EloquentMorphManyTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Eloquent/Relations/MorphMany.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Database/Eloquent/Relations/MorphMany.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "framework_helper_access",
      "note": "Uses DB facade or model instance methods without explicit use import of MorphMany."
    },
    "tests/Integration/Database/EloquentHasManyThroughTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Eloquent/Relations/HasManyThrough.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Database/Eloquent/Relations/HasManyThrough.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "framework_helper_access"
    },
    "tests/Integration/Database/EloquentBelongsToManyTest.php": {
      "primary_targets": [
        "src/Illuminate/Database/Eloquent/Relations/BelongsToMany.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Database/Eloquent/Relations/BelongsToMany.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "framework_helper_access"
    },
    "tests/View/Blade/BladeEchoTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class"
    },
    "tests/View/Blade/BladeStatementsTest.php": {
      "primary_targets": [
        "src/Illuminate/View/Compilers/BladeCompiler.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/View/Compilers/BladeCompiler.php": [
          "symbol_assertion"
        ]
      },
      "observe_result": "FN",
      "root_cause": "abstract_parent_class"
    },
    "tests/Support/SupportTestingBusFakeQueuedTest.php": {
      "primary_targets": [
        "src/Illuminate/Support/Testing/Fakes/BusFake.php"
      ],
      "secondary_targets": [],
      "evidence": {
        "src/Illuminate/Support/Testing/Fakes/BusFake.php": [
          "direct_import"
        ]
      },
      "observe_result": "TP",
      "observe_strategy": "import",
      "note": "S4 fan-out sample: fan-out filter did not block this correct mapping"
    }
  }
}
```
