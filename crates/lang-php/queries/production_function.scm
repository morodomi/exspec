;; 1. Top-level function definitions
(function_definition
  name: (name) @name) @function

;; 2. Class methods
(class_declaration
  name: (name) @class_name
  body: (declaration_list
    (method_declaration
      name: (name) @method_name) @method))

;; 3. Interface methods (implicitly public)
(interface_declaration
  name: (name) @class_name
  body: (declaration_list
    (method_declaration
      name: (name) @method_name) @method))

;; 4. Trait methods
(trait_declaration
  name: (name) @class_name
  body: (declaration_list
    (method_declaration
      name: (name) @method_name) @method))
