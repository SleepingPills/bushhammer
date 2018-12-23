#![allow(unused_imports, dead_code, unused_variables)]

use serde_derive::{Deserialize, Serialize};
use t51core::prelude::*;

use std::io;

fn test(buf: &[u8]) {
    reader(buf);
}

fn reader<R: io::Read>(mut r: R) {
}

fn writer<W: io::Write>(mut w: W) {
}

fn main() {
}
