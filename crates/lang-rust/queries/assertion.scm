; assert!, assert_eq!, assert_ne! macros
(macro_invocation
  macro: (identifier) @_name
  (#match? @_name "^(assert|assert_eq|assert_ne|debug_assert|debug_assert_eq|debug_assert_ne)$")) @assertion
