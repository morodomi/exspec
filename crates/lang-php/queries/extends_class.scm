;; Extract parent class name from class declarations.
;; class Foo extends Bar { ... }
;; => @parent_class = "Bar"
(class_declaration
  (base_clause
    (name) @parent_class))
