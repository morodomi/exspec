; Detect sleep/delay calls in Rust test functions

; thread::sleep(...)
(call_expression
  function: (scoped_identifier
    path: (identifier) @_path
    name: (identifier) @_name)
  (#eq? @_path "thread")
  (#eq? @_name "sleep")) @wait

; std::thread::sleep(...)
(call_expression
  function: (scoped_identifier
    path: (scoped_identifier
      path: (identifier) @_std
      name: (identifier) @_mod)
    name: (identifier) @_fn)
  (#eq? @_std "std")
  (#eq? @_mod "thread")
  (#eq? @_fn "sleep")) @wait

; tokio::time::sleep(...)
(call_expression
  function: (scoped_identifier
    path: (scoped_identifier
      path: (identifier) @_tok
      name: (identifier) @_time)
    name: (identifier) @_fn2)
  (#eq? @_tok "tokio")
  (#eq? @_time "time")
  (#eq? @_fn2 "sleep")) @wait
