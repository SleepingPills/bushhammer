#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]
#![feature(no_more_cas)]

pub mod alloc;
pub mod sentinel;
pub mod component;
pub mod entity;
pub mod entity2;
pub mod identity;
pub mod registry;
pub mod sync;

#[macro_use]
pub mod system;

pub mod world;
pub mod prelude;
