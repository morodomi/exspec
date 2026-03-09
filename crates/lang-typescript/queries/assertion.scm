;; Match expect(...).toBe(...) and similar
(call_expression
  function: (member_expression
    object: (call_expression
      function: (identifier) @fn
      (#match? @fn "^expect$")))) @assertion

;; Match expect(...).not/resolves/rejects.toX(...) — modifier chain (depth-2)
;; The outer call_expression constrains the called member to .toX(...).
;; (rejects also matched in error_test.scm for T103)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_fn2
        (#match? @_fn2 "^expect$"))
      property: (property_identifier) @_prop
      (#match? @_prop "^(not|resolves|rejects)$")))) @assertion

;; Match expect(...).modifier.modifier.toX(...) — two-modifier chain (depth-3)
;; e.g., expect(p).resolves.not.toThrow(), expect(p).rejects.not.toBe(...)
;; For simplicity, accepts any combination of not|resolves|rejects modifiers.
;; Terminal matcher constraint (^to[A-Z]) follows Jest/Vitest naming convention.
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_fn_d3
          (#match? @_fn_d3 "^expect$"))
        property: (property_identifier) @_prop_d3a
        (#match? @_prop_d3a "^(not|resolves|rejects)$"))
      property: (property_identifier) @_prop_d3b
      (#match? @_prop_d3b "^(not|resolves|rejects)$"))
    property: (property_identifier) @_matcher_d3
    (#match? @_matcher_d3 "^to[A-Z]"))) @assertion

;; Match expectTypeOf(...).toEqualTypeOf<T>() and similar (chained method call)
(call_expression
  function: (member_expression
    object: (call_expression
      function: (identifier) @_fn3
      (#eq? @_fn3 "expectTypeOf")))) @assertion

;; Match standalone expectType<T>(value) without chained method call
(call_expression
  function: (identifier) @_fn_et
  (#eq? @_fn_et "expectType")) @assertion

;; Match expect.assertions(N), expect.hasAssertions(), expect.unreachable()
(call_expression
  function: (member_expression
    object: (identifier) @_fn_assert
    property: (property_identifier) @_prop_assert
    (#eq? @_fn_assert "expect")
    (#match? @_prop_assert "^(assertions|hasAssertions|unreachable)$"))) @assertion

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
