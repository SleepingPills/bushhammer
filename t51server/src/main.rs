#![allow(unused_imports, dead_code, unused_variables)]

use serde_derive::{Deserialize, Serialize};
use t51core::prelude::*;
use t51core_proc::Component;

#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C1(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C2(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C3(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C4(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C5(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C6(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C7(u32);
#[derive(Component, Serialize, Deserialize, Debug, Clone)]
pub struct C8(u32);

fn main() {
    // Create World
    let mut world = World::default();

    // Register Components
    world.register_component::<C1>();
    world.register_component::<C2>();
    world.register_component::<C3>();
    world.register_component::<C4>();
    world.register_component::<C5>();
    world.register_component::<C6>();
    world.register_component::<C7>();
    world.register_component::<C8>();

    // Build the world
    world.build();

    // Run
    let bit_set = C1::get_unique_id()
        + C2::get_unique_id()
        + C3::get_unique_id()
        + C4::get_unique_id()
        + C5::get_unique_id()
        + C7::get_unique_id()
        + C8::get_unique_id();

    let mut accum = 0;

    for id in bit_set.decompose() {
        println!("{}", id);
        accum += 1;
    }

    assert_eq!(accum, 6);
}
