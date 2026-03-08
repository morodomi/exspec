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
