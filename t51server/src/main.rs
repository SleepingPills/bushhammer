#![allow(unused_imports, dead_code, unused_variables)]
use byteorder::{ReadBytesExt, BigEndian};
use std::io::{Read, Write};
use t51core::prelude::*;

fn main() {
    let data = vec![1u8, 10u8, 3u8];
    let mut reader = &data[..];
    reader.read_u64::<BigEndian>().unwrap();
}
