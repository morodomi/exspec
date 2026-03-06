; let mock_xxx = MockXxx::new();
(let_declaration
  pattern: (identifier) @var_name
  value: (call_expression
    function: (scoped_identifier
      path: (identifier) @_class
      name: (identifier) @_method
      (#match? @_class "^Mock")
      (#eq? @_method "new")))) @mock_assignment
