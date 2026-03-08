;; Match expect(...).toBe(...) and similar
(call_expression
  function: (member_expression
    object: (call_expression
      function: (identifier) @fn
      (#match? @fn "^expect$")))) @assertion

;; Match expect(...).rejects.toThrow(...) — chained rejects pattern
;; (also matched in error_test.scm for T103)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_fn2
        (#match? @_fn2 "^expect$"))
      property: (property_identifier) @_prop
      (#eq? @_prop "rejects")))) @assertion

;; Match expectTypeOf(...).toEqualTypeOf<T>() and similar
(call_expression
  function: (member_expression
    object: (call_expression
      function: (identifier) @_fn3
      (#eq? @_fn3 "expectTypeOf")))) @assertion

;; Match assert.* (Node assert module)
(call_expression
  function: (member_expression
    object: (identifier) @obj
    (#match? @obj "^assert$"))) @assertion
