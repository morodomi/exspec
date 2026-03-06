; PHPUnit: $this->assert*()
(member_call_expression
  object: (variable_name (name) @_obj)
  name: (name) @_method
  (#eq? @_obj "this")
  (#match? @_method "^assert")) @assertion

; Pest: expect(...)->toBe(...) and similar
(member_call_expression
  object: (function_call_expression
    function: (name) @_fn
    (#eq? @_fn "expect"))
  name: (name) @_method
  (#match? @_method "^to")) @assertion
