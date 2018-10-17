#[macro_use]
extern crate t51core;
extern crate indexmap;


use t51core::prelude::*;
use indexmap::IndexMap;


/*fn test3(comp: (IndexMap<usize, (usize, usize)>, ReadStore<i32>, WriteStore<i32>)) {
    let ctx = comp.join();

    for (id, a, b) in ctx {
        *b = 5;
        let (c, d) = ctx.get(5);
    }
}*/


fn main() {
}
