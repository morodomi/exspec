;; Detect #[cfg(test)] attribute (simple and compound forms)
;; In tree-sitter-rust, #[cfg(test)] is an attribute_item sibling of the mod_item.
;; We match the attribute_item and navigate to the next sibling in code.

;; Pattern 1: #[cfg(test)] — simple form
(attribute_item
  (attribute
    (identifier) @attr_name
    arguments: (token_tree
      (identifier) @cfg_arg))) @cfg_test_attr

;; Pattern 2: #[cfg(all(test, ...))] or #[cfg(any(test, ...))] — compound form
;; Matches when `test` identifier is inside a nested token_tree (one level deep)
(attribute_item
  (attribute
    (identifier) @attr_name
    arguments: (token_tree
      (token_tree
        (identifier) @cfg_arg)))) @cfg_test_attr
