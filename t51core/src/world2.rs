use crate::component2::{Column, Component, Shard, ShardedColumn};
use crate::entity2::dynamic::DynVec;
use crate::entity2::{Entity, EntityId, ShardDef, TransactionContext};
use crate::identity2::{ComponentId, ShardKey, SystemId};
use crate::registry::{Registry, TraitBox};
use crate::sentinel;
use crate::sync::RwCell;
use crate::system2::{System, SystemEntry, SystemRuntime};
use hashbrown::HashMap;
use std::any::TypeId;
use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::Arc;

pub struct World {
    // Entity Handling
    entity_registry: HashMap<EntityId, Entity>,
    entity_del_buffer: HashMap<ShardKey, Vec<Entity>>,

    // Systems
    system_registry: Registry<SystemId>,

    // Components & Shards
    component_registry: Registry<ComponentId>,
    shards: HashMap<ShardKey, Shard>,

    system_transactions: Vec<TransactionContext>,
    transactions: sentinel::Take<TransactionContext>,

    // Reference Data
    system_ids: HashMap<TypeId, SystemId>,
    finalized: bool,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> &mut TransactionContext {
        if self.finalized {
            panic!("World must be finalized before adding entities")
        }

        &mut self.transactions
    }
}

impl World {
    #[inline]
    pub fn new() -> Self {
        let mut world = World {
            entity_del_buffer: HashMap::new(),
            component_registry: Registry::new(),
            entity_registry: HashMap::new(),
            system_registry: Registry::new(),
            shards: HashMap::new(),
            system_transactions: Vec::new(),
            transactions: sentinel::Take::new(TransactionContext::new(Arc::new(ATOMIC_USIZE_INIT))),
            system_ids: HashMap::new(),
            finalized: false,
        };
        // Entity ID is always a registered component
        world.register_component::<EntityId>();
        world
    }

    pub fn build(&mut self) {
        self.finalized = true;

        // Create a copy of the main transaction context for each system so they can be run in parallel
        for _ in 0..self.system_registry.len() {
            self.system_transactions.push(self.transactions.clone());
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
    pub fn create_runtime<T>(&self, system: T) -> SystemEntry<T>
    where
        T: System,
    {
        SystemEntry::new(system, &self.component_registry)
    }

    /// Register the supplied system with the world.
    pub fn register_system<T>(&mut self, system: T) -> SystemId
    where
        T: 'static + System,
    {
        if self.finalized {
            panic!("Can't add systems to finalized world")
        }

        let runtime = self.create_runtime(system);
        let id = SystemId::new::<T>(self.system_registry.len());

        self.system_registry.register(id, runtime);
        self.system_registry.register_trait::<SystemEntry<T>, SystemRuntime>(&id);
        self.system_ids.insert(TypeId::of::<T>(), id);
        id
    }

    /// Process all currently registered systems.
    #[inline]
    pub fn process_systems(&mut self) {
        for (id, mut system) in self.system_registry.iter_mut::<SystemRuntime>() {
            unsafe {
                system.run(self.get_system_transactions(id.indexer()));
            }
        }
    }

    #[inline]
    pub fn get_system<T>(&self, id: SystemId) -> Arc<RwCell<SystemEntry<T>>>
    where
        T: 'static + System,
    {
        self.system_registry.get::<SystemEntry<T>>(&id)
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
    // TODO: Adjust this such that it extracts all the data that will be needed from self and
    // passes them as individual arguments. No need to borrow self then.
    /// Process all transactions in the queue.
    pub fn process_transactions(&mut self) {
        let mut main_tx = self.transactions.take();
        self.process_context(&mut main_tx);
        self.transactions.put(main_tx);

        for i in 0..self.system_transactions.len() {
            unsafe {
                let tx = self.get_system_transactions(i);
                self.process_context(tx);
            }
        }

        self.process_removals();
    }

    fn process_context(&mut self, ctx: &mut TransactionContext) {
        // Drain all deleted entities into the delete buffer
        for id in ctx.deleted.drain(..) {
            if let Some(entity) = self.entity_registry.remove(&id) {
                let buffer = self.entity_del_buffer.entry(entity.shard_key).or_insert_with(|| Vec::new());
                buffer.push(entity);
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

        let comp_reg = &self.component_registry;
        let sys_reg = &self.system_registry;

        // Get the shard (or add a new one)
        let shard = self.shards.entry(shard_key).or_insert_with(|| {
            let mut sections: HashMap<_, _> = shard_def
                .components
                .keys()
                .map(|cid| (*cid, comp_reg.get_trait::<Column>(cid).write().new_section()))
                .collect();

            sections.insert(
                entity_comp_id,
                comp_reg.get_trait::<Column>(&entity_comp_id).write().new_section(),
            );

            Shard::new(shard_key, sections)
        });

        // Drain the component data into the columns
        for (comp_id, data) in shard_def.components.iter_mut() {
            let section = shard.sections[&comp_id];
            let mut column = self.component_registry.get_trait::<Column>(comp_id).write();
            column.ingest(&shard_def.entity_ids, data, section);
        }

        // Register the entities, drain the entity Ids into the relevant column and notify the systems
        // in case the shard was just added or repopulated.
        let mut entity_id_column = self.component_registry.get::<ShardedColumn<EntityId>>(&entity_comp_id).write();
        let entity_id_section = shard.sections[&entity_comp_id];

        // Notify systems in case the shard length was zero
        if entity_id_column.section_len(entity_id_section) == 0 {
            sys_reg
                .iter_mut::<SystemRuntime>()
                .filter(|(_, sys)| sys.check_shard(shard_key))
                .for_each(|(_, mut sys)| sys.add_shard(shard));
        }

        for &entity_id in shard_def.entity_ids.iter() {
            self.entity_registry.insert(
                entity_id,
                Entity {
                    id: entity_id,
                    shard_key,
                },
            );
        }

        entity_id_column.ingest_entity_ids(&shard_def.entity_ids, entity_id_section);
        entity_id_column.ingest_core(&mut shard_def.entity_ids, entity_id_section);
    }

    fn process_removals(&mut self) {
        unimplemented!()
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
        let store = ShardedColumn::<T>::new();

        // Add the store to the registry
        self.component_registry.register(id, store);
        self.component_registry.register_trait::<ShardedColumn<T>, Column>(&id);

        // Register the entity builder vector type
        self.transactions.add_builder::<T>();
    }
}
