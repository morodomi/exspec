;; pub mod module_name;
(mod_item
  name: (identifier) @mod_name) @pub_mod

;; pub use module::*;
;; pub use module::{Symbol1, Symbol2};
(use_declaration
  argument: (_) @use_arg) @pub_use
