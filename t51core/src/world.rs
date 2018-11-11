use crate::component;
use crate::entity;
use crate::object::{ComponentId, EntityId, ShardId, SystemId};
use crate::registry::Registry;
use crate::registry::TraitBox;
use crate::sentinel;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;
use sequence_trie::SequenceTrie;
use std::hash::{Hash, BuildHasher, BuildHasherDefault};

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: IndexMap<SystemId, Box<system::SystemRuntime>>,
    shards: HashMap<ShardId, component::Shard>,
    shard_trie: SequenceTrie<ComponentId, ShardId>,
    transactions: sentinel::Take<Vec<entity::Transaction>>,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> entity::EntityStore {
        entity::EntityStore::new(&self.entity_registry, &mut self.transactions)
    }
}

impl World {
    #[inline]
    pub fn run(&mut self) {
        self.process_transactions();
        self.process_systems();
    }
}

impl World {
    /// Drain all the system transactions into the common transaction queue.
    fn collect_transactions(&mut self) {
        let transactions = &mut self.transactions;

        for (_, system) in self.system_registry.iter_mut() {
            transactions.append(system.get_transactions());
        }
    }

    /// Process all transactions in the queue.
    fn process_transactions(&mut self) {
        self.collect_transactions();

        // Take the transactions out
        let mut transactions = self.transactions.take();

        for transaction in transactions.drain(..) {
            match transaction {
                entity::Transaction::AddEnt(ent_def) => self.apply_add(ent_def),
                entity::Transaction::EditEnt(id, ent_def) => self.apply_edit(id, ent_def),
                entity::Transaction::RemoveEnt(id) => self.apply_remove(id),
            }
        }

        self.transactions.put(transactions)
    }

    /// Add a new entity to the world.
    fn apply_add(&mut self, mut ent_def: entity::EntityDef) {
        let id = self.next_entity_id();

        // Add the id as a mandatory extra component
        ent_def
            .components
            .insert(ComponentId::from::<EntityId>(), entity::CompDef::Boxed(Box::new(id)));

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
        for (comp_id, comp_def) in ent_def.components.drain() {
            let column = &mut self.get_column(comp_id).write();

            let section = shard.get_loc(comp_id);

            let loc = match comp_def {
                entity::CompDef::Boxed(boxed) => column.ingest_box(boxed, section),
                entity::CompDef::Json(json) => column.ingest_json(json, section),
                _ => panic!("No-op component definition on a new entity"),
            };

            components.insert(comp_id, (section, loc));
        }

        let entity = entity::Entity {
            id,
            shard_id: shard.id,
            components,
        };

        self.entity_registry.insert(entity.id, entity);
    }

    /// Edit an existing entity.
    fn apply_edit(&mut self, id: EntityId, ent_def: entity::EntityDef) {
        // Retrieve current entity
        // Compare set of components on entity and the new definition
        // If they are the same, just apply the transactions as it is clearly just an update
        //
        // If they are different, the existing component entries need to be removed
        // Get/create new shard
        // We get the id of the last entry in the array (it will be swapped in)
        // We retrieve the last entity entry
        // Iterate entries of the old entity:
        // For each entry, we pop the new definition out of the indexmap
        // If it's a Nop, we transfer over to the new shard
        // If it's a component def, we add the new one and drop the old one
        // If it is missing, we simply delete the old one.
        // In each case, we update the last entry entity to the swapped in index
        // We go over any remaining entries in the entity definition and add them
        if let Some(current_ent) = self.entity_registry.remove(&id) {
            // Check if the new definition has the same set of components
            if current_ent.components.len() == ent_def.components.len()
                && (current_ent.components.keys().all(|cid| ent_def.components.contains_key(cid)))
            {

            } else {

            }
        }
    }

    /// Remove an existing entity from the world.
    fn apply_remove(&mut self, id: EntityId) {
        unimplemented!()
    }

    /// Create a new shard based on the supplied component combination and return it's ID.
    fn create_shard(&mut self, components: &Vec<ComponentId>) -> ShardId {
        // TODO: This should notify all systems interested in this component set!!
        // TODO: Check if we actually need shards to be in a trie? Why?f`
        let sections: HashMap<_, _> = components
            .iter()
            .map(|&cid| (cid, self.get_column(cid).write().new_section()))
            .collect();

        let id = self.shards.len();
        let shard = component::Shard::new(id, sections);
        self.shards.insert(id, shard);
        id
    }

    /// Get the next entity id.
    #[inline]
    fn next_entity_id(&self) -> EntityId {
        return self.entity_registry.len();
    }

    #[inline]
    fn get_column(&self, comp_id: ComponentId) -> TraitBox<component::Column> {
        self.component_registry.get_trait::<component::Column>(&comp_id)
    }

    fn get_shard_composite_key(&self, iterable: impl Iterator<Item=ComponentId>) -> usize {
        unimplemented!()
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
        let id = SystemId::from::<T>();
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
        let id = ComponentId::from::<T>();
        let store = component::ShardedColumn::<T>::new();

        self.component_registry.register(id, store);
    }
}
