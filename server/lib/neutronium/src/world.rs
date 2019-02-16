use crate::component::Component;
use crate::component::{ComponentClassAux, ComponentCoords, Shard};
use crate::entity::{EntityId, ShardDef, TransactionContext};
use crate::identity::{ShardKey, SystemId};
use crate::messagebus::Bus;
use crate::registry::Registry;
use crate::system::{RunSystem, System, SystemRuntime};
use anymap::AnyMap;
use flux::logging;
use hashbrown::HashMap;
use std::intrinsics::type_name;
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT};
use std::sync::Arc;
use std::thread;
use std::time;

pub struct World {
    // Global Settings
    frame_delta_time: time::Duration,
    delta: f32,
    timestamp: time::Instant,

    // Game State
    entity_counter: Arc<AtomicUsize>,
    state: GameState,

    // Transactions
    system_transactions: Vec<TransactionContext>,
    transactions: TransactionContext,
    finalized: bool,

    // Messaging
    messages: Bus,

    // Logging
    log: logging::Logger,
}

impl Default for World {
    fn default() -> Self {
        World::new(20, None)
    }
}

impl World {
    /// Creates a `World` instance initialized with default parameters:
    /// FPS: 20
    #[inline]
    pub fn new<'a, L: Into<Option<&'a logging::Logger>>>(fps: u64, log: L) -> Self {
        let world_log = match log.into() {
            Some(log) => log.new(logging::o!()),
            _ => logging::Logger::root(logging::Discard, logging::o!()),
        };

        let counter = Arc::new(ATOMIC_USIZE_INIT);
        let frame_delta_time = time::Duration::from_millis(1000 / fps);

        let world = World {
            frame_delta_time,
            delta: Self::duration_to_delta(frame_delta_time),
            timestamp: time::Instant::now(),
            entity_counter: counter.clone(),
            state: GameState::new(),
            system_transactions: Vec::new(),
            transactions: TransactionContext::new(counter),
            finalized: false,
            messages: Bus::new(),
            log: world_log,
        };

        world
    }

    /// Builds and finalizes this world. After finalization, new components, resources and
    /// systems can no longer be added.
    pub fn build(&mut self) {
        self.finalized = true;
        logging::info!(self.log, "initializing world"; "context" => "build");

        for (id, mut system) in self.state.systems.iter_mut::<System>() {
            logging::info!(self.log, "initializing system";
                            "context" => "build",
                            "system" => %id);

            system.init(&self.state.resources);

            // Create a copy of the main transaction context for each system so they can be run in parallel
            self.system_transactions
                .push(TransactionContext::new(self.entity_counter.clone()));
        }

        logging::info!(self.log, "world initialization finished"; "context" => "build");
    }

    /// Process all transactions in the queue.
    #[inline]
    pub fn process_transactions(&mut self) {
        logging::trace!(self.log, "processing main transactions"; "context" => "process_transactions");
        self.state.process_context(&mut self.transactions);

        logging::trace!(self.log, "processing system transactions"; "context" => "process_transactions");
        for tx in self.system_transactions.iter_mut() {
            self.state.process_context(tx);
        }
        logging::debug!(self.log, "transaction processing finished"; "context" => "process_transactions");
    }

    /// Process messages
    #[inline]
    pub fn process_messages(&mut self) {
        logging::trace!(self.log, "processing messages"; "context" => "process_messages");
        self.messages.clear();

        for (id, mut system) in self.state.systems.iter_mut::<System>() {
            logging::trace!(self.log, "processing system messages";
                            "context" => "process_messages",
                            "system" => %id);
            system.transfer_messages(&mut self.messages);
        }
        logging::debug!(self.log, "message processing finished"; "context" => "process_messages");
    }

    /// Runs one game iteration
    #[inline]
    pub fn run_once(&mut self) -> bool {
        self.process_transactions();
        self.process_systems();
        self.process_messages();

        // Eventually, process stopping conditions from various triggers (local or via network).
        true
    }

    /// Runs the main game loop with frame rate limiting.
    #[inline]
    pub fn run(&mut self) {
        let mut proceed = true;

        let mut prev_timestamp = time::Instant::now() - self.frame_delta_time;

        while proceed {
            self.timestamp = time::Instant::now();
            self.delta = Self::duration_to_delta(self.timestamp - prev_timestamp);

            logging::trace!(self.log, "frame started";
                            "context" => "run",
                            "timestamp" => ?self.timestamp,
                            "delta" => ?self.delta);

            proceed = self.run_once();

            let elapsed = time::Instant::now().duration_since(self.timestamp);

            logging::trace!(self.log, "frame finished"; "context" => "run","elapsed" => ?elapsed);

            if elapsed < self.frame_delta_time {
                let timeout = self.frame_delta_time - elapsed;
                logging::trace!(self.log, "frame timeout triggered"; "context" => "run", "timeout" => ?timeout);
                thread::sleep(timeout);
            }

            prev_timestamp = self.timestamp;
        }
    }

