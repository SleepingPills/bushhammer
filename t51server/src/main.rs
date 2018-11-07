#![allow(unused_imports, dead_code, unused_variables)]
use t51core::object::EntityId;
use t51core::prelude::*;

pub struct Goof<'a> {
    coll: Vec<&'a i32>,
}

impl<'a> System for Goof<'a> {
    type Data = (Read<'a, EntityId>, Read<'a, i32>, Write<'a, u64>);

    fn run(&mut self, data: &SystemData<Self::Data>, entities: EntityStore) {
        let mut ctx = data.context();

        let (d, e, f) = ctx.get_entity(5).unwrap();

        for (a, b, c) in ctx {
            *c = 5;
        }
    }
}

fn main() {}
