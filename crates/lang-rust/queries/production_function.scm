;; 1. Top-level function definitions (pub and non-pub)
(function_item
  name: (identifier) @name) @function

;; 2. Methods inside impl blocks
(impl_item
  type: (_) @class_name
  body: (declaration_list
    (function_item
      name: (identifier) @method_name) @method))
