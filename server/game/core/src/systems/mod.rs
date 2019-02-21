use crate::config::GameConfig;
use crate::replicator::Replicator;
use flux::logging;
use neutronium::prelude::World;

pub fn build_world(world: &mut World, config: &GameConfig, log: &logging::Logger) {
    build_replicator(world, config, log);
    world.build();
}

fn build_replicator(world: &mut World, config: &GameConfig, log: &logging::Logger) {
    logging::info!(log, "building *** Replicator *** ");

    let replicator = Replicator::new(&config.server, log);

    world.register_system(replicator);
}
