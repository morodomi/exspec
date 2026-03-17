;; Re-export patterns in __init__.py:
;;   from .module import Foo
;;   from .module import Foo, Bar

;; Named re-export: from .module import Symbol
(import_from_statement
  module_name: (_) @from_specifier
  name: (dotted_name
    (identifier) @symbol_name))
