; use proptest; or use quickcheck;
(use_declaration
  argument: (identifier) @_name
  (#match? @_name "^(proptest|quickcheck)$")) @pbt_import

; use proptest::SomeItem; or use quickcheck::SomeItem;
(use_declaration
  argument: (scoped_identifier
    path: (identifier) @_name
    (#match? @_name "^(proptest|quickcheck)"))) @pbt_import

; use proptest::prelude::*; (use_wildcard contains scoped_identifier)
(use_declaration
  argument: (use_wildcard
    (scoped_identifier
      path: (identifier) @_name
      (#match? @_name "^(proptest|quickcheck)")))) @pbt_import

; use proptest::prelude::{...}; (scoped_use_list)
(use_declaration
  argument: (scoped_use_list
    (scoped_identifier
      path: (identifier) @_name
      (#match? @_name "^(proptest|quickcheck)")))) @pbt_import
