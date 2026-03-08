; PHPUnit: $this->assert*()
(member_call_expression
  object: (variable_name (name) @_obj)
  name: (name) @_method
  (#eq? @_obj "this")
  (#match? @_method "^assert")) @assertion

; PHPUnit: $this->expectException() — exception verification counts as assertion
; (also matched in error_test.scm for T103)
(member_call_expression
  object: (variable_name (name) @_expect_obj)
  name: (name) @_expect_method
  (#eq? @_expect_obj "this")
  (#eq? @_expect_method "expectException")) @assertion

; PHPUnit: $this->expectExceptionMessage()
; (also matched in error_test.scm for T103)
(member_call_expression
  object: (variable_name (name) @_expect_obj2)
  name: (name) @_expect_method2
  (#eq? @_expect_obj2 "this")
  (#eq? @_expect_method2 "expectExceptionMessage")) @assertion

; PHPUnit: $this->expectExceptionCode()
; (also matched in error_test.scm for T103)
(member_call_expression
  object: (variable_name (name) @_expect_obj3)
  name: (name) @_expect_method3
  (#eq? @_expect_obj3 "this")
  (#eq? @_expect_method3 "expectExceptionCode")) @assertion

; Pest: expect(...)->toBe(...) and similar
(member_call_expression
  object: (function_call_expression
    function: (name) @_fn
    (#eq? @_fn "expect"))
  name: (name) @_method
  (#match? @_method "^to")) @assertion
