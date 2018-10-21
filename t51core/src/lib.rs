#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(cfg_target_has_atomic)]

extern crate anymap;
extern crate indexmap;


pub mod alloc;
pub mod sync;
pub mod registry;
pub mod component;
pub mod system;
pub mod world;
pub mod prelude;
