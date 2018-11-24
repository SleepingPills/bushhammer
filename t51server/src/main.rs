#![allow(unused_imports, dead_code, unused_variables)]
#[macro_use]
extern crate t51core;
extern crate t51core_proc;

use t51core::identity2::{ComponentId, ComponentTypeIdentity};
use t51core::component::Component;
use t51core_proc::Component;
use serde_derive::{Deserialize, Serialize};

#[derive(Component, Deserialize, Debug)]
struct Velocity;

fn main() {
    Velocity::acquire_unique_id();

    println!("*** YO YO YO {:?} ***", Velocity::get_type_name());
}