    #[inline]
    pub fn entities(&mut self) -> &mut TransactionContext {
        if !self.finalized {
            panic!("World must be finalized before adding entities")
        }

        &mut self.transactions
    }

    #[inline]
    fn duration_to_delta(duration: time::Duration) -> f32 {
        duration.as_float_secs() as f32
    }
}

impl World {
    /// Create a new runtime using the supplied system. The runtime is wired up with
    /// all required subsystems and ready to execute.
    #[inline]
    pub fn create_runtime<T>(&self, system: T) -> SystemRuntime<T>
    where
        T: RunSystem,
    {
        SystemRuntime::new(system)
    }

    /// Register the supplied system with the world.
    pub fn register_system<T>(&mut self, system: T) -> SystemId
    where
        T: 'static + RunSystem,
    {
        if self.finalized {
            panic!("Can't add systems to finalized world")
        }

        let runtime = self.create_runtime(system);
        let id = SystemId::new::<T>(self.state.systems.len());

        logging::debug!(self.log, "registering system";
                        "context" => "register_system",
                        "id" => ?id);

        self.state.systems.register(id, runtime);
        self.state.systems.register_trait::<SystemRuntime<T>, System>(&id);
        id
    }

    /// Process all currently registered systems.
    #[inline]
    pub fn process_systems(&mut self) {
        logging::debug!(self.log, "executing systems"; "context" => "process_systems");

        for (id, mut system) in self.state.systems.iter_mut::<System>() {
            logging::debug!(self.log, "system running";
                            "context" => "process_systems",
                            "system" => %id);

            unsafe {
                system.run(
                    &self.state.entities,
                    self.get_system_transactions(id.indexer()),
                    &self.messages,
                    self.delta,
                    self.timestamp,
                );
            }
        }

        logging::debug!(self.log, "system execution finished"; "context" => "process_systems");
    }

    // TODO: Check the performance impact of drain/rebuild and switch if negligible
    /// Horribly unsafe function to get mutable references to multiple elements of the system
    /// transactions without having to drain and rebuild the vector all the time.
    #[inline]
    unsafe fn get_system_transactions<'a>(&self, idx: usize) -> &'a mut TransactionContext {
        let ptr = self.system_transactions.as_ptr() as *mut TransactionContext;
        &mut *ptr.add(idx)
    }
}

impl World {
    /// Register the supplied resource instance.
    pub fn register_resource<T>(&mut self, resource: T)
    where
        T: 'static,
    {
        if self.finalized {
            panic!("Can't add resource to finalized world")
        }

        logging::debug!(self.log, "registering resource";
                        "context" => "register_resource",
                        "type" => unsafe { type_name::<T>() });

        let boxed = Box::new(resource);
        self.state.resources.insert(Box::into_raw_non_null(boxed));
    }
}

pub struct GameState {
    entities: HashMap<EntityId, ComponentCoords>,
    systems: Registry<SystemId>,
    resources: AnyMap,
    shards: HashMap<ShardKey, Shard>,
    log: logging::Logger,
}

impl GameState {
    #[inline]
    pub fn new(log: logging::Logger) -> GameState {
        GameState {
            entities: HashMap::new(),
            systems: Registry::new(),
            resources: AnyMap::new(),
            shards: HashMap::new(),
            log: log.new(logging::o!()),
        }
    }
}

impl GameState {
    fn process_context(&mut self, ctx: &mut TransactionContext) {
        logging::trace!(self.log, "deleting entities"; "context" => "process_context");
        // Drain all deleted entities into the delete buffer
        for id in ctx.deleted.drain(..) {
            logging::trace!(self.log, "deleting entity"; "context" => "process_context", "id" -> id);
            if let Some(coords) = self.entities.remove(&id) {
                self.process_remove(coords);
            }
        }

        logging::trace!(self.log, "adding entities"; "context" => "process_context");
        for (&key, shard) in ctx.added.iter_mut() {
            shard.
            logging::trace!(self.log, "deleting entity"; "context" => "process_context", "id" -> id);
            // Only process shards with actual data in them
            if !shard.entity_ids.is_empty() {
                self.process_add_uniform(key, shard);
            }
        }
    }

