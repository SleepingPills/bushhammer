#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]
#![feature(const_vec_new)]

pub mod alloc;
pub mod sentinel;
pub mod component;
pub mod entity;
pub mod entity2;
pub mod identity;
pub mod identity2;
pub mod registry;
pub mod sync;

#[macro_use]
pub mod system;

pub mod world;
pub mod prelude;
