use crate::config::GameConfig;
use crate::replicator::Replicator;
use flux::logging;
use neutronium::prelude::World;

pub fn init_world(world: &mut World, config: &GameConfig, log: &logging::Logger) {
    init_replicator(world, config, log);
    world.build();
}

fn init_replicator(world: &mut World, config: &GameConfig, log: &logging::Logger) {
    logging::info!(log, "creating *** Replicator *** ");

    let replicator = Replicator::new(&config.server, log);

    world.register_system(replicator);
}
