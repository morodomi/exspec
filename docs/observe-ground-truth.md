# Ground Truth: nestjs/nest test-to-code mapping

## Metadata

| Key | Value |
|-----|-------|
| Repository | nestjs/nest |
| Commit | `4593f5889c482fc0e29060222757c0fececa94fa` |
| Date | 2026-03-16 |
| Scope | `packages/common`, `packages/core` |
| Production files (total) | 364 |
| Test files (total) | 130 |
| Human-audited test files | 77/130 (59%) |
| Primary mappings | 166 |
| Unmapped production files | 206 |

## Methodology

### Definition

Ground truth = "the production files that each test is primarily testing" (SUT),
not merely "what the test imports".

### Process

1. **ts-morph symbol-level resolution**: TypeScript Compiler API resolves all imports
   including barrel re-exports (`index.ts`) to concrete definition files
2. **Name-based candidate generation**: spec filename -> production filename matching
3. **Automated classification**: Evidence scoring (symbol_assertion, filename_match,
   test_name_match, call_usage, constructor_usage, provider_registration)
4. **Stratified human audit** (77/130 = 59%):
   - All decorator/interface primary targets (25 files)
   - Random non-decorator sample (20 files)
   - Initial calibration set (21 files)
   - Re-audit after rule changes (11 files)
5. **Automated rules** (conservative):
   - Setup dependency demotion (constructor-only, no name evidence)
   - Filename match requires import corroboration
   - Stricter confidence levels (2+ independent strong evidence for high)

### Annotation Guideline

See `docs/observe-gt-guideline.md` for full classification rules.

### Bias Mitigation

- ts-morph (TypeScript Compiler API) used instead of tree-sitter to avoid
  tautological evaluation (exspec uses tree-sitter)
- Symbol-level resolution prevents barrel expansion noise
- Human audit of 59% of entries across all risk strata
- Decorator-as-fixture pattern explicitly handled

## Confidence Distribution

| Level | Count | Meaning |
|-------|-------|---------|
| high | 126 | 2+ independent strong evidence types |
| medium | 3 | 1 strong evidence type |
| uncertain | 1 | Integration test or no clear SUT |

## Ground Truth Table

