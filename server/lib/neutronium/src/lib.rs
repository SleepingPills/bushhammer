#![feature(nll)]
#![feature(unsize)]
#![feature(integer_atomics)]
#![feature(core_intrinsics)]
#![feature(const_vec_new)]
#![feature(const_raw_ptr_deref)]
#![feature(box_into_raw_non_null)]

#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]
#![allow(clippy::new_without_default_derive)]

pub mod prelude;

#[macro_use]
pub mod identity;
pub mod alloc;
pub mod messagebus;
pub mod component;
pub mod entity;
pub mod registry;
pub mod sentinel;
pub mod sync;

pub mod net;
pub mod system;
pub mod world;