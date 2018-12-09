#![allow(unused_imports, dead_code, unused_variables)]

use serde_derive::{Deserialize, Serialize};
use t51core::prelude::*;
use t51core_proc::Component;

fn main() {
    let mut world = World::default();
    world.build();
    world.run();
}
