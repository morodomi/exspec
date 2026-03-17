;; use crate::module::Symbol;
;; use crate::module::{Symbol1, Symbol2};
;; use crate::module::*;
(use_declaration
  argument: (scoped_identifier) @use_path)

;; use crate::module::{Symbol1, Symbol2};
(use_declaration
  argument: (use_as_clause
    path: (scoped_identifier) @use_as_path))

;; use list: use crate::module::{A, B};
(use_declaration
  argument: (scoped_use_list
    path: (scoped_identifier) @use_list_path
    list: (use_list) @use_list))
