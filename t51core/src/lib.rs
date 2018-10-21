#![feature(unsize)]
#![feature(integer_atomics)]

extern crate anymap;
extern crate indexmap;


pub mod alloc;
pub mod sync;
pub mod registry;
pub mod component;
pub mod system;
pub mod world;
pub mod prelude;
