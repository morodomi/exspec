; assert statement
(assert_statement) @assertion

; unittest self.assert* methods
(call
  function: (attribute
    object: (identifier) @obj
    attribute: (identifier) @method)
  (#match? @obj "^self$")
  (#match? @method "^assert")) @assertion

; pytest.raises() — exception verification counts as assertion
; (also matched in error_test.scm for T103)
(call
  function: (attribute
    object: (identifier) @_pytest_obj
    attribute: (identifier) @_pytest_attr)
  (#eq? @_pytest_obj "pytest")
  (#eq? @_pytest_attr "raises")) @assertion

; unittest.mock: mock.assert_*() methods (assert_called_once, assert_not_called, etc.)
(call
  function: (attribute
    attribute: (identifier) @_mock_method)
  (#match? @_mock_method "^assert_")) @assertion

; pytest.warns() — warning verification counts as assertion
; (also matched in error_test.scm for T103)
(call
  function: (attribute
    object: (identifier) @_pytest_warns_obj
    attribute: (identifier) @_pytest_warns_attr)
  (#eq? @_pytest_warns_obj "pytest")
  (#eq? @_pytest_warns_attr "warns")) @assertion

; pytest.fail() — explicit failure oracle counts as assertion
; unconditionally fails the test with a message (functionally equivalent to `assert False, msg`)
(call
  function: (attribute
    object: (identifier) @_pytest_fail_obj
    attribute: (identifier) @_pytest_fail_attr)
  (#eq? @_pytest_fail_obj "pytest")
  (#eq? @_pytest_fail_attr "fail")) @assertion
