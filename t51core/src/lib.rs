#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]

pub mod alloc;
pub mod component;
pub mod entity;
pub mod object;
pub mod prelude;
pub mod registry;
pub mod sync;
pub mod system;
pub mod system2;
pub mod world;
