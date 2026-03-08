; Detect error/exception testing patterns in TypeScript

; .toThrow()
(call_expression
  function: (member_expression
    property: (property_identifier) @_method)
  (#eq? @_method "toThrow")) @error_test

; .toThrowError(...)
(call_expression
  function: (member_expression
    property: (property_identifier) @_method2)
  (#eq? @_method2 "toThrowError")) @error_test

; expect(...).rejects — also in assertion.scm for T001
(member_expression
  object: (call_expression
    function: (identifier) @_fn)
  property: (property_identifier) @_prop
  (#eq? @_fn "expect")
  (#eq? @_prop "rejects")) @error_test
