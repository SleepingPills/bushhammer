#![allow(unused_imports, dead_code, unused_variables)]

use serde_derive::{Deserialize, Serialize};
use t51core::prelude::*;
use t51core_proc::Component;

trait PtrGetter {
    fn get_ptr(&self) -> *const ();
}

impl<T> PtrGetter for Vec<T> {
    fn get_ptr(&self) -> *const () {
        self as *const Vec<T> as *const ()
    }
}

fn main() {
    let stackalloc = Vec::<i32>::new();
    println!("{:p}", &stackalloc);
    let vector = Box::new(stackalloc);
    println!("{:p}", vector.get_ptr());
    let boxed: Box<PtrGetter> = vector;
    println!("{:p}", boxed.get_ptr() as *const Vec<i32>);
}
