; $mockXxx = $this->createMock(...)
(assignment_expression
  left: (variable_name (name) @var_name)
  right: (member_call_expression
    object: (variable_name (name) @_obj)
    name: (name) @_method
    (#eq? @_obj "this")
    (#eq? @_method "createMock"))) @mock_assignment

; $mockXxx = Mockery::mock(...)
(assignment_expression
  left: (variable_name (name) @var_name)
  right: (scoped_call_expression
    scope: (name) @_scope
    name: (name) @_method
    (#eq? @_scope "Mockery")
    (#eq? @_method "mock"))) @mock_assignment
