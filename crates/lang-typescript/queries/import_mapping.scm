;; Named imports: import { A, B } from './module'
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @symbol_name)))
  source: (string
    (string_fragment) @module_specifier))

;; Default import: import A from './module'
(import_statement
  (import_clause
    (identifier) @symbol_name)
  source: (string
    (string_fragment) @module_specifier))

;; Namespace import: import * as Ns from './module'
(import_statement
  (import_clause
    (namespace_import
      (identifier) @symbol_name))
  source: (string
    (string_fragment) @module_specifier))
