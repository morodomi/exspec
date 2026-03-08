; Detect error/exception testing patterns in Python

; pytest.raises(...) — also in assertion.scm for T001
(call
  function: (attribute
    object: (identifier) @_obj
    attribute: (identifier) @_attr)
  (#eq? @_obj "pytest")
  (#eq? @_attr "raises")) @error_test

; pytest.warns(...) — also in assertion.scm for T001
(call
  function: (attribute
    object: (identifier) @_obj_w
    attribute: (identifier) @_attr_w)
  (#eq? @_obj_w "pytest")
  (#eq? @_attr_w "warns")) @error_test

; self.assertRaises(...)
(call
  function: (attribute
    object: (identifier) @_obj2
    attribute: (identifier) @_method)
  (#eq? @_obj2 "self")
  (#eq? @_method "assertRaises")) @error_test

; self.assertRaisesRegex(...)
(call
  function: (attribute
    object: (identifier) @_obj3
    attribute: (identifier) @_method2)
  (#eq? @_obj3 "self")
  (#eq? @_method2 "assertRaisesRegex")) @error_test

; self.assertWarns(...)
(call
  function: (attribute
    object: (identifier) @_obj4
    attribute: (identifier) @_method3)
  (#eq? @_obj4 "self")
  (#eq? @_method3 "assertWarns")) @error_test

; self.assertWarnsRegex(...)
(call
  function: (attribute
    object: (identifier) @_obj5
    attribute: (identifier) @_method4)
  (#eq? @_obj5 "self")
  (#eq? @_method4 "assertWarnsRegex")) @error_test