    fn process_add_uniform(&mut self, shard_key: ShardKey, shard_def: &mut ShardDef) {
        let entity_comp_cls = EntityId::get_class();

        // Add the entity component id to the shard key
        let shard_key = shard_key + entity_comp_cls;

        let systems = &self.systems;

        // Get the shard (or add a new one)
        let shard = self.shards.entry(shard_key).or_insert_with(|| {
            let store: HashMap<_, _> = shard_def
                .components
                .keys()
                .map(|cls| (*cls, cls.comp_vec_builder()()))
                .collect();

            Shard::new(shard_key, store)
        });

        // Notify systems in case the shard was empty before
        if shard.len() == 0 {
            systems
                .iter_mut::<System>()
                .for_each(|(_, mut sys)| sys.add_shard(shard));
        }

        // Ingest the data and grab the location of the first item added
        let mut loc_start = shard.ingest(shard_def);

        // Insert entity records using the new locations
        for id in shard_def.entity_ids.drain(..) {
            self.entities.insert(id, (shard_key, loc_start));
            loc_start += 1;
        }
    }

    fn process_remove(&mut self, (shard_key, loc): ComponentCoords) {
        let shard = self.shards.get_mut(&shard_key).unwrap();

        // Update the location of the swapped-in entity
        if let Some(swapped_id) = shard.remove(loc) {
            self.entities.insert(swapped_id, (shard_key, loc));
        }

        // Remove the shard from the systems if it got emptied out
        if shard.len() == 0 {
            self.systems
                .iter_mut::<System>()
                .for_each(|(_, mut sys)| sys.remove_shard(shard_key));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_init;
    use crate::identity::{ComponentClass, Topic};
    use crate::messagebus::Message;
    use crate::system::{Components, Context, Read, Resources, Router, Write};
    use crate::topic_init;
    use serde_derive::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::marker::PhantomData;
    use std::ptr::NonNull;
    use std::rc::Rc;

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompA(i32);

    component_init!(CompA);

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompB(u64);

    component_init!(CompB);

    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompC {
        x: i32,
        y: i32,
    }

    component_init!(CompC);

    impl CompC {
        fn new(x: i32, y: i32) -> CompC {
            CompC { x, y }
        }
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct Msg1(i32);

    topic_init!(Msg1);

    #[derive(Debug, Clone, Eq, PartialEq)]
    struct Msg2(i32);

    topic_init!(Msg2);

    #[test]
    fn test_add_entity() {
        let mut world = World::default();
        world.build();

        {
            let mut batcher = world.entities().batch::<(CompA, CompB)>();
            batcher.add(CompA(1), CompB(1));
            batcher.add(CompA(2), CompB(2));
            batcher.commit();
        }

        world.entities().add((CompA(3), CompB(3), CompC::new(3, 3)));

        world.process_transactions();

        assert_eq!(world.state.entities.len(), 3);
        assert_eq!(world.state.shards.len(), 2);
        assert_eq!(
            world.state.entities[&0.into()],
            (EntityId::get_class() + CompA::get_class() + CompB::get_class(), 0)
        );
        assert_eq!(
            world.state.entities[&1.into()],
            (EntityId::get_class() + CompA::get_class() + CompB::get_class(), 1)
        );
        assert_eq!(
            world.state.entities[&2.into()],
            (
                EntityId::get_class() + CompA::get_class() + CompB::get_class() + CompC::get_class(),
                0
            )
        );
    }

    #[test]
    fn test_remove_entity() {
        let mut world = World::default();
        world.build();

        {
            let mut batcher = world.entities().batch::<(CompA, CompB)>();
            batcher.add(CompA(1), CompB(1));
            batcher.add(CompA(2), CompB(2));
            batcher.add(CompA(3), CompB(3));
            batcher.add(CompA(4), CompB(4));
            batcher.commit();
        }

        world.process_transactions();
        assert_eq!(world.state.entities.len(), 4);
        assert_eq!(world.state.entities[&0.into()].1, 0);
        assert_eq!(world.state.entities[&1.into()].1, 1);
        assert_eq!(world.state.entities[&2.into()].1, 2);
        assert_eq!(world.state.entities[&3.into()].1, 3);

        world.entities().remove(0.into());

        world.process_transactions();
        assert_eq!(world.state.entities.len(), 3);
        assert_eq!(world.state.entities[&1.into()].1, 1);
        assert_eq!(world.state.entities[&2.into()].1, 2);
        assert_eq!(world.state.entities[&3.into()].1, 0);

        world.entities().remove(1.into());

        world.process_transactions();
        assert_eq!(world.state.entities.len(), 2);
        assert_eq!(world.state.entities[&2.into()].1, 1);
        assert_eq!(world.state.entities[&3.into()].1, 0);

        world.entities().remove(3.into());

        world.process_transactions();
        assert_eq!(world.state.entities.len(), 1);
        assert_eq!(world.state.entities[&2.into()].1, 0);

        world.entities().remove(2.into());

        world.process_transactions();
        assert_eq!(world.state.entities.len(), 0);
    }

    #[test]
    fn test_resources() {
        struct TestResource1 {
            x: i32,
        }

        struct TestResource2 {
            x: i32,
        }

        struct TestSystem<'a> {
            _p: PhantomData<&'a ()>,
        }

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Resources<(Read<'a, TestResource1>, Write<'a, TestResource2>)>;

            fn run(&mut self, mut ctx: Context<Self::Data>, _tx: &mut TransactionContext, _msg: Router) {
                let (r1, mut r2) = ctx.resources();
                r2.x = r1.x;
            }
        }

        let mut world = World::default();
        world.register_resource(TestResource1 { x: 100 });
        world.register_resource(TestResource2 { x: 0 });
        world.register_system(TestSystem { _p: PhantomData });
        world.build();

        world.run_once();

        let resource_val = world.state.resources.get::<NonNull<TestResource2>>().unwrap();

        assert_eq!(unsafe { resource_val.as_ref() }.x, 100)
    }

    #[test]
    fn test_ingest_system_transactions() {
        // Create a system that adds a new entity and removes an existing one
        struct TestSystem<'a> {
            _p: PhantomData<&'a ()>,
        }

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = Components<(Read<'a, EntityId>, Read<'a, CompA>, Write<'a, CompB>)>;

            fn run(&mut self, _ctx: Context<Self::Data>, tx: &mut TransactionContext, _msg: Router) {
                tx.add((CompA(3), CompB(3)));
                tx.remove(0.into());
            }
        }

        let mut world = World::default();
        world.register_system(TestSystem { _p: PhantomData });
        world.build();

        {
            let mut batcher = world.entities().batch::<(CompA, CompB)>();
            batcher.add(CompA(0), CompB(0));
            batcher.add(CompA(1), CompB(1));
            batcher.add(CompA(2), CompB(2));
            batcher.commit();
        }

        // Process the initial state
        world.process_transactions();

        assert_eq!(world.state.entities.len(), 3);
        assert_eq!(world.state.entities[&0.into()].1, 0);
        assert_eq!(world.state.entities[&1.into()].1, 1);
        assert_eq!(world.state.entities[&2.into()].1, 2);

        // Run the system, triggering the edit and addition
        world.run_once();
        world.process_transactions();

        assert_eq!(world.state.entities.len(), 3);
        assert_eq!(world.state.entities[&1.into()].1, 1);
        assert_eq!(world.state.entities[&2.into()].1, 0);
        assert_eq!(world.state.entities[&3.into()].1, 2);
    }

