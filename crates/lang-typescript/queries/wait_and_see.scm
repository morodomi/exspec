; Detect sleep/delay calls in TypeScript test functions

; setTimeout(...)
(call_expression
  function: (identifier) @_fn
  (#eq? @_fn "setTimeout")) @wait

; sleep(...)
(call_expression
  function: (identifier) @_fn2
  (#eq? @_fn2 "sleep")) @wait
