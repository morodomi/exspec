;; Detect #[cfg(test)] attribute
;; In tree-sitter-rust, #[cfg(test)] is an attribute_item sibling of the mod_item.
;; We match the attribute_item and navigate to the next sibling in code.
(attribute_item
  (attribute
    (identifier) @attr_name
    arguments: (token_tree
      (identifier) @cfg_arg))) @cfg_test_attr
