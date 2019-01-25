use chrono;
use serde_derive::{Serialize, Deserialize};

use neutronium::prelude::{ComponentId, Component};
use neutronium::component_init;

#[derive(Serialize, Deserialize, Debug)]
pub struct Poof {

}

component_init!(Poof);

fn main() {
    let a = Poof::get_unique_id();

    println!("Hi! {:?}", a);
}
