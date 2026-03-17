;; 1. Top-level function definitions
(function_definition
  name: (identifier) @name) @function

;; 2. Class methods (function_definition inside a class body)
(class_definition
  name: (identifier) @class_name
  body: (block
    (function_definition
      name: (identifier) @method_name) @method))

;; 3. Decorated functions at top level
(decorated_definition
  (function_definition
    name: (identifier) @decorated_name)) @decorated_function

;; 4. Decorated methods inside classes
(class_definition
  name: (identifier) @decorated_class_name
  body: (block
    (decorated_definition
      (function_definition
        name: (identifier) @decorated_method_name)) @decorated_method))
