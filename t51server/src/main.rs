#![allow(unused_imports, dead_code, unused_variables)]
use t51core::system::SystemData;
use t51core::entity::EntityId;
use t51core_proc::{make_system};

#[make_system]
struct MySys<'a> {
    data: SystemData<(EntityId, &'a i32, &'a u64, &'a mut u64)>,
    plod: i32,
    glod: &'a str,
}

fn test(sys: &mut MySys) {
    let ctx = sys.data.get_ctx();
    for (id, a, b, c) in ctx.iter() {
        sys.plod = 4;
    };
}

fn main() {

}
