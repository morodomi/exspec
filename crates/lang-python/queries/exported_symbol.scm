;; Extract symbols from __all__ = ["Foo", "Bar"]
;;
;; Handles:
;;   __all__ = ["Foo"]
;;   __all__ = ("Foo",)
;;   __all__ = []  (empty — pattern 2)
;;   __all__ = ()  (empty — pattern 2)

;; Pattern 1: __all__ with one or more symbols
(assignment
  left: (identifier) @var_name
  (#eq? @var_name "__all__")
  right: [
    (list
      (string) @symbol)
    (tuple
      (string) @symbol)
  ])

;; Pattern 2: __all__ declaration (matches empty and non-empty, signals existence)
(assignment
  left: (identifier) @all_decl
  (#eq? @all_decl "__all__")
  right: [(list) (tuple)])