| Test File | Primary Target(s) | Evidence | Confidence |
|-----------|-------------------|----------|------------|
| packages/common/test/decorators/apply-decorators.spec.ts | packages/common/decorators/core/apply-decorators.ts | barrel_import, call_usage, filename_match, test_name_match | high |
| packages/common/test/decorators/bind.decorator.spec.ts | packages/common/decorators/core/bind.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/catch.decorator.spec.ts | packages/common/decorators/core/catch.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/controller.decorator.spec.ts | packages/common/decorators/core/controller.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/create-param-decorator.spec.ts | packages/common/decorators/http/create-route-param-metadata.decorator.ts | call_usage, direct_import, test_name_match | medium |
| packages/common/test/decorators/dependencies.decorator.spec.ts | packages/common/decorators/core/dependencies.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/exception-filters.decorator.spec.ts | packages/common/decorators/core/exception-filters.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/global.decorator.spec.ts | packages/common/decorators/modules/global.decorator.ts | barrel_import, call_usage, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/decorators/header.decorator.spec.ts | packages/common/decorators/http/header.decorator.ts | barrel_import, call_usage, filename_match, test_name_match | high |
| packages/common/test/decorators/http-code.decorator.spec.ts | packages/common/decorators/http/http-code.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/inject.decorator.spec.ts | packages/common/decorators/core/inject.decorator.ts | barrel_import, call_usage, filename_match, test_name_match | high |
| packages/common/test/decorators/injectable.decorator.spec.ts | packages/common/decorators/core/injectable.decorator.ts | barrel_import, call_usage, filename_match, test_name_match | high |
| packages/common/test/decorators/module.decorator.spec.ts | packages/common/decorators/modules/module.decorator.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/decorators/redirect.decorator.spec.ts | packages/common/decorators/http/redirect.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/render.decorator.spec.ts | packages/common/decorators/http/render.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/request-mapping.decorator.spec.ts | packages/common/decorators/http/request-mapping.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/route-params.decorator.spec.ts | packages/common/decorators/http/route-params.decorator.ts | barrel_import, call_usage, filename_match, symbol_assertion | high |
| packages/common/test/decorators/set-metadata.decorator.spec.ts | packages/common/decorators/core/set-metadata.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/sse.decorator.spec.ts | packages/common/decorators/http/sse.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/use-guards.decorator.spec.ts | packages/common/decorators/core/use-guards.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/use-interceptors.decorator.spec.ts | packages/common/decorators/core/use-interceptors.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/use-pipes.decorator.spec.ts | packages/common/decorators/core/use-pipes.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/decorators/version.decorator.spec.ts | packages/common/decorators/core/version.decorator.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/exceptions/http.exception.spec.ts | packages/common/exceptions/http.exception.ts | barrel_import, constructor_usage, filename_match, symbol_assertion, test_name_match | high |
|  | packages/common/exceptions/bad-request.exception.ts | barrel_import, constructor_usage, symbol_assertion |  |
| packages/common/test/file-stream/streamable-file.spec.ts | packages/common/file-stream/streamable-file.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/common/test/module-utils/configurable-module.builder.spec.ts | packages/common/module-utils/configurable-module.builder.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/common/test/module-utils/utils/get-injection-providers.util.spec.ts | packages/common/module-utils/utils/get-injection-providers.util.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/default-value.pipe.spec.ts | packages/common/pipes/default-value.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/file/file-type.validator.spec.ts | packages/common/pipes/file/file-type.validator.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/common/test/pipes/file/max-file-size.validator.spec.ts | packages/common/pipes/file/max-file-size.validator.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/common/test/pipes/file/parse-file-pipe.builder.spec.ts | packages/common/pipes/file/file-type.validator.ts | barrel_import, call_usage, constructor_usage, symbol_assertion, test_name_match | high |
|  | packages/common/pipes/file/parse-file-pipe.builder.ts | barrel_import, constructor_usage, filename_match, test_name_match |  |
| packages/common/test/pipes/file/parse-file.pipe.spec.ts | packages/common/pipes/file/parse-file.pipe.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-array.pipe.spec.ts | packages/common/pipes/parse-array.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-bool.pipe.spec.ts | packages/common/pipes/parse-bool.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-date.pipe.spec.ts | packages/common/pipes/parse-date.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-enum.pipe.spec.ts | packages/common/pipes/parse-enum.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-float.pipe.spec.ts | packages/common/pipes/parse-float.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-int.pipe.spec.ts | packages/common/pipes/parse-int.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/parse-uuid.pipe.spec.ts | packages/common/pipes/parse-uuid.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/pipes/validation.pipe.spec.ts | packages/common/pipes/validation.pipe.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/serializer/class-serializer.interceptor.spec.ts | packages/common/serializer/class-serializer.interceptor.ts | constructor_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/services/logger.service.spec.ts | packages/common/services/logger.service.ts | barrel_import, call_usage, constructor_usage, filename_match, symbol_assertion, test_name_match | high |
|  | packages/common/services/console-logger.service.ts | barrel_import, call_usage, constructor_usage, test_name_match |  |
| packages/common/test/services/utils/filter-log-levels.util.spec.ts | packages/common/services/utils/filter-log-levels.util.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/services/utils/is-log-level-enabled.util.spec.ts | packages/common/services/utils/is-log-level-enabled.util.ts | barrel_import, call_usage, filename_match, symbol_assertion, test_name_match | high |
|  | packages/common/services/logger.service.ts | direct_import, symbol_assertion, test_name_match |  |
| packages/common/test/utils/forward-ref.util.spec.ts | packages/common/utils/forward-ref.util.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/utils/load-package.util.spec.ts | packages/common/utils/load-package.util.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/utils/merge-with-values.util.spec.ts | packages/common/utils/merge-with-values.util.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/common/test/utils/random-string-generator.util.spec.ts | packages/common/utils/random-string-generator.util.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/utils/select-exception-filter-metadata.util.spec.ts | packages/common/utils/select-exception-filter-metadata.util.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/utils/shared.utils.spec.ts | packages/common/utils/shared.utils.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/common/test/utils/validate-each.util.spec.ts | packages/common/utils/validate-each.util.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/core/test/application-config.spec.ts | packages/core/application-config.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/discovery/discoverable-meta-host-collection.spec.ts | packages/core/discovery/discoverable-meta-host-collection.ts | direct_import, filename_match, symbol_assertion, test_name_match | high |
|  | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, test_name_match |  |
| packages/core/test/discovery/discovery-service.spec.ts | packages/core/discovery/discovery-service.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | constructor_usage, direct_import, test_name_match |  |
| packages/core/test/errors/test/exception-handler.spec.ts | packages/core/errors/exception-handler.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/errors/test/exceptions-zone.spec.ts | packages/core/errors/exceptions-zone.ts | direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/core/test/errors/test/messages.spec.ts | packages/core/errors/messages.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/exceptions/base-exception-filter.spec.ts | packages/core/exceptions/base-exception-filter-context.ts | constructor_usage, direct_import, test_name_match | medium |
| packages/core/test/exceptions/exceptions-handler.spec.ts | packages/core/exceptions/exceptions-handler.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/exceptions/external-exception-filter-context.spec.ts | packages/core/exceptions/external-exception-filter-context.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/exceptions/external-exceptions-handler.spec.ts | packages/core/exceptions/external-exceptions-handler.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/guards/guards-consumer.spec.ts | packages/core/guards/guards-consumer.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/guards/guards-context-creator.spec.ts | packages/core/guards/guards-context-creator.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/helpers/application-ref-host.spec.ts | packages/core/helpers/http-adapter-host.ts | constructor_usage, direct_import, test_name_match | medium |
| packages/core/test/helpers/barrier.spec.ts | packages/core/helpers/barrier.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/helpers/context-id-factory.spec.ts | packages/core/helpers/context-id-factory.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/core/test/helpers/context-utils.spec.ts | packages/core/helpers/context-utils.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/helpers/execution-context-host.spec.ts | packages/core/helpers/execution-context-host.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/helpers/external-context-creator.spec.ts | packages/core/helpers/external-context-creator.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | constructor_usage, direct_import, symbol_assertion, test_name_match |  |
| packages/core/test/helpers/external-proxy.spec.ts | packages/core/helpers/external-proxy.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/helpers/router-method-factory.spec.ts | packages/core/helpers/router-method-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/hooks/before-app-shutdown.hook.spec.ts | packages/core/hooks/before-app-shutdown.hook.ts | call_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import |  |
| packages/core/test/hooks/on-app-bootstrap.hook.spec.ts | packages/core/hooks/on-app-bootstrap.hook.ts | call_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, test_name_match |  |
| packages/core/test/hooks/on-app-shutdown.hook.spec.ts | packages/core/hooks/on-app-shutdown.hook.ts | call_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import |  |
| packages/core/test/hooks/on-module-destroy.hook.spec.ts | packages/core/hooks/on-module-destroy.hook.ts | call_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, test_name_match |  |
| packages/core/test/hooks/on-module-init.hook.spec.ts | packages/core/hooks/on-module-init.hook.ts | call_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, test_name_match |  |
| packages/core/test/injector/compiler.spec.ts | packages/core/injector/compiler.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/injector/container.spec.ts | packages/core/injector/container.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/injector/helpers/provider-classifier.spec.ts | packages/core/injector/helpers/provider-classifier.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
|  | packages/common/interfaces/modules/provider.interface.ts | barrel_import, call_usage, symbol_assertion, test_name_match |  |
| packages/core/test/injector/helpers/silent-logger.spec.ts | packages/core/injector/helpers/silent-logger.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/common/services/logger.service.ts | barrel_import, call_usage, symbol_assertion, test_name_match |  |
| packages/core/test/injector/injector.spec.ts | packages/core/injector/injector.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/common/decorators/core/inject.decorator.ts | call_usage, direct_import, test_name_match |  |
|  | packages/common/decorators/core/injectable.decorator.ts | call_usage, direct_import, test_name_match |  |
|  | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, test_name_match |  |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import |  |
| packages/core/test/injector/instance-loader.spec.ts | packages/common/decorators/core/controller.decorator.ts | call_usage, direct_import, symbol_assertion, test_name_match | high |
|  | packages/core/injector/instance-loader.ts | constructor_usage, direct_import, filename_match, test_name_match |  |
|  | packages/common/decorators/core/injectable.decorator.ts | barrel_import, call_usage, symbol_assertion, test_name_match |  |
|  | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, symbol_assertion |  |
| packages/core/test/injector/instance-wrapper.spec.ts | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
|  | packages/common/interfaces/scope-options.interface.ts | barrel_import, call_usage, symbol_assertion, test_name_match |  |
| packages/core/test/injector/internal-core-module/internal-core-module-factory.spec.ts | packages/core/injector/internal-core-module/internal-core-module-factory.ts | direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/core/test/injector/lazy-module-loader/lazy-module-loader.spec.ts | packages/core/injector/lazy-module-loader/lazy-module-loader.ts | barrel_import, constructor_usage, filename_match, test_name_match | high |
| packages/core/test/injector/module.spec.ts | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
|  | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, symbol_assertion |  |
| packages/core/test/injector/nested-transient-isolation.spec.ts | *(none)* |  | uncertain |
| packages/core/test/injector/opaque-key-factory/by-reference-module-opaque-key-factory.spec.ts | packages/core/injector/opaque-key-factory/by-reference-module-opaque-key-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/injector/opaque-key-factory/deep-hashed-module-opaque-key-factory.spec.ts | packages/core/injector/opaque-key-factory/deep-hashed-module-opaque-key-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/injector/topology-tree/tree-node.spec.ts | packages/core/injector/topology-tree/tree-node.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/inspector/graph-inspector.spec.ts | packages/core/inspector/graph-inspector.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/injector/instance-wrapper.ts | call_usage, constructor_usage, direct_import, test_name_match |  |
|  | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, test_name_match |  |
| packages/core/test/inspector/serialized-graph.spec.ts | packages/core/inspector/interfaces/node.interface.ts | call_usage, direct_import, symbol_assertion, test_name_match | high |
|  | packages/core/inspector/serialized-graph.ts | constructor_usage, direct_import, filename_match, test_name_match |  |
|  | packages/core/inspector/interfaces/edge.interface.ts | call_usage, direct_import, test_name_match |  |
| packages/core/test/interceptors/interceptors-consumer.spec.ts | packages/core/interceptors/interceptors-consumer.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/interceptors/interceptors-context-creator.spec.ts | packages/core/interceptors/interceptors-context-creator.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/metadata-scanner.spec.ts | packages/core/metadata-scanner.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/builder.spec.ts | packages/core/middleware/builder.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/container.spec.ts | packages/core/middleware/container.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/middleware-module.spec.ts | packages/core/injector/module.ts | call_usage, constructor_usage, direct_import, symbol_assertion, test_name_match | high |
|  | packages/core/middleware/middleware-module.ts | constructor_usage, direct_import, filename_match, test_name_match |  |
|  | packages/core/middleware/builder.ts | constructor_usage, direct_import, symbol_assertion |  |
|  | packages/core/middleware/container.ts | constructor_usage, direct_import, symbol_assertion |  |
| packages/core/test/middleware/resolver.spec.ts | packages/core/middleware/resolver.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/route-info-path-extractor.spec.ts | packages/core/middleware/route-info-path-extractor.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/routes-mapper.spec.ts | packages/core/middleware/routes-mapper.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/middleware/utils.spec.ts | packages/core/middleware/utils.ts | call_usage, direct_import, filename_match, symbol_assertion, test_name_match | high |
| packages/core/test/nest-application-context.spec.ts | packages/core/nest-application-context.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/nest-application.spec.ts | packages/core/nest-application.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/pipes/params-token-factory.spec.ts | packages/core/pipes/params-token-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/common/enums/route-paramtypes.enum.ts | direct_import, symbol_assertion, test_name_match |  |
| packages/core/test/pipes/pipes-consumer.spec.ts | packages/core/pipes/pipes-consumer.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/pipes/pipes-context-creator.spec.ts | packages/core/pipes/pipes-context-creator.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/repl/assign-to-object.util.spec.ts | packages/core/repl/assign-to-object.util.ts | call_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/debug-repl-fn.spec.ts | packages/core/repl/native-functions/debug-repl-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/get-repl-fn.spec.ts | packages/core/repl/native-functions/get-repl-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/help-repl-fn.spec.ts | packages/core/repl/native-functions/help-repl-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/methods-repl-fn.spec.ts | packages/core/repl/native-functions/methods-repl-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/resolve-repl-fn.spec.ts | packages/core/repl/native-functions/resolve-repl-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/native-functions/select-repl-fn.spec.ts | packages/core/repl/native-functions/select-relp-fn.ts | barrel_import, filename_match, test_name_match | high |
| packages/core/test/repl/repl-context.spec.ts | packages/core/repl/repl-context.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/paths-explorer.spec.ts | packages/core/router/paths-explorer.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/route-params-factory.spec.ts | packages/core/router/route-params-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/common/enums/route-paramtypes.enum.ts | direct_import, symbol_assertion, test_name_match |  |
| packages/core/test/router/route-path-factory.spec.ts | packages/core/router/route-path-factory.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/router-exception-filters.spec.ts | packages/core/router/router-exception-filters.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/router-execution-context.spec.ts | packages/core/router/router-execution-context.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/router-explorer.spec.ts | packages/core/router/router-explorer.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/router-module.spec.ts | packages/core/router/router-module.ts | constructor_usage, direct_import, filename_match, provider_registration, symbol_assertion, test_name_match | high |
| packages/core/test/router/router-proxy.spec.ts | packages/core/router/router-proxy.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/core/helpers/execution-context-host.ts | constructor_usage, direct_import, symbol_assertion |  |
| packages/core/test/router/router-response-controller.spec.ts | packages/core/router/router-response-controller.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
|  | packages/common/enums/request-method.enum.ts | barrel_import, symbol_assertion, test_name_match |  |
| packages/core/test/router/routes-resolver.spec.ts | packages/core/router/routes-resolver.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/sse-stream.spec.ts | packages/core/router/sse-stream.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/router/utils/flat-routes.spec.ts | packages/core/router/utils/flatten-route-paths.util.ts | barrel_import, call_usage, symbol_assertion, test_name_match | high |
| packages/core/test/scanner.spec.ts | packages/common/decorators/modules/module.decorator.ts | call_usage, direct_import, provider_registration, symbol_assertion, test_name_match | high |
|  | packages/common/decorators/core/controller.decorator.ts | call_usage, direct_import, provider_registration, symbol_assertion |  |
|  | packages/core/scanner.ts | constructor_usage, direct_import, filename_match, test_name_match |  |
|  | packages/common/decorators/core/injectable.decorator.ts | barrel_import, call_usage, symbol_assertion, test_name_match |  |
|  | packages/core/injector/instance-wrapper.ts | constructor_usage, direct_import, symbol_assertion |  |
| packages/core/test/services/reflector.service.spec.ts | packages/core/services/reflector.service.ts | constructor_usage, direct_import, filename_match, test_name_match | high |
| packages/core/test/utils/noop-adapter.spec.ts | *(none)* | Audit: NoopHttpAdapter defined inline in spec. No production file. | high |

