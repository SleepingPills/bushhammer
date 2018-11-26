use crate::component2::{Column, Component, Shard, ShardedColumn};
use crate::entity2::dynamic::DynVec;
use crate::entity2::{Entity, EntityId, TransactionContext};
use crate::identity2::{ComponentId, ShardKey, SystemId};
use crate::registry::{Registry, TraitBox};
use crate::sentinel;
use crate::sync::RwCell;
use crate::system2::{System, SystemEntry, SystemRuntime};
use hashbrown::HashMap;
use std::any::TypeId;
use std::sync::Arc;
use std::sync::atomic::{ATOMIC_USIZE_INIT};

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

    /// Horribly unsafe function to get mutable references to multiple elements of the system
    /// transactions without having to drain and rebuild the vector all the time.
    #[inline]
    unsafe fn get_system_transactions<'a>(&self, idx: usize) -> &'a mut TransactionContext {
        let ptr = self.system_transactions.as_ptr() as *mut TransactionContext;
        &mut *ptr.add(idx)
    }
}

impl World {
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
            self.process_add_uniform(key, shard);
        }
    }

    fn process_add_uniform(&mut self, shard_key: ShardKey, shard_def: &mut HashMap<ComponentId, DynVec>) {
        let comp_reg = &self.component_registry;
        let sys_reg = &self.system_registry;

        // Get the shard (or add a new one)
        let shard = self.shards.entry(shard_key).or_insert_with(|| {
            let sections: HashMap<_, _> = shard_def
                .keys()
                .map(|cid| (*cid, comp_reg.get_trait::<Column>(cid).write().new_section()))
                .collect();

            let shard = Shard::new(shard_key, sections);

            // Notify systems that a new shard was added
            sys_reg
                .iter_mut::<SystemRuntime>()
                .filter(|(_, sys)| sys.check_shard(shard_key))
                .for_each(|(_, mut sys)| sys.add_shard(&shard));

            shard
        });

        // TODO: Figure out some nice way of adding entities in batches ASSUMING the shard_def already
        // has the entity ids inserted correctly.

        /*
        // Ingest all components and stash away the coordinates.
        let mut components = HashMap::new();
        
        let shard_loc = ent_def
            .components
            .drain()
            .map(|(comp_id, comp_def)| {
                self.get_column(comp_id).apply_mut(|column| {
                    let section = shard.get_section(comp_id);
                    components.insert(comp_id, section);
        
                    match comp_def {
                        entity::CompDef::Boxed(boxed) => column.ingest_box(boxed, section),
                        entity::CompDef::Json(json) => column.ingest_json(json, section),
                        _ => panic!("No-op component definition on a new entity"),
                    }
                })
            })
            .fold1(|acc, loc| {
                if acc != loc {
                    panic!("Divergent section locations")
                }
        
                loc
            })
            .unwrap();
        
        let ent = entity::Entity {
            id,
            shard_id,
            shard_loc,
            comp_sections: components,
        };
        
        self.entity_registry.insert(ent.id, ent);
        */
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
