use crate::config::Server;
use flux::logging;
use neutronium::net::endpoint::Endpoint;
use neutronium::prelude::{Context, Router, RunSystem, TransactionContext};

pub struct Replicator {
    endpoint: Endpoint,
    log: logging::Logger,
}

impl Replicator {
    pub fn new(config: &Server, log: &logging::Logger) -> Replicator {
        Replicator {
            endpoint: Endpoint::new(&config.address, config.token.clone(), &log)
                .expect("Failed creating endpoint"),
            log: log.new(logging::o!())
        }
    }
}

impl RunSystem for Replicator {
    type Data = ();

    fn run(&mut self, ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
        logging::trace!(self.log, "running Replicator system", "context" => "run");
        /*
        TODO: Extend system with delta time measurement

        Endpoint
        1. Push
        2. Pull
        3. Sync
        */
        self.endpoint.sync(ctx.timestamp);
    }
}
