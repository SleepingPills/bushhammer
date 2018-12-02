use crate::component2::Component;
use crate::component3::{ComponentCoords, ComponentVec, Shard};
use crate::entity2::{EntityId, ShardDef, TransactionContext};
use crate::identity2::{ComponentId, ShardKey, SystemId};
use crate::registry::Registry;
use crate::sync::RwCell;
use crate::system3::{RunSystem, System, SystemRuntime};
use hashbrown::HashMap;
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::Arc;

pub struct World {
    state: GameState,

    // Transactions
    system_transactions: Vec<TransactionContext>,
    transactions: TransactionContext,
    finalized: bool,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> &mut TransactionContext {
        if !self.finalized {
            panic!("World must be finalized before adding entities")
        }

        &mut self.transactions
    }
}

impl World {
    #[inline]
    pub fn new() -> Self {
        let mut world = World {
            state: GameState::new(),
            system_transactions: Vec::new(),
            transactions: TransactionContext::new(Arc::new(ATOMIC_USIZE_INIT)),
            finalized: false,
        };
        // Entity ID is always a registered component
        world.register_component::<EntityId>();
        world
    }

    pub fn build(&mut self) {
        self.finalized = true;

        // Create a copy of the main transaction context for each system so they can be run in parallel
        for _ in 0..self.state.systems.len() {
            self.system_transactions.push(self.transactions.clone());
        }
    }

    /// Process all transactions in the queue.
    pub fn process_transactions(&mut self) {
        self.state.process_context(&mut self.transactions);

        for tx in self.system_transactions.iter_mut() {
            self.state.process_context(tx);
        }
    }

    #[inline]
    pub fn run_once(&mut self) {
        self.process_transactions();
        self.process_systems();
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

        self.state.systems.register(id, runtime);
        self.state.systems.register_trait::<SystemRuntime<T>, System>(&id);
        id
    }

    /// Process all currently registered systems.
    #[inline]
    pub fn process_systems(&mut self) {
        for (id, mut system) in self.state.systems.iter_mut::<System>() {
            unsafe {
                system.run(&self.state.entities, self.get_system_transactions(id.indexer()));
            }
        }
    }

    #[inline]
    pub fn get_system<T>(&self, id: SystemId) -> Arc<RwCell<SystemRuntime<T>>>
    where
        T: 'static + RunSystem,
    {
        self.state.systems.get::<SystemRuntime<T>>(&id)
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
    /// Register the supplied component type.
    pub fn register_component<T>(&mut self)
    where
        T: 'static + Component,
    {
        if self.finalized {
            panic!("Can't add component to finalized world")
        }

        let id = T::acquire_unique_id();

        // Register the entity and component builder vector types
        self.transactions.add_builder::<T>();
        self.state.builders.insert(id, Box::new(|| Box::new(Vec::<T>::new())));
    }
}

pub struct GameState {
    entities: HashMap<EntityId, ComponentCoords>,
    systems: Registry<SystemId>,

    shards: HashMap<ShardKey, Shard>,
    builders: HashMap<ComponentId, Box<Fn() -> Box<ComponentVec>>>,
}

impl GameState {
    #[inline]
    pub fn new() -> GameState {
        GameState {
            entities: HashMap::new(),
            systems: Registry::new(),
            shards: HashMap::new(),
            builders: HashMap::new(),
        }
    }
}

impl GameState {
    fn process_context(&mut self, ctx: &mut TransactionContext) {
        // Drain all deleted entities into the delete buffer
        for id in ctx.deleted.drain(..) {
            if let Some(coords) = self.entities.remove(&id) {
                self.process_remove(coords);
            }
        }

        for (&key, shard) in ctx.added.iter_mut() {
            // Only process shards with actual data in them
            if shard.entity_ids.len() > 0 {
                self.process_add_uniform(key, shard);
            }
        }
    }

    fn process_add_uniform(&mut self, shard_key: ShardKey, shard_def: &mut ShardDef) {
        let entity_comp_id = EntityId::get_unique_id();

        // Add the entity component id to the shard key
        let shard_key = shard_key + entity_comp_id;

        let systems = &self.systems;
        let builders = &self.builders;

        // Get the shard (or add a new one)
        let shard = self.shards.entry(shard_key).or_insert_with(|| {
            let store: HashMap<_, _> = shard_def.components.keys().map(|cid| (*cid, builders[cid]())).collect();

            Shard::new(shard_key, store)
        });

        // Notify systems in case the shard was empty before
        if shard.len() == 0 {
            systems.iter_mut::<System>().for_each(|(_, mut sys)| sys.add_shard(shard));
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
    use serde_derive::{Deserialize, Serialize};
    use std::marker::PhantomData;
    use t51core_proc::Component;
    use crate::system3::store::Read;
    use crate::system3::store::Write;
    use crate::system3::Context;

    #[derive(Component, Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompA(i32);

    #[derive(Component, Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompB(u64);

    #[derive(Component, Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct CompC {
        x: i32,
        y: i32,
    }

    impl CompC {
        fn new(x: i32, y: i32) -> CompC {
            CompC { x, y }
        }
    }

    #[test]
    fn test_add_entity() {
        let mut world = World::new();
        world.register_component::<CompA>();
        world.register_component::<CompB>();
        world.register_component::<CompC>();
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
            (EntityId::get_unique_id() + CompA::get_unique_id() + CompB::get_unique_id(), 0)
        );
        assert_eq!(
            world.state.entities[&1.into()],
            (EntityId::get_unique_id() + CompA::get_unique_id() + CompB::get_unique_id(), 1)
        );
        assert_eq!(
            world.state.entities[&2.into()],
            (
                EntityId::get_unique_id() + CompA::get_unique_id() + CompB::get_unique_id() + CompC::get_unique_id(),
                0
            )
        );
    }

    #[test]
    fn test_remove_entity() {
        let mut world = World::new();
        world.register_component::<CompA>();
        world.register_component::<CompB>();
        world.register_component::<CompC>();
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
    fn test_ingest_system_transactions() {
        // Create a system that adds a new entity and removes an existing one
        struct TestSystem<'a> {
            _p: PhantomData<&'a ()>,
        }

        impl<'a> RunSystem for TestSystem<'a> {
            type Data = (Read<'a, EntityId>, Read<'a, CompA>, Write<'a, CompB>);

            fn run(&mut self, _data: Context<Self::Data>, tx: &mut TransactionContext) {
                tx.add((CompA(3), CompB(3)));
                tx.remove(0.into());
            }
        }

        let mut world = World::new();
        world.register_component::<CompA>();
        world.register_component::<CompB>();
        world.register_component::<CompC>();
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
}
