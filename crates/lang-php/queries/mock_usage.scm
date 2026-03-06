; $this->createMock(...)
(member_call_expression
  object: (variable_name (name) @_obj)
  name: (name) @_method
  (#eq? @_obj "this")
  (#eq? @_method "createMock")) @mock

; $this->getMockBuilder(...)
(member_call_expression
  object: (variable_name (name) @_obj)
  name: (name) @_method
  (#eq? @_obj "this")
  (#eq? @_method "getMockBuilder")) @mock

; Mockery::mock(...)
(scoped_call_expression
  scope: (name) @_scope
  name: (name) @_method
  (#eq? @_scope "Mockery")
  (#eq? @_method "mock")) @mock
