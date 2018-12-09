#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]
#![feature(const_vec_new)]
#![feature(box_into_raw_non_null)]

pub mod alloc;
pub mod messagebus;
pub mod component;
pub mod entity;
pub mod identity;
pub mod registry;
pub mod sentinel;
pub mod sync;

pub mod system;

pub mod prelude;
pub mod world;
