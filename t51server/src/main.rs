#![allow(unused_imports, dead_code)]
#[macro_use]
extern crate t51core;
extern crate indexmap;
extern crate t51core_proc;

use t51core::system::SystemData;
use t51core_proc::{make_system, make_system2};
use t51core::entity::EntityId;


#[make_system2]
struct MySys<'a> {
    data: SystemData<(EntityId, &'a i32, &'a u64, &'a mut u64)>,
    plod: i32,
    glod: &'a str,
}

/*
fn test(sys: &MySys) {
    for (a, b, c) in sys.data.get_ctx() {

    }
}
*/

fn test(sys: &MySys) {
    sys.data.get_ctx();
}

fn main() {

}