## Audit Trail

### Corrections Applied

| Test File | Change | Reason |
|-----------|--------|--------|
| packages/core/test/application-config.spec.ts | Audit: GlobalPrefixOptions is type annotation only | Human audit |
| packages/core/test/errors/test/messages.spec.ts | Audit: Module is test fixture | Human audit |
| packages/core/test/exceptions/external-exception-filter-context.spec.ts | Audit: Catch is fixture for exception filter test | Human audit |
| packages/core/test/injector/container.spec.ts | Audit: Module is fixture; Audit: Global is fixture | Human audit |
| packages/core/test/injector/internal-core-module/internal-core-module-factory.spec.ts | Audit: Primary changed to factory file | Human audit |
| packages/core/test/injector/lazy-module-loader/lazy-module-loader.spec.ts | Audit: Module is fixture | Human audit |
| packages/core/test/injector/module.spec.ts | Audit: Injectable is fixture | Human audit |
| packages/core/test/middleware/container.spec.ts | Audit: NestContainer is setup dependency | Human audit |
| packages/core/test/middleware/routes-mapper.spec.ts | Audit: Controller is test fixture; Audit: Version is test fixture | Human audit |
| packages/core/test/repl/native-functions/select-repl-fn.spec.ts | Audit: select-relp-fn.ts (typo in filename) promoted to primary | Human audit |
| packages/core/test/router/router-exception-filters.spec.ts | Audit: Catch is fixture | Human audit |
| packages/core/test/router/router-explorer.spec.ts | Audit: All/request-mapping is fixture | Human audit |
| packages/core/test/router/router-module.spec.ts | Audit: Module is fixture | Human audit |
| packages/core/test/utils/noop-adapter.spec.ts | Audit: NoopHttpAdapter defined inline in spec. No production file. | Human audit |
| packages/common/test/utils/validate-each.util.spec.ts | Added shared.utils.ts to secondary (utility function for test setup) | FP audit (14 pairs) |
| packages/core/test/discovery/discovery-service.spec.ts | Added discoverable-meta-host-collection.ts to secondary (spy/stub target) | FP audit (14 pairs) |
| packages/core/test/exceptions/external-exceptions-handler.spec.ts | Added external-exception-filter.ts to secondary (log suppression setup) | FP audit (14 pairs) |
| packages/core/test/injector/internal-core-module/internal-core-module-factory.spec.ts | Added 4 files to secondary (expected-value tokens in provider array) | FP audit (14 pairs) |
| packages/core/test/inspector/graph-inspector.spec.ts | Added serialized-graph.ts to secondary (internal field access) | FP audit (14 pairs) |
| packages/core/test/inspector/serialized-graph.spec.ts | Added application-config.ts to secondary (test data token) | FP audit (14 pairs) |
| packages/core/test/nest-application-context.spec.ts | Added context-id-factory.ts to secondary (input value generation) | FP audit (14 pairs) |
| packages/core/test/router/router-execution-context.spec.ts | Added handler-metadata-storage.ts, sse-stream.ts to secondary (type casts only) | FP audit (14 pairs) |
| packages/core/test/router/router-explorer.spec.ts | Added execution-context-host.ts to secondary (expected-value token) | FP audit (14 pairs) |
| packages/core/test/router/router-response-controller.spec.ts | Added sse-stream.ts to secondary (stub target class) | FP audit (14 pairs) |
| packages/common/test/decorators/route-params.decorator.spec.ts | Added request-method.enum.ts, parse-int.pipe.ts to secondary (value comparison, decorator arg) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/application-config.spec.ts | Added exclude-route-metadata.interface.ts to secondary (type parameter) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/exceptions/exceptions-handler.spec.ts | Added invalid-exception-filter.exception.ts to secondary (throw assertion target) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/injector/container.spec.ts | Added circular-dependency.exception.ts, unknown-module.exception.ts to secondary (throw assertion targets) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/injector/module.spec.ts | Added unknown-element.exception.ts, unknown-export.exception.ts to secondary (throw assertion targets) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/inspector/graph-inspector.spec.ts | Added enhancer-metadata-cache-entry.interface.ts to secondary (type annotation + object construction) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/router/router-explorer.spec.ts | Added unknown-request-mapping.exception.ts, route-path-metadata.interface.ts to secondary (throw assertion, type annotation) | Phase 11 re-dogfood (12 pairs) |
| packages/core/test/scanner.spec.ts | Added module-override.interface.ts to secondary (type annotation + value construction) | Phase 11 re-dogfood (12 pairs) |

### Audit Coverage by Stratum

| Stratum | Audited | Total | Coverage | Correction Rate |
|---------|---------|-------|----------|-----------------|
| Decorator primary (common) | 16 | 16 | 100% | 0% |
| Decorator primary (core) | 9 | 9 | 100% | 67% |
| Non-decorator (random) | 20 | 73 | 27% | 0% |
| Multi-primary | 5 | 10 | 50% | 20% |
| Barrel import | 5 | 28 | 18% | 20% |
| Uncertain | 3 | 3 | 100% | 33% (reclassified) |
| Calibration + re-audit | 19 | - | - | - |

## Expected Evaluation Notes

- **Layer 1 (directory matching)**: Expected 0 matches.
  All tests are in separate `test/` directories.
