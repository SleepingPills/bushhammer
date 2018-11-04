#![allow(unused_imports, dead_code, unused_variables)]
use std::collections::HashMap;
use t51core::prelude::*;

pub struct Goof<'a> {
    coll: Vec<&'a i32>
}

impl<'a> System for Goof<'a> {
    type Data = (Read<'a, EntityId>, Read<'a, i32>, Write<'a, u64>);

    fn run(&mut self, ctx: Context<Self::Data>, entities: EntityStore) {
        for (a, b, c) in ctx.iter() {

        }
    }
}

fn main() {}

