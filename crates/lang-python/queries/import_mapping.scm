;; Named imports: from .module import Symbol
;;   from myapp.models import User
;;   from .models import User
;;   from . import views  (module_name is just ".", symbol is the module)
(import_from_statement
  module_name: (_) @module_name
  name: (dotted_name
    (identifier) @symbol_name))

;; import os  (plain import, no from) - captured for completeness but skipped in Rust
(import_statement
  name: (dotted_name
    (identifier) @import_name))
