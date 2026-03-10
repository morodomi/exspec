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
;;   calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled
;; tree-sitter has no recursive matching, so depths 1-5 are enumerated explicitly.

;; Depth 1: expect(x).TERMINAL
(expression_statement
  (member_expression
    object: (call_expression
      function: (identifier) @_chai1
      (#match? @_chai1 "^expect$"))
    property: (property_identifier) @_term1
    (#match? @_term1 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Depth 2: expect(x).chain.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_chai2
        (#match? @_chai2 "^expect$")))
    property: (property_identifier) @_term2
    (#match? @_term2 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Depth 3: expect(x).a.b.TERMINAL
(expression_statement
  (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_chai3
          (#match? @_chai3 "^expect$"))))
    property: (property_identifier) @_term3
    (#match? @_term3 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

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
    (#match? @_term4 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

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
    (#match? @_term5 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Chai BDD property-style assertions in arrow function concise body (#49).
;; In concise arrow body (=> expr), there is no expression_statement wrapper.
;; arrow_function body: and expression_statement are structurally exclusive,
;; so no double-count risk.

;; Arrow depth 1: => expect(x).TERMINAL
(arrow_function
  body: (member_expression
    object: (call_expression
      function: (identifier) @_chai_a1
      (#match? @_chai_a1 "^expect$"))
    property: (property_identifier) @_term_a1
    (#match? @_term_a1 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Arrow depth 2: => expect(x).chain.TERMINAL
(arrow_function
  body: (member_expression
    object: (member_expression
      object: (call_expression
        function: (identifier) @_chai_a2
        (#match? @_chai_a2 "^expect$")))
    property: (property_identifier) @_term_a2
    (#match? @_term_a2 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Arrow depth 3: => expect(x).a.b.TERMINAL
(arrow_function
  body: (member_expression
    object: (member_expression
      object: (member_expression
        object: (call_expression
          function: (identifier) @_chai_a3
          (#match? @_chai_a3 "^expect$"))))
    property: (property_identifier) @_term_a3
    (#match? @_term_a3 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Arrow depth 4: => expect(x).a.b.c.TERMINAL
(arrow_function
  body: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (call_expression
            function: (identifier) @_chai_a4
            (#match? @_chai_a4 "^expect$")))))
    property: (property_identifier) @_term_a4
    (#match? @_term_a4 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

;; Arrow depth 5: => expect(x).a.b.c.d.TERMINAL
(arrow_function
  body: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (member_expression
            object: (call_expression
              function: (identifier) @_chai_a5
              (#match? @_chai_a5 "^expect$"))))))
    property: (property_identifier) @_term_a5
    (#match? @_term_a5 "^(ok|true|false|null|undefined|exist|exists|empty|NaN|extensible|sealed|frozen|arguments|Arguments|finite|calledOnce|calledTwice|calledThrice|called|notCalled|returned|rejected|fulfilled|throw)$"))) @assertion

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
      (#match? @_chai_mc2_chain "^(to|be|been|have|a|an|deep|nested|own|ordered|any|all|itself|eventually|and)$"))
    property: (property_identifier) @_chai_mc2_term
    (#match? @_chai_mc2_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

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
        (#match? @_chai_mc3_chain1 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
      property: (property_identifier) @_chai_mc3_chain2
      (#match? @_chai_mc3_chain2 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
    property: (property_identifier) @_chai_mc3_term
    (#match? @_chai_mc3_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

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
          (#match? @_chai_mc4_chain1 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
        property: (property_identifier) @_chai_mc4_chain2
        (#match? @_chai_mc4_chain2 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
      property: (property_identifier) @_chai_mc4_chain3
      (#match? @_chai_mc4_chain3 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
    property: (property_identifier) @_chai_mc4_term
    (#match? @_chai_mc4_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

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
            (#match? @_chai_mc5_chain1 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
          property: (property_identifier) @_chai_mc5_chain2
          (#match? @_chai_mc5_chain2 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
        property: (property_identifier) @_chai_mc5_chain3
        (#match? @_chai_mc5_chain3 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
      property: (property_identifier) @_chai_mc5_chain4
      (#match? @_chai_mc5_chain4 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
    property: (property_identifier) @_chai_mc5_term
    (#match? @_chai_mc5_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

;; Chai depth-6: expect(x).C1.C2.C3.C4.C5.METHOD(args)
;; e.g., expect(x).to.be.rejected.and.be.instanceof(Error)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (member_expression
            object: (member_expression
              object: (call_expression
                function: (identifier) @_chai_mc6
                (#match? @_chai_mc6 "^expect$"))
              property: (property_identifier) @_chai_mc6_chain1
              (#match? @_chai_mc6_chain1 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
            property: (property_identifier) @_chai_mc6_chain2
            (#match? @_chai_mc6_chain2 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
          property: (property_identifier) @_chai_mc6_chain3
          (#match? @_chai_mc6_chain3 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
        property: (property_identifier) @_chai_mc6_chain4
        (#match? @_chai_mc6_chain4 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
      property: (property_identifier) @_chai_mc6_chain5
      (#match? @_chai_mc6_chain5 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
    property: (property_identifier) @_chai_mc6_term
    (#match? @_chai_mc6_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

;; Chai depth-7: expect(x).C1.C2.C3.C4.C5.C6.METHOD(args)
;; e.g., expect(p).to.be.rejected.and.be.an.instanceof(Error)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (member_expression
        object: (member_expression
          object: (member_expression
            object: (member_expression
              object: (member_expression
                object: (call_expression
                  function: (identifier) @_chai_mc7
                  (#match? @_chai_mc7 "^expect$"))
                property: (property_identifier) @_chai_mc7_chain1
                (#match? @_chai_mc7_chain1 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
              property: (property_identifier) @_chai_mc7_chain2
              (#match? @_chai_mc7_chain2 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
            property: (property_identifier) @_chai_mc7_chain3
            (#match? @_chai_mc7_chain3 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
          property: (property_identifier) @_chai_mc7_chain4
          (#match? @_chai_mc7_chain4 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
        property: (property_identifier) @_chai_mc7_chain5
        (#match? @_chai_mc7_chain5 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
      property: (property_identifier) @_chai_mc7_chain6
      (#match? @_chai_mc7_chain6 "^(to|be|been|have|not|a|an|deep|nested|own|ordered|any|all|itself|eventually|and|rejected|fulfilled)$"))
    property: (property_identifier) @_chai_mc7_term
    (#match? @_chai_mc7_term "^(equal|equals|eql|eq|a|an|include|contain|contains|throw|throws|match|property|keys|lengthOf|length|members|satisfy|closeTo|above|below|least|most|within|instanceOf|instanceof|respondTo|oneOf|change|increase|decrease|by|string|ownProperty|calledWith|calledOnceWith|calledWithExactly|calledOn|callCount|returned|thrown|rejectedWith)$"))) @assertion

;; Supertest-style .expect() method-call chained on another call_expression.
;; e.g., request(app).get('/').expect(200), app.inject({...}).expect(201)
;; Broad by design: also matches someBuilder().expect('foo').
;; Risk direction: false negative (misses assertion-free), not false positive.
;; No double-count with depth-1 expect pattern: depth-1 requires
;; `function: (identifier)` as the call root, while this pattern requires
;; `object: (call_expression)` — structurally disjoint.
(call_expression
  function: (member_expression
    object: (call_expression)
    property: (property_identifier) @_supertest_prop
    (#eq? @_supertest_prop "expect"))) @assertion

;; Match assert.* (Node assert module)
(call_expression
  function: (member_expression
    object: (identifier) @obj
    (#match? @obj "^assert$"))) @assertion

;; sinon.assert.X() / Sinon.assert.X() — depth-2 (#48)
;; e.g., sinon.assert.calledOnce(spy), sinon.assert.callOrder(spy1, spy2)
(call_expression
  function: (member_expression
    object: (member_expression
      object: (identifier) @_sinon_obj
      (#match? @_sinon_obj "^[Ss]inon$")
      property: (property_identifier) @_sinon_assert
      (#eq? @_sinon_assert "assert")))) @assertion

;; <expr>.verify() — Sinon mock expectation verification (#51)
;; e.g., expectation.verify(), mock.verify(), sinon.mock(obj).expects('m').once().verify()
;; .verify() throws if expectation not met, making it a legitimate test oracle.
;; Broad match (Option A): any <expr>.verify() counts as assertion.
;; Risk direction: false negative (misses assertion-free), not false positive.
(call_expression
  function: (member_expression
    property: (property_identifier) @_verify_prop
    (#eq? @_verify_prop "verify"))) @assertion
