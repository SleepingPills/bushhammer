use crate::replicator::Replicator;
use flux::logging;
use neutronium::prelude::World;

pub fn init_world(world: &mut World, log: &logging::Logger) {
    init_replicator(world, log);
}

fn init_replicator(world: &mut World, log: &logging::Logger) {
    logging::info!(log, "initializing *** Replicator *** ")
}
