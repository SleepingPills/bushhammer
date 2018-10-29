#![allow(unused_imports, dead_code)]
#[macro_use]
extern crate t51core;
extern crate indexmap;
extern crate t51core_proc;

use t51core::system::SystemData;
use t51core::entity::EntityId;
use t51core_proc::{make_system};


#[make_system]
struct MySys<'a> {
    data: SystemData<(EntityId, &'a i32, &'a u64, &'a mut u64)>,
    plod: i32,
    glod: &'a str,
}

fn test(sys: &MySys) {
    sys.data.get_ctx();
}

fn main() {

}
