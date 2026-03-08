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

;; Match expect.soft(...).toX(), expect.element(...).toX(), expect.poll(...).toX()
(call_expression
  function: (member_expression
    object: (call_expression
      function: (member_expression
        object: (identifier) @_fn4
        property: (property_identifier) @_prop4
        (#eq? @_fn4 "expect")
        (#match? @_prop4 "^(soft|element|poll)$"))))) @assertion

;; Match assert.* (Node assert module)
(call_expression
  function: (member_expression
    object: (identifier) @obj
    (#match? @obj "^assert$"))) @assertion
