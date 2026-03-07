; Detect error/exception testing patterns in Python

; pytest.raises(...)
(call
  function: (attribute
    object: (identifier) @_obj
    attribute: (identifier) @_attr)
  (#eq? @_obj "pytest")
  (#eq? @_attr "raises")) @error_test

; self.assertRaises(...)
(call
  function: (attribute
    attribute: (identifier) @_method)
  (#eq? @_method "assertRaises")) @error_test

; self.assertRaisesRegex(...)
(call
  function: (attribute
    attribute: (identifier) @_method2)
  (#eq? @_method2 "assertRaisesRegex")) @error_test

; self.assertWarns(...)
(call
  function: (attribute
    attribute: (identifier) @_method3)
  (#eq? @_method3 "assertWarns")) @error_test

; self.assertWarnsRegex(...)
(call
  function: (attribute
    attribute: (identifier) @_method4)
  (#eq? @_method4 "assertWarnsRegex")) @error_test
