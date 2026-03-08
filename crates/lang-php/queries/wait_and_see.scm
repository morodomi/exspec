; Detect sleep/delay calls in PHP test functions

; sleep(...)
(function_call_expression
  function: (name) @_fn
  (#eq? @_fn "sleep")) @wait

; usleep(...)
(function_call_expression
  function: (name) @_fn2
  (#eq? @_fn2 "usleep")) @wait
