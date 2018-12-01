#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]
#![feature(const_vec_new)]
#![feature(box_into_raw_non_null)]

pub mod alloc;
pub mod sentinel;
pub mod component;
pub mod component2;
pub mod component3;
pub mod entity;
pub mod entity2;
pub mod identity;
pub mod identity2;
pub mod registry;
pub mod sync;

#[macro_use]
pub mod system;
pub mod system2;
pub mod system3;

pub mod world;
pub mod world2;
pub mod prelude;
