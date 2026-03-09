; Any ->assert*() method call (covers $this, $response, chained calls, etc.)
; In PHP test files, object methods beginning with assert... are treated as assertion oracles by convention.
; ^assert([A-Z_]|$) ensures: assertStatus YES, assertEquals YES, assert() bare YES, but assertionHelper NO.
(member_call_expression
  name: (name) @_method
  (#match? @_method "^assert([A-Z_]|$)")) @assertion

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

; PHPUnit static: self::assert*() / static::assert*() / parent::assert*()
; In tree-sitter-php, self/static/parent are parsed as relative_scope (not name).
; parent::assert*() is intentionally included: valid oracle in PHPUnit inheritance chains.
(scoped_call_expression
  scope: (relative_scope) @_scope
  name: (name) @_smethod
  (#match? @_smethod "^assert([A-Z_]|$)")) @assertion

; Named-class static: Assert::assertEquals(), SomeAssert::assertTrue(), etc.
; Matches scoped_call_expression where scope is a simple name ending with "Assert" (or exact "Assert").
(scoped_call_expression
  scope: (name) @_cls
  name: (name) @_sm
  (#match? @_cls "Assert$")
  (#match? @_sm "^assert([A-Z_]|$)")) @assertion

; FQCN static: PHPUnit\Framework\Assert::assertSame(), etc.
; In tree-sitter-php, fully qualified class names use qualified_name node.
(scoped_call_expression
  scope: (qualified_name) @_qcls
  name: (name) @_qm
  (#match? @_qcls "Assert$")
  (#match? @_qm "^assert([A-Z_]|$)")) @assertion

; Mockery: ->shouldReceive(...) — sets mock expectation (verified at teardown)
(member_call_expression
  name: (name) @_m1 (#eq? @_m1 "shouldReceive")) @assertion

; Mockery: ->shouldHaveReceived(...) — post-execution mock verification
(member_call_expression
  name: (name) @_m2 (#eq? @_m2 "shouldHaveReceived")) @assertion

; Mockery: ->shouldNotHaveReceived(...) — negative mock verification
(member_call_expression
  name: (name) @_m3 (#eq? @_m3 "shouldNotHaveReceived")) @assertion

; PHPUnit mock: $this->expects(...) — mock expectation (constrained to $this only)
; $mock->expects() and other non-$this calls are NOT counted as assertions,
; because ->expects() is a common method name that may not be a test oracle.
(member_call_expression
  object: (variable_name (name) @_exp_this)
  name: (name) @_e
  (#eq? @_exp_this "this")
  (#eq? @_e "expects")) @assertion

; Pest: expect(...)->toBe(...) and similar
(member_call_expression
  object: (function_call_expression
    function: (name) @_fn
    (#eq? @_fn "expect"))
  name: (name) @_method
  (#match? @_method "^to")) @assertion

; Laravel Artisan console testing: expects* expectation methods
(member_call_expression
  name: (name) @_artisan
  (#match? @_artisan "^expects(Output|OutputToContain|NoOutput|Question|Choice|Confirmation|Search|DatabaseQueryCount|Table)$")) @assertion

; PHPUnit: expectNotToPerformAssertions() — marks test as intentionally assertion-free
(member_call_expression
  name: (name) @_npa
  (#eq? @_npa "expectNotToPerformAssertions")) @assertion

; PHPUnit: expectOutputString() — output assertion
(member_call_expression
  name: (name) @_eos
  (#eq? @_eos "expectOutputString")) @assertion
