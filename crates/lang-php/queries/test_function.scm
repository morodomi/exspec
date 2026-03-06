; PHPUnit: public function test_* (snake_case) or testFoo (camelCase)
(method_declaration
  name: (name) @name
  (#match? @name "^test")) @function

; PHPUnit: #[Test] attribute
(method_declaration
  attributes: (attribute_list
    (attribute_group
      (attribute
        (name) @_attr
        (#eq? @_attr "Test"))))
  name: (name) @name) @function

; Pest: test('name', fn) / it('name', fn)
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

; Pest: test('name', fn)->with(...) chained
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
