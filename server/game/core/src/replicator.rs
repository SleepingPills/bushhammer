use neutronium::net::endpoint::Endpoint;
use neutronium::prelude::{Context, Router, RunSystem, TransactionContext};

pub struct Replicator {
    endpoint: Endpoint,
}

impl RunSystem for Replicator {
    type Data = ();

    fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
        unimplemented!()
    }
}
