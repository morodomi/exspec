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

;; expect.soft/element/poll depth-3: expect.soft(x).not.toBe(y)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression
        function: (member_expression
          object: (identifier) @_fn_sep3
          property: (property_identifier) @_prop_sep3
          (#eq? @_fn_sep3 "expect")
          (#match? @_prop_sep3 "^(soft|element|poll)$")))
      property: (property_identifier) @_mod_sep3
      (#match? @_mod_sep3 "^(not|resolves|rejects)$"))
    property: (property_identifier) @_term_sep3
    (#match? @_term_sep3 "^to[A-Z]"))) @assertion

;; expect.soft/element/poll depth-4: expect.soft(x).resolves.not.toBe(y)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (member_expression
            object: (identifier) @_fn_sep4
            property: (property_identifier) @_prop_sep4
            (#eq? @_fn_sep4 "expect")
            (#match? @_prop_sep4 "^(soft|element|poll)$")))
        property: (property_identifier) @_mod_sep4a
        (#match? @_mod_sep4a "^(not|resolves|rejects)$"))
      property: (property_identifier) @_mod_sep4b
      (#match? @_mod_sep4b "^(not|resolves|rejects)$"))
    property: (property_identifier) @_term_sep4
    (#match? @_term_sep4 "^to[A-Z]"))) @assertion

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
    (#match? @_term1 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned)$"))) @assertion

;; Depth 2: expect(x).chain.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_chai2
        (#match? @_chai2 "^expect$")))
    property: (property_identifier) @_term2
    (#match? @_term2 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned)$"))) @assertion

;; Depth 3: expect(x).a.b.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_chai3
          (#match? @_chai3 "^expect$"))))
    property: (property_identifier) @_term3
    (#match? @_term3 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned)$"))) @assertion

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
    (#match? @_term4 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned)$"))) @assertion

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
    (#match? @_term5 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned)$"))) @assertion

;; Chai BDD method-call chain assertions (with trailing parentheses).
;; These patterns are kept separate from the existing Jest/Vitest modifier-chain
;; patterns to avoid double-counting while covering common Chai/Sinon-Chai method
;; terminals found during dogfooding.
;; Terminal methods use a bounded, dogfooding-driven initial vocabulary.
;; Intermediate chain words: to|be|been|have|deep|nested|own|ordered|any|all|itself (depth-2),
;; +not (depth-3+).

;; Chai depth-2: expect(x).CHAIN.METHOD(args)
;; e.g., expect(x).to.equal(y), expect(obj).itself.respondTo('bar')
;; `not` is excluded at depth-2 to avoid overlap with the existing modifier-chain
;; pattern for expect(x).not.<method>().
(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_chai_mc2
        (#match? @_chai_mc2 "^expect$"))
      property: (property_identifier) @_chai_mc2_chain
      (#match? @_chai_mc2_chain "^(to|be|been|have|deep|nested|own|ordered|any|all|itself)$"))
    property: (property_identifier) @_chai_mc2_term
    (#match? @_chai_mc2_term "^(equal|eql|a|an|include|contain|throw|match|property|keys|lengthOf|members|satisfy|closeTo|above|below|least|most|within|instanceOf|respondTo|oneOf|change|increase|decrease|by|string|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown)$"))) @assertion

;; Chai depth-3: expect(x).CHAIN1.CHAIN2.METHOD(args)
;; e.g., expect(x).to.be.a('string'), expect(x).to.not.equal(y)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_chai_mc3
          (#match? @_chai_mc3 "^expect$"))
        property: (property_identifier) @_chai_mc3_chain1
        (#match? @_chai_mc3_chain1 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
      property: (property_identifier) @_chai_mc3_chain2
      (#match? @_chai_mc3_chain2 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
    property: (property_identifier) @_chai_mc3_term
    (#match? @_chai_mc3_term "^(equal|eql|a|an|include|contain|throw|match|property|keys|lengthOf|members|satisfy|closeTo|above|below|least|most|within|instanceOf|respondTo|oneOf|change|increase|decrease|by|string|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown)$"))) @assertion

;; Chai depth-4: expect(x).CHAIN1.CHAIN2.CHAIN3.METHOD(args)
;; e.g., expect(spy).to.have.been.calledWith(arg)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (call_expression
            function: (identifier) @_chai_mc4
            (#match? @_chai_mc4 "^expect$"))
          property: (property_identifier) @_chai_mc4_chain1
          (#match? @_chai_mc4_chain1 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
        property: (property_identifier) @_chai_mc4_chain2
        (#match? @_chai_mc4_chain2 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
      property: (property_identifier) @_chai_mc4_chain3
      (#match? @_chai_mc4_chain3 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
    property: (property_identifier) @_chai_mc4_term
    (#match? @_chai_mc4_term "^(equal|eql|a|an|include|contain|throw|match|property|keys|lengthOf|members|satisfy|closeTo|above|below|least|most|within|instanceOf|respondTo|oneOf|change|increase|decrease|by|string|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown)$"))) @assertion

;; Chai depth-5: expect(x).CHAIN1.CHAIN2.CHAIN3.CHAIN4.METHOD(args)
;; e.g., expect(spy).to.not.have.been.calledWith(arg)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (member_expression
            object: (call_expression
              function: (identifier) @_chai_mc5
              (#match? @_chai_mc5 "^expect$"))
            property: (property_identifier) @_chai_mc5_chain1
            (#match? @_chai_mc5_chain1 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
          property: (property_identifier) @_chai_mc5_chain2
          (#match? @_chai_mc5_chain2 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
        property: (property_identifier) @_chai_mc5_chain3
        (#match? @_chai_mc5_chain3 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
      property: (property_identifier) @_chai_mc5_chain4
      (#match? @_chai_mc5_chain4 "^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$"))
    property: (property_identifier) @_chai_mc5_term
    (#match? @_chai_mc5_term "^(equal|eql|a|an|include|contain|throw|match|property|keys|lengthOf|members|satisfy|closeTo|above|below|least|most|within|instanceOf|respondTo|oneOf|change|increase|decrease|by|string|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown)$"))) @assertion

;; Match assert.* (Node assert module)
(call_expression
  function: (member_expression
    object: (identifier) @obj
    (#match? @obj "^assert$"))) @assertion
