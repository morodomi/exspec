; PHPUnit: #[DataProvider('provider')]
(method_declaration
  attributes: (attribute_list
    (attribute_group
      (attribute
        (name) @_attr
        (#eq? @_attr "DataProvider"))))) @parameterized

; Pest: test(...)->with(...)
(expression_statement
  (member_call_expression
    object: (function_call_expression
      function: (name) @_fn
      (#match? @_fn "^(test|it)$"))
    name: (name) @_method
    (#eq? @_method "with"))) @parameterized
