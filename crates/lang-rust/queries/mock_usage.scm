; mockall: MockXxx::new() - call_expression with ::new()
(call_expression
  function: (scoped_identifier
    path: (identifier) @_class
    name: (identifier) @_method
    (#match? @_class "^Mock")
    (#eq? @_method "new"))) @mock