- **Layer 2 (import tracing)**: Primary evaluation target.
  Direct imports should resolve; barrel imports partially supported (Phase 8b+).
- **Barrel imports**: 197 symbols resolved through barrels via ts-morph.
  exspec supports barrel resolution (Phase 8b), including `export *` and named re-exports.
  Remaining barrel FN: `http.exception.spec.ts` imports through `../../exceptions` barrel
  where all re-exported files are `.exception.ts` (filtered by `is_non_sut_helper`).
- **Cross-package imports (B2)**: Tests in `packages/core` importing from `packages/common`
  are only resolved when observe runs on the project root (not separate packages).
  Single-package execution misses 8-13 cross-package primary targets.
- **@nestjs/* imports**: Partially resolved via tsconfig path alias support (Phase 8c).
  Cross-package aliases resolve when observe runs on root.

## Machine-Readable Data

```json
{
  "metadata": {
    "repository": "nestjs/nest",
    "commit": "4593f5889c482fc0e29060222757c0fececa94fa",
    "date": "2026-03-16",
    "scope": [
      "packages/common",
      "packages/core"
    ],
    "methodology": "ts-morph symbol resolution + stratified human audit (59%)",
    "guideline": "docs/observe-gt-guideline.md"
  },
  "file_mappings": {
    "packages/common/test/decorators/apply-decorators.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/apply-decorators.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/core/use-guards.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/apply-decorators.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/bind.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/bind.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/http/route-params.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/bind.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/catch.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/catch.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/catch.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/controller.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/controller.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/controller.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/create-param-decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/create-route-param-metadata.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/pipes/parse-int.pipe.ts"
      ],
      "confidence": "medium",
      "evidence": {
        "packages/common/decorators/http/create-route-param-metadata.decorator.ts": [
          "call_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/dependencies.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/dependencies.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/dependencies.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/exception-filters.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/exception-filters.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/validate-each.util.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/exception-filters.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/global.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/modules/global.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/modules/global.decorator.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/header.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/header.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/header.decorator.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/http-code.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/http-code.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/http-code.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/inject.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/inject.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/inject.decorator.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/injectable.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/injectable.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/injectable.decorator.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/module.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/modules/module.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/modules/module.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/redirect.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/redirect.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/redirect.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/render.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/render.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/render.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/request-mapping.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/request-mapping.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/request-mapping.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/route-params.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/route-params.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/common/enums/route-paramtypes.enum.ts",
        "packages/common/enums/request-method.enum.ts",
        "packages/common/pipes/parse-int.pipe.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/route-params.decorator.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "symbol_assertion"
        ]
      }
    },
    "packages/common/test/decorators/set-metadata.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/set-metadata.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/set-metadata.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/sse.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/http/sse.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/http/sse.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/use-guards.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/use-guards.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/validate-each.util.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/use-guards.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/use-interceptors.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/use-interceptors.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/validate-each.util.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/use-interceptors.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/use-pipes.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/use-pipes.decorator.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/validate-each.util.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/use-pipes.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/decorators/version.decorator.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/version.decorator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/version.decorator.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/exceptions/http.exception.spec.ts": {
      "primary_targets": [
        "packages/common/exceptions/http.exception.ts",
        "packages/common/exceptions/bad-request.exception.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/exceptions/http.exception.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/exceptions/bad-request.exception.ts": [
          "barrel_import",
          "constructor_usage",
          "symbol_assertion"
        ]
      }
    },
    "packages/common/test/file-stream/streamable-file.spec.ts": {
      "primary_targets": [
        "packages/common/file-stream/streamable-file.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/http-status.enum.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/file-stream/streamable-file.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/module-utils/configurable-module.builder.spec.ts": {
      "primary_targets": [
        "packages/common/module-utils/configurable-module.builder.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/module-utils/configurable-module.builder.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/module-utils/utils/get-injection-providers.util.spec.ts": {
      "primary_targets": [
        "packages/common/module-utils/utils/get-injection-providers.util.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/modules/provider.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/module-utils/utils/get-injection-providers.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/default-value.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/default-value.pipe.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/default-value.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/file/file-type.validator.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/file/file-type.validator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/file/file-type.validator.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/file/max-file-size.validator.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/file/max-file-size.validator.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/file/max-file-size.validator.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/file/parse-file-pipe.builder.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/file/file-type.validator.ts",
        "packages/common/pipes/file/parse-file-pipe.builder.ts"
      ],
      "secondary_targets": [
        "packages/common/pipes/file/file-validator.interface.ts",
        "packages/common/pipes/file/max-file-size.validator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/file/file-type.validator.ts": [
          "barrel_import",
          "call_usage",
          "constructor_usage",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/pipes/file/parse-file-pipe.builder.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/file/parse-file.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/file/parse-file.pipe.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/file/parse-file.pipe.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-array.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-array.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts",
        "packages/common/exceptions/bad-request.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-array.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-bool.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-bool.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-bool.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-date.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-date.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/bad-request.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-date.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-enum.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-enum.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-enum.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-float.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-float.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-float.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-int.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-int.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-int.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/parse-uuid.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/parse-uuid.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/features/pipe-transform.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/parse-uuid.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/pipes/validation.pipe.spec.ts": {
      "primary_targets": [
        "packages/common/pipes/validation.pipe.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/unprocessable-entity.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/pipes/validation.pipe.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/serializer/class-serializer.interceptor.spec.ts": {
      "primary_targets": [
        "packages/common/serializer/class-serializer.interceptor.ts"
      ],
      "secondary_targets": [
        "packages/common/file-stream/streamable-file.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/serializer/class-serializer.interceptor.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/services/logger.service.spec.ts": {
      "primary_targets": [
        "packages/common/services/logger.service.ts",
        "packages/common/services/console-logger.service.ts"
      ],
      "secondary_targets": [
        "packages/common/services/logger.service.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/services/logger.service.ts": [
          "barrel_import",
          "call_usage",
          "constructor_usage",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/services/console-logger.service.ts": [
          "barrel_import",
          "call_usage",
          "constructor_usage",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/services/utils/filter-log-levels.util.spec.ts": {
      "primary_targets": [
        "packages/common/services/utils/filter-log-levels.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/services/utils/filter-log-levels.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/services/utils/is-log-level-enabled.util.spec.ts": {
      "primary_targets": [
        "packages/common/services/utils/is-log-level-enabled.util.ts",
        "packages/common/services/logger.service.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/services/utils/is-log-level-enabled.util.ts": [
          "barrel_import",
          "call_usage",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/services/logger.service.ts": [
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/forward-ref.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/forward-ref.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/forward-ref.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/load-package.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/load-package.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/load-package.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/merge-with-values.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/merge-with-values.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/merge-with-values.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/random-string-generator.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/random-string-generator.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/random-string-generator.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/select-exception-filter-metadata.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/select-exception-filter-metadata.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/select-exception-filter-metadata.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/shared.utils.spec.ts": {
      "primary_targets": [
        "packages/common/utils/shared.utils.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/shared.utils.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/common/test/utils/validate-each.util.spec.ts": {
      "primary_targets": [
        "packages/common/utils/validate-each.util.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/shared.utils.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/utils/validate-each.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/application-config.spec.ts": {
      "primary_targets": [
        "packages/core/application-config.ts"
      ],
      "secondary_targets": [
        "packages/common/interfaces/global-prefix-options.interface.ts",
        "packages/core/router/interfaces/exclude-route-metadata.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/application-config.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/discovery/discoverable-meta-host-collection.spec.ts": {
      "primary_targets": [
        "packages/core/discovery/discoverable-meta-host-collection.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/modules-container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/discovery/discoverable-meta-host-collection.ts": [
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/discovery/discovery-service.spec.ts": {
      "primary_targets": [
        "packages/core/discovery/discovery-service.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/injector/modules-container.ts",
        "packages/core/discovery/discoverable-meta-host-collection.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/discovery/discovery-service.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/errors/test/exception-handler.spec.ts": {
      "primary_targets": [
        "packages/core/errors/exception-handler.ts"
      ],
      "secondary_targets": [
        "packages/core/errors/exceptions/runtime.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/errors/exception-handler.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/errors/test/exceptions-zone.spec.ts": {
      "primary_targets": [
        "packages/core/errors/exceptions-zone.ts"
      ],
      "secondary_targets": [
        "packages/common/services/logger.service.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/errors/exceptions-zone.ts": [
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/errors/test/messages.spec.ts": {
      "primary_targets": [
        "packages/core/errors/messages.ts"
      ],
      "secondary_targets": [
        "packages/core/errors/exceptions/unknown-dependencies.exception.ts",
        "packages/core/helpers/messages.ts",
        "packages/core/injector/module.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/errors/messages.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/exceptions/base-exception-filter.spec.ts": {
      "primary_targets": [
        "packages/core/exceptions/base-exception-filter-context.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/exceptions/base-exception-filter.ts"
      ],
      "confidence": "medium",
      "evidence": {
        "packages/core/exceptions/base-exception-filter-context.ts": [
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/exceptions/exceptions-handler.spec.ts": {
      "primary_targets": [
        "packages/core/exceptions/exceptions-handler.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/http.exception.ts",
        "packages/common/utils/shared.utils.ts",
        "packages/core/helpers/execution-context-host.ts",
        "packages/core/errors/exceptions/invalid-exception-filter.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/exceptions/exceptions-handler.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/exceptions/external-exception-filter-context.spec.ts": {
      "primary_targets": [
        "packages/core/exceptions/external-exception-filter-context.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/core/exception-filters.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/common/decorators/core/catch.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/exceptions/external-exception-filter-context.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/exceptions/external-exceptions-handler.spec.ts": {
      "primary_targets": [
        "packages/core/exceptions/external-exceptions-handler.ts"
      ],
      "secondary_targets": [
        "packages/core/exceptions/external-exception-filter.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/exceptions/external-exceptions-handler.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/guards/guards-consumer.spec.ts": {
      "primary_targets": [
        "packages/core/guards/guards-consumer.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/guards/guards-consumer.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/guards/guards-context-creator.spec.ts": {
      "primary_targets": [
        "packages/core/guards/guards-context-creator.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/guards/guards-context-creator.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/application-ref-host.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/http-adapter-host.ts"
      ],
      "secondary_targets": [],
      "confidence": "medium",
      "evidence": {
        "packages/core/helpers/http-adapter-host.ts": [
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/barrier.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/barrier.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/barrier.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/context-id-factory.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/context-id-factory.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/context-id-factory.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/context-utils.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/context-utils.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/route-paramtypes.enum.ts",
        "packages/core/helpers/execution-context-host.ts",
        "packages/common/decorators/http/route-params.decorator.ts",
        "packages/common/decorators/http/create-route-param-metadata.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/context-utils.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/execution-context-host.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/execution-context-host.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/execution-context-host.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/external-context-creator.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/external-context-creator.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/forbidden.exception.ts",
        "packages/core/exceptions/external-exception-filter-context.ts",
        "packages/core/guards/guards-consumer.ts",
        "packages/core/guards/guards-context-creator.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/modules-container.ts",
        "packages/core/interceptors/interceptors-consumer.ts",
        "packages/core/interceptors/interceptors-context-creator.ts",
        "packages/core/pipes/pipes-consumer.ts",
        "packages/core/pipes/pipes-context-creator.ts",
        "packages/core/router/route-params-factory.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/external-context-creator.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/external-proxy.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/external-proxy.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/http.exception.ts",
        "packages/core/exceptions/external-exceptions-handler.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/external-proxy.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/helpers/router-method-factory.spec.ts": {
      "primary_targets": [
        "packages/core/helpers/router-method-factory.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/helpers/router-method-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/hooks/before-app-shutdown.hook.spec.ts": {
      "primary_targets": [
        "packages/core/hooks/before-app-shutdown.hook.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/hooks/before-app-shutdown.hook.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import"
        ]
      }
    },
    "packages/core/test/hooks/on-app-bootstrap.hook.spec.ts": {
      "primary_targets": [
        "packages/core/hooks/on-app-bootstrap.hook.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/hooks/on-app-bootstrap.hook.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/hooks/on-app-shutdown.hook.spec.ts": {
      "primary_targets": [
        "packages/core/hooks/on-app-shutdown.hook.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/hooks/on-app-shutdown.hook.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import"
        ]
      }
    },
    "packages/core/test/hooks/on-module-destroy.hook.spec.ts": {
      "primary_targets": [
        "packages/core/hooks/on-module-destroy.hook.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/hooks/on-module-destroy.hook.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/hooks/on-module-init.hook.spec.ts": {
      "primary_targets": [
        "packages/core/hooks/on-module-init.hook.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/hooks/on-module-init.hook.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/compiler.spec.ts": {
      "primary_targets": [
        "packages/core/injector/compiler.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/opaque-key-factory/by-reference-module-opaque-key-factory.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/compiler.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/container.spec.ts": {
      "primary_targets": [
        "packages/core/injector/container.ts"
      ],
      "secondary_targets": [
        "packages/core/middleware/container.ts",
        "packages/common/decorators/modules/module.decorator.ts",
        "packages/common/decorators/modules/global.decorator.ts",
        "packages/core/errors/exceptions/circular-dependency.exception.ts",
        "packages/core/errors/exceptions/unknown-module.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/container.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/helpers/provider-classifier.spec.ts": {
      "primary_targets": [
        "packages/core/injector/helpers/provider-classifier.ts",
        "packages/common/interfaces/modules/provider.interface.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/helpers/provider-classifier.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/interfaces/modules/provider.interface.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/helpers/silent-logger.spec.ts": {
      "primary_targets": [
        "packages/core/injector/helpers/silent-logger.ts",
        "packages/common/services/logger.service.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/helpers/silent-logger.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/services/logger.service.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/injector.spec.ts": {
      "primary_targets": [
        "packages/core/injector/injector.ts",
        "packages/common/decorators/core/inject.decorator.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/injector.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/settlement-signal.ts",
        "packages/common/decorators/core/optional.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/injector.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/decorators/core/inject.decorator.ts": [
          "call_usage",
          "direct_import",
          "test_name_match"
        ],
        "packages/common/decorators/core/injectable.decorator.ts": [
          "call_usage",
          "direct_import",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import"
        ]
      }
    },
    "packages/core/test/injector/instance-loader.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/core/injector/instance-loader.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/injector/injector.ts",
        "packages/core/inspector/graph-inspector.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/core/controller.decorator.ts": [
          "call_usage",
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/injector/instance-loader.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/decorators/core/injectable.decorator.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ]
      }
    },
    "packages/core/test/injector/instance-wrapper.spec.ts": {
      "primary_targets": [
        "packages/core/injector/instance-wrapper.ts",
        "packages/common/interfaces/scope-options.interface.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/constants.ts",
        "packages/core/helpers/context-id-factory.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/interfaces/scope-options.interface.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/internal-core-module/internal-core-module-factory.spec.ts": {
      "primary_targets": [
        "packages/core/injector/internal-core-module/internal-core-module-factory.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/helpers/external-context-creator.ts",
        "packages/core/helpers/http-adapter-host.ts",
        "packages/core/injector/internal-core-module/internal-core-module.ts",
        "packages/core/inspector/serialized-graph.ts",
        "packages/core/injector/lazy-module-loader/lazy-module-loader.ts",
        "packages/core/injector/modules-container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/internal-core-module/internal-core-module-factory.ts": [
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/lazy-module-loader/lazy-module-loader.spec.ts": {
      "primary_targets": [
        "packages/core/injector/lazy-module-loader/lazy-module-loader.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/module-ref.ts",
        "packages/core/injector/injector.ts",
        "packages/core/injector/instance-loader.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/metadata-scanner.ts",
        "packages/core/scanner.ts",
        "packages/core/injector/container.ts",
        "packages/common/decorators/modules/module.decorator.ts",
        "packages/core/injector/modules-container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/lazy-module-loader/lazy-module-loader.ts": [
          "barrel_import",
          "constructor_usage",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/module.spec.ts": {
      "primary_targets": [
        "packages/core/injector/module.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/core/errors/exceptions/runtime.exception.ts",
        "packages/common/interfaces/scope-options.interface.ts",
        "packages/common/decorators/modules/module.decorator.ts",
        "packages/core/injector/container.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/errors/exceptions/unknown-element.exception.ts",
        "packages/core/errors/exceptions/unknown-export.exception.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ]
      }
    },
    "packages/core/test/injector/nested-transient-isolation.spec.ts": {
      "primary_targets": [],
      "secondary_targets": [
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/injector.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/injector/module.ts",
        "packages/common/interfaces/scope-options.interface.ts"
      ],
      "confidence": "uncertain",
      "evidence": {}
    },
    "packages/core/test/injector/opaque-key-factory/by-reference-module-opaque-key-factory.spec.ts": {
      "primary_targets": [
        "packages/core/injector/opaque-key-factory/by-reference-module-opaque-key-factory.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/opaque-key-factory/by-reference-module-opaque-key-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/opaque-key-factory/deep-hashed-module-opaque-key-factory.spec.ts": {
      "primary_targets": [
        "packages/core/injector/opaque-key-factory/deep-hashed-module-opaque-key-factory.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/opaque-key-factory/deep-hashed-module-opaque-key-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/injector/topology-tree/tree-node.spec.ts": {
      "primary_targets": [
        "packages/core/injector/topology-tree/tree-node.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/topology-tree/tree-node.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/inspector/graph-inspector.spec.ts": {
      "primary_targets": [
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/injector/module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/inspector/serialized-graph.ts",
        "packages/core/inspector/interfaces/enhancer-metadata-cache-entry.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/inspector/graph-inspector.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ],
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/inspector/serialized-graph.spec.ts": {
      "primary_targets": [
        "packages/core/inspector/interfaces/node.interface.ts",
        "packages/core/inspector/serialized-graph.ts",
        "packages/core/inspector/interfaces/edge.interface.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/inspector/interfaces/node.interface.ts": [
          "call_usage",
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/inspector/serialized-graph.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/inspector/interfaces/edge.interface.ts": [
          "call_usage",
          "direct_import",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/interceptors/interceptors-consumer.spec.ts": {
      "primary_targets": [
        "packages/core/interceptors/interceptors-consumer.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/interceptors/interceptors-consumer.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/interceptors/interceptors-context-creator.spec.ts": {
      "primary_targets": [
        "packages/core/interceptors/interceptors-context-creator.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/interceptors/interceptors-context-creator.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/metadata-scanner.spec.ts": {
      "primary_targets": [
        "packages/core/metadata-scanner.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/metadata-scanner.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/builder.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/builder.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/middleware/route-info-path-extractor.ts",
        "packages/core/middleware/routes-mapper.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/common/decorators/core/version.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/builder.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/container.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/container.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/instance-wrapper.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/injector/module.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/container.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/middleware-module.spec.ts": {
      "primary_targets": [
        "packages/core/injector/module.ts",
        "packages/core/middleware/middleware-module.ts",
        "packages/core/middleware/builder.ts",
        "packages/core/middleware/container.ts"
      ],
      "secondary_targets": [
        "packages/core/errors/exceptions/invalid-middleware.exception.ts",
        "packages/core/errors/exceptions/runtime.exception.ts",
        "packages/core/middleware/route-info-path-extractor.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/router/router-exception-filters.ts",
        "packages/common/decorators/core/injectable.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/injector/module.ts": [
          "call_usage",
          "constructor_usage",
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/middleware/middleware-module.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/middleware/builder.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ],
        "packages/core/middleware/container.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ]
      }
    },
    "packages/core/test/middleware/resolver.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/resolver.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/injector.ts",
        "packages/core/middleware/container.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/resolver.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/route-info-path-extractor.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/route-info-path-extractor.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts",
        "packages/core/middleware/utils.ts",
        "packages/core/application-config.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/route-info-path-extractor.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/routes-mapper.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/routes-mapper.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/core/version.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/routes-mapper.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/middleware/utils.spec.ts": {
      "primary_targets": [
        "packages/core/middleware/utils.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/shared.utils.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/middleware/utils.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/nest-application-context.spec.ts": {
      "primary_targets": [
        "packages/core/nest-application-context.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/injector/injector.ts",
        "packages/core/injector/instance-loader.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/common/interfaces/modules/provider.interface.ts",
        "packages/common/interfaces/scope-options.interface.ts",
        "packages/core/helpers/context-id-factory.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/nest-application-context.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/nest-application.spec.ts": {
      "primary_targets": [
        "packages/core/nest-application.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/middleware/utils.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/nest-application.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/pipes/params-token-factory.spec.ts": {
      "primary_targets": [
        "packages/core/pipes/params-token-factory.ts",
        "packages/common/enums/route-paramtypes.enum.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/pipes/params-token-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/enums/route-paramtypes.enum.ts": [
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/pipes/pipes-consumer.spec.ts": {
      "primary_targets": [
        "packages/core/pipes/pipes-consumer.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/pipes/pipes-consumer.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/pipes/pipes-context-creator.spec.ts": {
      "primary_targets": [
        "packages/core/pipes/pipes-context-creator.ts"
      ],
      "secondary_targets": [
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/pipes/pipes-context-creator.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/assign-to-object.util.spec.ts": {
      "primary_targets": [
        "packages/core/repl/assign-to-object.util.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/assign-to-object.util.ts": [
          "call_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/debug-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/debug-repl-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/repl/repl-context.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/debug-repl-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/get-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/get-repl-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/repl/repl-context.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/get-repl-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/help-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/help-repl-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/repl/repl-context.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/help-repl-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/methods-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/methods-repl-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts",
        "packages/core/repl/repl-context.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/methods-repl-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/resolve-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/resolve-repl-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/repl/repl-context.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/resolve-repl-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/native-functions/select-repl-fn.spec.ts": {
      "primary_targets": [
        "packages/core/repl/native-functions/select-relp-fn.ts"
      ],
      "secondary_targets": [
        "packages/core/repl/repl-context.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/native-functions/select-relp-fn.ts": [
          "barrel_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/repl/repl-context.spec.ts": {
      "primary_targets": [
        "packages/core/repl/repl-context.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/repl/repl-context.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/paths-explorer.spec.ts": {
      "primary_targets": [
        "packages/core/router/paths-explorer.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/metadata-scanner.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/paths-explorer.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/route-params-factory.spec.ts": {
      "primary_targets": [
        "packages/core/router/route-params-factory.ts",
        "packages/common/enums/route-paramtypes.enum.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/router/route-params-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/enums/route-paramtypes.enum.ts": [
          "direct_import",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/route-path-factory.spec.ts": {
      "primary_targets": [
        "packages/core/router/route-path-factory.ts"
      ],
      "secondary_targets": [
        "packages/common/enums/request-method.enum.ts",
        "packages/common/interfaces/version-options.interface.ts",
        "packages/common/enums/version-type.enum.ts",
        "packages/core/application-config.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/route-path-factory.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/router-exception-filters.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-exception-filters.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/core/exception-filters.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/common/decorators/core/catch.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-exception-filters.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/router-execution-context.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-execution-context.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/forbidden.exception.ts",
        "packages/common/enums/route-paramtypes.enum.ts",
        "packages/core/application-config.ts",
        "packages/core/guards/guards-consumer.ts",
        "packages/core/guards/guards-context-creator.ts",
        "packages/core/injector/container.ts",
        "packages/core/interceptors/interceptors-consumer.ts",
        "packages/core/interceptors/interceptors-context-creator.ts",
        "packages/core/pipes/pipes-consumer.ts",
        "packages/core/pipes/pipes-context-creator.ts",
        "packages/core/router/route-params-factory.ts",
        "packages/core/helpers/handler-metadata-storage.ts",
        "packages/core/router/sse-stream.ts",
        "packages/common/decorators/http/route-params.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-execution-context.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/router-explorer.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-explorer.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/injector/injector.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/metadata-scanner.ts",
        "packages/core/router/route-path-factory.ts",
        "packages/core/router/router-exception-filters.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/core/helpers/execution-context-host.ts",
        "packages/core/errors/exceptions/unknown-request-mapping.exception.ts",
        "packages/core/router/interfaces/route-path-metadata.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-explorer.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/router-module.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-module.ts"
      ],
      "secondary_targets": [
        "packages/core/injector/modules-container.ts",
        "packages/core/injector/container.ts",
        "packages/core/injector/module.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-module.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "provider_registration",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/router-proxy.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-proxy.ts",
        "packages/core/helpers/execution-context-host.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/http.exception.ts",
        "packages/core/exceptions/exceptions-handler.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-proxy.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/core/helpers/execution-context-host.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ]
      }
    },
    "packages/core/test/router/router-response-controller.spec.ts": {
      "primary_targets": [
        "packages/core/router/router-response-controller.ts",
        "packages/common/enums/request-method.enum.ts"
      ],
      "secondary_targets": [
        "packages/common/utils/shared.utils.ts",
        "packages/common/enums/http-status.enum.ts",
        "packages/core/router/sse-stream.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/router-response-controller.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/enums/request-method.enum.ts": [
          "barrel_import",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/routes-resolver.spec.ts": {
      "primary_targets": [
        "packages/core/router/routes-resolver.ts"
      ],
      "secondary_targets": [
        "packages/common/exceptions/bad-request.exception.ts",
        "packages/common/exceptions/http.exception.ts",
        "packages/common/decorators/http/request-mapping.decorator.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/injector/injector.ts",
        "packages/core/injector/instance-wrapper.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/inspector/serialized-graph.ts",
        "packages/common/decorators/modules/module.decorator.ts",
        "packages/core/injector/container.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/routes-resolver.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/sse-stream.spec.ts": {
      "primary_targets": [
        "packages/core/router/sse-stream.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/router/sse-stream.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/router/utils/flat-routes.spec.ts": {
      "primary_targets": [
        "packages/core/router/utils/flatten-route-paths.util.ts"
      ],
      "secondary_targets": [
        "packages/common/decorators/modules/module.decorator.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/core/router/utils/flatten-route-paths.util.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/scanner.spec.ts": {
      "primary_targets": [
        "packages/common/decorators/modules/module.decorator.ts",
        "packages/common/decorators/core/controller.decorator.ts",
        "packages/core/scanner.ts",
        "packages/common/decorators/core/injectable.decorator.ts",
        "packages/core/injector/instance-wrapper.ts"
      ],
      "secondary_targets": [
        "packages/core/errors/exceptions/invalid-class-module.exception.ts",
        "packages/core/errors/exceptions/invalid-module.exception.ts",
        "packages/core/errors/exceptions/undefined-module.exception.ts",
        "packages/common/decorators/core/use-guards.decorator.ts",
        "packages/core/application-config.ts",
        "packages/core/constants.ts",
        "packages/core/injector/container.ts",
        "packages/core/inspector/graph-inspector.ts",
        "packages/core/metadata-scanner.ts",
        "packages/common/decorators/core/catch.decorator.ts",
        "packages/common/interfaces/scope-options.interface.ts",
        "packages/core/interfaces/module-override.interface.ts"
      ],
      "confidence": "high",
      "evidence": {
        "packages/common/decorators/modules/module.decorator.ts": [
          "call_usage",
          "direct_import",
          "provider_registration",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/common/decorators/core/controller.decorator.ts": [
          "call_usage",
          "direct_import",
          "provider_registration",
          "symbol_assertion"
        ],
        "packages/core/scanner.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ],
        "packages/common/decorators/core/injectable.decorator.ts": [
          "barrel_import",
          "call_usage",
          "symbol_assertion",
          "test_name_match"
        ],
        "packages/core/injector/instance-wrapper.ts": [
          "constructor_usage",
          "direct_import",
          "symbol_assertion"
        ]
      }
    },
    "packages/core/test/services/reflector.service.spec.ts": {
      "primary_targets": [
        "packages/core/services/reflector.service.ts"
      ],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {
        "packages/core/services/reflector.service.ts": [
          "constructor_usage",
          "direct_import",
          "filename_match",
          "test_name_match"
        ]
      }
    },
    "packages/core/test/utils/noop-adapter.spec.ts": {
      "primary_targets": [],
      "secondary_targets": [],
      "confidence": "high",
      "evidence": {}
    }
  },
  "unmapped_production_files": [
    "packages/common/constants.ts",
    "packages/common/decorators/core/index.ts",
    "packages/common/decorators/http/index.ts",
    "packages/common/decorators/index.ts",
    "packages/common/decorators/modules/index.ts",
    "packages/common/enums/index.ts",
    "packages/common/enums/shutdown-signal.enum.ts",
    "packages/common/exceptions/bad-gateway.exception.ts",
    "packages/common/exceptions/conflict.exception.ts",
    "packages/common/exceptions/gateway-timeout.exception.ts",
    "packages/common/exceptions/gone.exception.ts",
    "packages/common/exceptions/http-version-not-supported.exception.ts",
    "packages/common/exceptions/im-a-teapot.exception.ts",
    "packages/common/exceptions/index.ts",
    "packages/common/exceptions/internal-server-error.exception.ts",
    "packages/common/exceptions/intrinsic.exception.ts",
    "packages/common/exceptions/method-not-allowed.exception.ts",
    "packages/common/exceptions/misdirected.exception.ts",
    "packages/common/exceptions/not-acceptable.exception.ts",
    "packages/common/exceptions/not-found.exception.ts",
    "packages/common/exceptions/not-implemented.exception.ts",
    "packages/common/exceptions/payload-too-large.exception.ts",
    "packages/common/exceptions/precondition-failed.exception.ts",
    "packages/common/exceptions/request-timeout.exception.ts",
    "packages/common/exceptions/service-unavailable.exception.ts",
    "packages/common/exceptions/unauthorized.exception.ts",
    "packages/common/exceptions/unsupported-media-type.exception.ts",
    "packages/common/file-stream/index.ts",
    "packages/common/file-stream/interfaces/index.ts",
    "packages/common/file-stream/interfaces/streamable-handler-response.interface.ts",
    "packages/common/file-stream/interfaces/streamable-options.interface.ts",
    "packages/common/index.ts",
    "packages/common/interfaces/abstract.interface.ts",
    "packages/common/interfaces/controllers/controller-metadata.interface.ts",
    "packages/common/interfaces/controllers/controller.interface.ts",
    "packages/common/interfaces/controllers/index.ts",
    "packages/common/interfaces/exceptions/exception-filter-metadata.interface.ts",
    "packages/common/interfaces/exceptions/exception-filter.interface.ts",
    "packages/common/interfaces/exceptions/index.ts",
    "packages/common/interfaces/exceptions/rpc-exception-filter-metadata.interface.ts",
    "packages/common/interfaces/exceptions/rpc-exception-filter.interface.ts",
    "packages/common/interfaces/exceptions/ws-exception-filter.interface.ts",
    "packages/common/interfaces/external/class-transform-options.interface.ts",
    "packages/common/interfaces/external/cors-options.interface.ts",
    "packages/common/interfaces/external/https-options.interface.ts",
    "packages/common/interfaces/external/transformer-package.interface.ts",
    "packages/common/interfaces/external/validation-error.interface.ts",
    "packages/common/interfaces/external/validator-options.interface.ts",
    "packages/common/interfaces/external/validator-package.interface.ts",
    "packages/common/interfaces/features/arguments-host.interface.ts",
    "packages/common/interfaces/features/can-activate.interface.ts",
    "packages/common/interfaces/features/custom-route-param-factory.interface.ts",
    "packages/common/interfaces/features/execution-context.interface.ts",
    "packages/common/interfaces/features/nest-interceptor.interface.ts",
    "packages/common/interfaces/features/paramtype.interface.ts",
    "packages/common/interfaces/hooks/before-application-shutdown.interface.ts",
    "packages/common/interfaces/hooks/index.ts",
    "packages/common/interfaces/hooks/on-application-bootstrap.interface.ts",
    "packages/common/interfaces/hooks/on-application-shutdown.interface.ts",
    "packages/common/interfaces/hooks/on-destroy.interface.ts",
    "packages/common/interfaces/hooks/on-init.interface.ts",
    "packages/common/interfaces/http/http-exception-body.interface.ts",
    "packages/common/interfaces/http/http-redirect-response.interface.ts",
    "packages/common/interfaces/http/http-server.interface.ts",
    "packages/common/interfaces/http/index.ts",
    "packages/common/interfaces/http/message-event.interface.ts",
    "packages/common/interfaces/http/raw-body-request.interface.ts",
    "packages/common/interfaces/index.ts",
    "packages/common/interfaces/injectable.interface.ts",
    "packages/common/interfaces/microservices/nest-hybrid-application-options.interface.ts",
    "packages/common/interfaces/microservices/nest-microservice-options.interface.ts",
    "packages/common/interfaces/middleware/index.ts",
    "packages/common/interfaces/middleware/middleware-config-proxy.interface.ts",
    "packages/common/interfaces/middleware/middleware-configuration.interface.ts",
    "packages/common/interfaces/middleware/middleware-consumer.interface.ts",
    "packages/common/interfaces/middleware/nest-middleware.interface.ts",
    "packages/common/interfaces/modules/dynamic-module.interface.ts",
    "packages/common/interfaces/modules/forward-reference.interface.ts",
    "packages/common/interfaces/modules/index.ts",
    "packages/common/interfaces/modules/injection-token.interface.ts",
    "packages/common/interfaces/modules/introspection-result.interface.ts",
    "packages/common/interfaces/modules/module-metadata.interface.ts",
    "packages/common/interfaces/modules/nest-module.interface.ts",
    "packages/common/interfaces/modules/optional-factory-dependency.interface.ts",
    "packages/common/interfaces/nest-application-context-options.interface.ts",
    "packages/common/interfaces/nest-application-context.interface.ts",
    "packages/common/interfaces/nest-application-options.interface.ts",
    "packages/common/interfaces/nest-application.interface.ts",
    "packages/common/interfaces/nest-microservice.interface.ts",
    "packages/common/interfaces/shutdown-hooks-options.interface.ts",
    "packages/common/interfaces/type.interface.ts",
    "packages/common/interfaces/websockets/web-socket-adapter.interface.ts",
    "packages/common/module-utils/constants.ts",
    "packages/common/module-utils/index.ts",
    "packages/common/module-utils/interfaces/configurable-module-async-options.interface.ts",
    "packages/common/module-utils/interfaces/configurable-module-cls.interface.ts",
    "packages/common/module-utils/interfaces/configurable-module-host.interface.ts",
    "packages/common/module-utils/interfaces/index.ts",
    "packages/common/module-utils/utils/generate-options-injection-token.util.ts",
    "packages/common/module-utils/utils/index.ts",
    "packages/common/pipes/file/file-validator-context.interface.ts",
    "packages/common/pipes/file/index.ts",
    "packages/common/pipes/file/interfaces/file.interface.ts",
    "packages/common/pipes/file/interfaces/index.ts",
    "packages/common/pipes/file/parse-file-options.interface.ts",
    "packages/common/pipes/index.ts",
    "packages/common/serializer/class-serializer.constants.ts",
    "packages/common/serializer/class-serializer.interfaces.ts",
    "packages/common/serializer/decorators/index.ts",
    "packages/common/serializer/decorators/serialize-options.decorator.ts",
    "packages/common/serializer/index.ts",
    "packages/common/services/index.ts",
    "packages/common/services/utils/index.ts",
    "packages/common/services/utils/is-log-level.util.ts",
    "packages/common/utils/assign-custom-metadata.util.ts",
    "packages/common/utils/cli-colors.util.ts",
    "packages/common/utils/extend-metadata.util.ts",
    "packages/common/utils/http-error-by-code.util.ts",
    "packages/common/utils/index.ts",
    "packages/common/utils/validate-module-keys.util.ts",
    "packages/core/adapters/http-adapter.ts",
    "packages/core/adapters/index.ts",
    "packages/core/discovery/discovery-module.ts",
    "packages/core/discovery/index.ts",
    "packages/core/errors/exceptions/circular-dependency.exception.ts",
    "packages/core/errors/exceptions/index.ts",
    "packages/core/errors/exceptions/invalid-class-scope.exception.ts",
    "packages/core/errors/exceptions/invalid-class.exception.ts",
    "packages/core/errors/exceptions/invalid-exception-filter.exception.ts",
    "packages/core/errors/exceptions/invalid-middleware-configuration.exception.ts",
    "packages/core/errors/exceptions/undefined-dependency.exception.ts",
    "packages/core/errors/exceptions/undefined-forwardref.exception.ts",
    "packages/core/errors/exceptions/unknown-element.exception.ts",
    "packages/core/errors/exceptions/unknown-export.exception.ts",
    "packages/core/errors/exceptions/unknown-module.exception.ts",
    "packages/core/errors/exceptions/unknown-request-mapping.exception.ts",
    "packages/core/exceptions/external-exception-filter.ts",
    "packages/core/exceptions/index.ts",
    "packages/core/guards/constants.ts",
    "packages/core/guards/index.ts",
    "packages/core/helpers/context-creator.ts",
    "packages/core/helpers/get-class-scope.ts",
    "packages/core/helpers/handler-metadata-storage.ts",
    "packages/core/helpers/index.ts",
    "packages/core/helpers/interfaces/external-handler-metadata.interface.ts",
    "packages/core/helpers/interfaces/index.ts",
    "packages/core/helpers/interfaces/params-metadata.interface.ts",
    "packages/core/helpers/is-durable.ts",
    "packages/core/helpers/load-adapter.ts",
    "packages/core/helpers/optional-require.ts",
    "packages/core/helpers/rethrow.ts",
    "packages/core/hooks/index.ts",
    "packages/core/index.ts",
    "packages/core/injector/abstract-instance-resolver.ts",
    "packages/core/injector/helpers/transient-instances.ts",
    "packages/core/injector/index.ts",
    "packages/core/injector/inquirer/index.ts",
    "packages/core/injector/inquirer/inquirer-constants.ts",
    "packages/core/injector/inquirer/inquirer-providers.ts",
    "packages/core/injector/instance-links-host.ts",
    "packages/core/injector/internal-core-module/index.ts",
    "packages/core/injector/internal-core-module/internal-core-module.ts",
    "packages/core/injector/internal-providers-storage.ts",
    "packages/core/injector/lazy-module-loader/lazy-module-loader-options.interface.ts",
    "packages/core/injector/opaque-key-factory/interfaces/module-opaque-key-factory.interface.ts",
    "packages/core/injector/topology-tree/topology-tree.ts",
    "packages/core/inspector/deterministic-uuid-registry.ts",
    "packages/core/inspector/index.ts",
    "packages/core/inspector/initialize-on-preview.allowlist.ts",
    "packages/core/inspector/interfaces/enhancer-metadata-cache-entry.interface.ts",
    "packages/core/inspector/interfaces/entrypoint.interface.ts",
    "packages/core/inspector/interfaces/extras.interface.ts",
    "packages/core/inspector/interfaces/serialized-graph-json.interface.ts",
    "packages/core/inspector/interfaces/serialized-graph-metadata.interface.ts",
    "packages/core/inspector/noop-graph-inspector.ts",
    "packages/core/inspector/partial-graph.host.ts",
    "packages/core/inspector/uuid-factory.ts",
    "packages/core/interceptors/index.ts",
    "packages/core/interfaces/module-definition.interface.ts",
    "packages/core/interfaces/module-override.interface.ts",
    "packages/core/middleware/index.ts",
    "packages/core/nest-factory.ts",
    "packages/core/pipes/index.ts",
    "packages/core/repl/constants.ts",
    "packages/core/repl/index.ts",
    "packages/core/repl/native-functions/index.ts",
    "packages/core/repl/repl-function.ts",
    "packages/core/repl/repl-logger.ts",
    "packages/core/repl/repl-native-commands.ts",
    "packages/core/repl/repl.interfaces.ts",
    "packages/core/repl/repl.ts",
    "packages/core/router/index.ts",
    "packages/core/router/interfaces/exceptions-filter.interface.ts",
    "packages/core/router/interfaces/exclude-route-metadata.interface.ts",
    "packages/core/router/interfaces/index.ts",
    "packages/core/router/interfaces/resolver.interface.ts",
    "packages/core/router/interfaces/route-params-factory.interface.ts",
    "packages/core/router/interfaces/route-path-metadata.interface.ts",
    "packages/core/router/interfaces/routes.interface.ts",
    "packages/core/router/legacy-route-converter.ts",
    "packages/core/router/request/index.ts",
    "packages/core/router/request/request-constants.ts",
    "packages/core/router/request/request-providers.ts",
    "packages/core/router/utils/exclude-route.util.ts",
    "packages/core/router/utils/index.ts",
    "packages/core/services/index.ts"
  ]
}
```
