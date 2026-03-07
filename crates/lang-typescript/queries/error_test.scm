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

; .rejects (property access within expect chain)
(member_expression
  property: (property_identifier) @_prop
  (#eq? @_prop "rejects")) @error_test
