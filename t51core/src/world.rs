use crate::component;
use crate::entity;
use crate::object::{ComponentId, EntityId, ShardId, SystemId};
use crate::registry::Registry;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;
use sequence_trie::SequenceTrie;

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: IndexMap<SystemId, Box<system::SystemRuntime>>,
    shards: HashMap<ShardId, component::Shard>,
    shard_trie: SequenceTrie<ComponentId, ShardId>,
    transactions: Option<Vec<entity::Transaction>>,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> entity::EntityStore {
        entity::EntityStore::new(&self.entity_registry, self.transactions.as_mut().unwrap())
    }
}

impl World {
    pub fn run(&mut self) {
        self.process_transactions();
        self.process_systems();
    }
}

impl World {
    /// Drain all the system transactions into the common transaction queue.
    fn collect_transactions(&mut self) {
        let transactions = self.transactions.as_mut().unwrap();

        for (_, system) in self.system_registry.iter_mut() {
            transactions.append(system.get_transactions());
        }
    }

    /// Process all transactions in the queue.
    fn process_transactions(&mut self) {
        self.collect_transactions();

        // Take the transactions out
        let mut transactions = self.transactions.take().unwrap();

        for transaction in transactions.drain(..) {
            match transaction {
                entity::Transaction::AddEnt(ent_def) => self.apply_add(ent_def),
                entity::Transaction::EditEnt(id, ent_def) => self.apply_edit(id, ent_def),
                entity::Transaction::RemoveEnt(id) => self.apply_remove(id),
            }
        }

        self.transactions = transactions.into();
    }

    /// Add a new entity to the world.
    fn apply_add(&mut self, mut ent_def: entity::EntityDef) {
        // Prepare a sorted list of components defined on the new entity
        let mut shard_comp: Vec<ComponentId> = ent_def.components.keys().cloned().collect();
        shard_comp.sort();

        // Check if a shard exists with the component combination
        let shard = match self.shard_trie.get(&shard_comp) {
            Some(shard_id) => &self.shards[shard_id],
            _ => {
                let shard_id = self.create_shard(&shard_comp);
                &self.shards[&shard_id]
            }
        };

        // Ingest all components and stash away the coordinates
        let mut components = HashMap::new();
        for (comp_id, comp_def) in ent_def.components.drain(..) {
            let column = &mut self.component_registry.get_trait::<component::Column>(&comp_id).write();

            let section = shard.get_loc(comp_id);

            let loc = match comp_def {
                entity::CompDef::Boxed(boxed) => column.ingest_box(boxed, section),
                entity::CompDef::Json(json) => column.ingest_json(json, section),
                _ => panic!("No-op component definition on a new entity"),
            };

            components.insert(comp_id, (section, loc));
        }

        let entity = entity::Entity {
            id: self.next_entity_id(),
            shard_id: shard.id,
            components,
        };

        self.entity_registry.insert(entity.id, entity);
    }

    /// Edit an existing entity.
    fn apply_edit(&mut self, id: EntityId, ent_def: entity::EntityDef) {
        unimplemented!()
    }

    /// Remove an existing entity from the world.
    fn apply_remove(&mut self, id: EntityId) {
        unimplemented!()
    }

    /// Create a new shard based on the supplied component combination and return it's ID.
    fn create_shard(&mut self, components: &Vec<ComponentId>) -> ShardId {
        unimplemented!()
    }

    /// Get the next entity id.
    fn next_entity_id(&self) -> EntityId {
        return self.entity_registry.len();
    }
}

impl World {
    /// Create a new runtime using the supplied system. The runtime is wired up with
    /// all required subsystems and ready to execute.
    #[inline]
    pub fn create_runtime<T>(&self, system: T) -> system::SystemEntry<T>
    where
        T: system::System,
    {
        system::SystemEntry::new(system, &self.component_registry)
    }

    /// Register the supplied system with the world.
    pub fn register_system<T>(&mut self, system: T)
    where
        T: 'static + system::System,
    {
        let id = SystemId::new::<T>();
        let runtime = self.create_runtime(system);

        self.system_registry.insert(id, Box::new(runtime));
    }

    /// Process all currently registered systems.
    pub fn process_systems(&mut self) {
        for (_, system) in self.system_registry.iter_mut() {
            system.run(&self.entity_registry);
        }
    }
}

impl World {
    /// Register the supplied component type.
    pub fn register_component<T>(&mut self)
    where
        T: 'static,
    {
        let id = ComponentId::new::<T>();
        let store = component::ShardedColumn::<T>::new();

        self.component_registry.register(id, store);
    }
}
