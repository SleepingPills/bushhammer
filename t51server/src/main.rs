#![allow(unused_imports, dead_code, unused_variables)]
use std::collections::HashMap;
use t51core::object::EntityId;
use t51core::prelude::*;

pub struct Goof<'a> {
    coll: Vec<&'a i32>,
}

impl<'a> System for Goof<'a> {
    type Data = (Read<'a, EntityId>, Read<'a, i32>, Write<'a, u64>);

    fn run(&mut self, data: &SystemData<Self::Data>, entities: EntityStore) {
        let ctx = data.context();

        for (a, b, c) in ctx {
            *c = 5;
        }
    }
}

fn main() {}
