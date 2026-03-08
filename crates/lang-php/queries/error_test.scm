; Detect error/exception testing patterns in PHP

; $this->expectException(...) — also in assertion.scm for T001
(member_call_expression
  object: (variable_name) @_obj
  name: (name) @_method
  (#eq? @_obj "$this")
  (#eq? @_method "expectException")) @error_test

; $this->expectExceptionMessage(...) — also in assertion.scm for T001
(member_call_expression
  object: (variable_name) @_obj2
  name: (name) @_method2
  (#eq? @_obj2 "$this")
  (#eq? @_method2 "expectExceptionMessage")) @error_test

; $this->expectExceptionCode(...) — also in assertion.scm for T001
(member_call_expression
  object: (variable_name) @_obj3
  name: (name) @_method3
  (#eq? @_obj3 "$this")
  (#eq? @_method3 "expectExceptionCode")) @error_test

; Pest: ->toThrow(...)
(member_call_expression
  name: (name) @_pest_method
  (#eq? @_pest_method "toThrow")) @error_test
