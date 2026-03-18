;; Exported symbols: pub items at top level (excluding pub use/pub mod,
;; which are handled by barrel resolution).

;; pub fn foo() {}
(function_item
  (visibility_modifier)
  name: (identifier) @symbol_name)

;; pub struct Foo {}
(struct_item
  (visibility_modifier)
  name: (type_identifier) @symbol_name)

;; pub enum Foo {}
(enum_item
  (visibility_modifier)
  name: (type_identifier) @symbol_name)

;; pub type Foo = ...;
(type_item
  (visibility_modifier)
  name: (type_identifier) @symbol_name)

;; pub const FOO: ... = ...;
(const_item
  (visibility_modifier)
  name: (identifier) @symbol_name)

;; pub static FOO: ... = ...;
(static_item
  (visibility_modifier)
  name: (identifier) @symbol_name)

;; pub trait Foo {}
(trait_item
  (visibility_modifier)
  name: (type_identifier) @symbol_name)
