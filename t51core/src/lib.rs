#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]

pub mod alloc;
pub mod sentinel;
pub mod component;
pub mod entity;
pub mod identity;
pub mod registry;
pub mod sync;

#[macro_use]
pub mod system;

pub mod world;
pub mod prelude;
