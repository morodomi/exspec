; Detect error/exception testing patterns in Rust

; #[should_panic] attribute
(attribute_item
  (attribute
    (identifier) @_attr
    (#eq? @_attr "should_panic"))) @error_test

; .unwrap_err() call
(call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  (#eq? @_method "unwrap_err")) @error_test

; Note: .is_err() removed — it's a weak proxy. Inside assert!() it becomes
; token_tree (not detectable), and standalone .is_err() without assertion
; is not a real error test. See #22.
