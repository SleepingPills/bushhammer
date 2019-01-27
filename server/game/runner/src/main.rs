use serde_derive::{Serialize, Deserialize};

use neutronium::prelude::{ComponentClass, Component, Topic, Message};
use neutronium::component_init;
use neutronium::topic_init;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Poof {

}

//component_init!(Poof);
topic_init!(Poof);


fn main() {
    let a = Poof::get_topic();

    unsafe {
        println!("{:?}", Topic::get_id_vec());
        println!("{:?}", Topic::get_name_vec());
    }

    println!("Hi! {:?}", a);
}