    #[test]
    fn test_system_messaging() {
        struct TestSystem1<'a> {
            _p: PhantomData<&'a ()>,
            messages: Rc<RefCell<Vec<Msg1>>>,
        }

        impl<'a> RunSystem for TestSystem1<'a> {
            type Data = ();

            fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, mut msg: Router) {
                for message in msg.read::<Msg1>() {
                    self.messages.borrow_mut().push(message.clone());
                }

                msg.publish(Msg2(0));
                msg.publish(Msg2(1));
                msg.publish(Msg2(2));
            }
        }

        struct TestSystem2<'a> {
            _p: PhantomData<&'a ()>,
            messages: Rc<RefCell<Vec<Msg2>>>,
        }

        impl<'a> RunSystem for TestSystem2<'a> {
            type Data = ();

            fn run(&mut self, _ctx: Context<Self::Data>, _tx: &mut TransactionContext, mut msg: Router) {
                for message in msg.read::<Msg2>() {
                    self.messages.borrow_mut().push(message.clone());
                }

                msg.publish(Msg1(0));
                msg.publish(Msg1(1));
            }
        }

        let system_messages1 = Rc::new(RefCell::new(Vec::new()));
        let system_messages2 = Rc::new(RefCell::new(Vec::new()));

        let mut world = World::default();

        world.register_system(TestSystem1 {
            _p: PhantomData,
            messages: system_messages1.clone(),
        });

        world.register_system(TestSystem2 {
            _p: PhantomData,
            messages: system_messages2.clone(),
        });
        world.build();

        // Run the world iteration once, propagating the messages
        world.run_once();

        assert_eq!(world.messages.read::<Msg1>(), &[Msg1(0), Msg1(1)]);
        assert_eq!(world.messages.read::<Msg2>(), &[Msg2(0), Msg2(1), Msg2(2)]);

        // Run the world iteration the second time, allowing the systems to ingest the messages
        world.run_once();

        assert_eq!(*system_messages1.borrow(), vec![Msg1(0), Msg1(1)]);
        assert_eq!(*system_messages2.borrow(), vec![Msg2(0), Msg2(1), Msg2(2)]);
    }
}
