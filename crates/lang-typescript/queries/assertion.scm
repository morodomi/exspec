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

;; Chai BDD property-style assertions (no trailing parentheses).
;; Terminal property allowlist: ok|true|false|null|undefined|exist|exists|empty|NaN|
;;   extensible|sealed|frozen|arguments|Arguments|finite|
;;   calledOnce|calledTwice|calledThrice|called|notCalled
;; tree-sitter has no recursive matching, so depths 1-5 are enumerated explicitly.

;; Depth 1: expect(x).TERMINAL
(expression_statement
  (member_expression
    object: (call_expression
      function: (identifier) @_chai1
      (#match? @_chai1 "^expect$"))
    property: (property_identifier) @_term1
    (#match? @_term1 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled)$"))) @assertion

;; Depth 2: expect(x).chain.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_chai2
        (#match? @_chai2 "^expect$")))
    property: (property_identifier) @_term2
    (#match? @_term2 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled)$"))) @assertion

;; Depth 3: expect(x).a.b.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_chai3
          (#match? @_chai3 "^expect$"))))
    property: (property_identifier) @_term3
    (#match? @_term3 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled)$"))) @assertion

;; Depth 4: expect(x).a.b.c.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (call_expression
            function: (identifier) @_chai4
            (#match? @_chai4 "^expect$")))))
    property: (property_identifier) @_term4
    (#match? @_term4 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled)$"))) @assertion

;; Depth 5: expect(x).a.b.c.d.TERMINAL (e.g., .to.not.have.been.calledOnce)
(expression_statement
  (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (member_expression
            object: (call_expression
              function: (identifier) @_chai5
              (#match? @_chai5 "^expect$"))))))
    property: (property_identifier) @_term5
    (#match? @_term5 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled)$"))) @assertion

;; Match assert.* (Node assert module)
(call_expression
  function: (member_expression
    object: (identifier) @obj
    (#match? @obj "^assert$"))) @assertion
