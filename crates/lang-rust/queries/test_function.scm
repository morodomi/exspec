; Rust: #[test] attribute_item - captures the attribute_item node.
; The function_item is the next sibling; Rust impl navigates via next_sibling().
(attribute_item
  (attribute
    (identifier) @_attr
    (#eq? @_attr "test"))) @test_attr

; Rust: #[tokio::test], #[async_std::test] etc. (scoped_identifier form)
; name: (identifier) must equal "test"
(attribute_item
  (attribute
    (scoped_identifier
      name: (identifier) @_attr)
    (#eq? @_attr "test"))) @test_attr
