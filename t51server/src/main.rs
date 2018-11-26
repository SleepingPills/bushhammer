#![allow(unused_imports, dead_code, unused_variables)]
#[macro_use]
extern crate t51core;
extern crate t51core_proc;

use t51core::identity2::ComponentId;
use t51core::component2::Component;
use t51core_proc::Component;
use serde_derive::{Deserialize, Serialize};

#[derive(Component, Deserialize, Debug)]
struct Velocity;

fn moof(a: Option<i32>, b: Option<i32>) -> Option<i32> {
    let ping = match a {
        Some(val) => val,
        _ => return None,
    };

    let pong = match b {
        Some(val) => val,
        _ => return None,
    };

    Some(ping + pong)
}

fn main() {
    Velocity::acquire_unique_id();

    println!("*** YO YO YO {:?} ***", Velocity::get_type_name());
}
