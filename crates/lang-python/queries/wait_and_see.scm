; Detect sleep/delay calls in Python test functions

; time.sleep(...)
(call
  function: (attribute
    object: (identifier) @_obj
    attribute: (identifier) @_attr)
  (#eq? @_obj "time")
  (#eq? @_attr "sleep")) @wait

; asyncio.sleep(...)
(call
  function: (attribute
    object: (identifier) @_obj2
    attribute: (identifier) @_attr2)
  (#eq? @_obj2 "asyncio")
  (#eq? @_attr2 "sleep")) @wait
