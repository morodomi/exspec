; PHPUnit: public function test_* (snake_case) or testFoo (camelCase)
(method_declaration
  name: (name) @name
  (#match? @name "^test")) @function

; PHPUnit: #[Test] attribute (short name)
(method_declaration
  attributes: (attribute_list
    (attribute_group
      (attribute
        (name) @_attr
        (#eq? @_attr "Test"))))
  name: (name) @name) @function

; PHPUnit: #[\PHPUnit\Framework\Attributes\Test] (fully qualified)
(method_declaration
  attributes: (attribute_list
    (attribute_group
      (attribute
        (qualified_name) @_qattr
        (#match? @_qattr "Test$"))))
  name: (name) @name) @function

; Pest: test('name', fn() { ... }) anonymous function
(expression_statement
  (function_call_expression
    function: (name) @_fn
    arguments: (arguments
      (argument
        (string
          (string_content) @name))
      (argument
        (anonymous_function)))
    (#match? @_fn "^(test|it)$"))) @function

; Pest: test('name', fn() => expr) arrow function
(expression_statement
  (function_call_expression
    function: (name) @_fn
    arguments: (arguments
      (argument
        (string
          (string_content) @name))
      (argument
        (arrow_function)))
    (#match? @_fn "^(test|it)$"))) @function

; Pest: test('name', fn() { ... })->with(...) anonymous chained
(expression_statement
  (member_call_expression
    object: (function_call_expression
      function: (name) @_fn
      arguments: (arguments
        (argument
          (string
            (string_content) @name))
        (argument
          (anonymous_function)))
      (#match? @_fn "^(test|it)$")))) @function

; Pest: test('name', fn() => expr)->with(...) arrow chained
(expression_statement
  (member_call_expression
    object: (function_call_expression
      function: (name) @_fn
      arguments: (arguments
        (argument
          (string
            (string_content) @name))
        (argument
          (arrow_function)))
      (#match? @_fn "^(test|it)$")))) @function
