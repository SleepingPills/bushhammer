use crate::component;
use crate::entity;
use crate::object::{ComponentId, EntityId, ShardId, SystemId};
use crate::registry::Registry;
use crate::registry::TraitBox;
use crate::sentinel;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::any::TypeId;

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: IndexMap<SystemId, Box<system::SystemRuntime>>,
    shards: HashMap<ShardId, component::Shard>,
    shards_map: HashMap<component::ShardKey, ShardId>,
    transactions: sentinel::Take<Vec<entity::Transaction>>,
    component_ids: HashMap<TypeId, ComponentId>,
    system_ids: HashMap<TypeId, SystemId>,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> entity::EntityStore {
        entity::EntityStore::new(&self.entity_registry, &self.component_ids, &mut self.transactions)
    }
}

impl World {
    #[inline]
    pub fn run(&mut self) {
        self.process_transactions();
        self.process_systems();
    }

    #[inline]
    pub fn new() -> Self {
        let mut world = World {
            component_registry: Registry::new(),
            entity_registry: HashMap::new(),
            system_registry: IndexMap::new(),
            shards: HashMap::new(),
            shards_map: HashMap::new(),
            transactions: sentinel::Take::new(Vec::new()),
            component_ids: HashMap::new(),
            system_ids: HashMap::new(),
        };
        // Entity ID is always a registered component
        world.register_component::<EntityId>();
        world
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
            .insert(self.get_component_id::<EntityId>(), entity::CompDef::Boxed(Box::new(id)));

        let shard_id = self.get_shard_id(&ent_def);
        let shard = &self.shards[&shard_id];

        // Ingest all components and stash away the coordinates
        let mut components = HashMap::new();
        for (comp_id, comp_def) in ent_def.components.drain(..) {
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

    fn get_shard_id(&mut self, ent_def: &entity::EntityDef) -> ShardId {
        let shard_key = component::composite_key(ent_def.components.keys());

        match self.shards_map.get(&shard_key) {
            Some(&id) => id,
            _ => self.create_shard(ent_def.components.keys(), shard_key),
        }
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
        // TODO: This might be just an apply_remove followed by an apply_add?!? It isn't because Nops have
        // to be handled - the new entity def doesn't have all the components of the old one.
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
        if let Some(entity) = self.entity_registry.remove(&id) {

        }
    }

    /// Create a new shard based on the supplied component combination and return it's ID.
    fn create_shard<'a>(&mut self, components: impl Iterator<Item = &'a ComponentId>, shard_key: component::ShardKey) -> ShardId {
        let sections: HashMap<_, _> = components
            .map(|&cid| (cid, self.get_column(cid).write().new_section()))
            .collect();

        let id = self.shards.len() as ShardId;
        let shard = component::Shard::new(id, sections);
        self.notify_systems_add_shard(&shard, shard_key);
        self.shards.insert(id, shard);
        id
    }

    /// Get the next entity id.
    #[inline]
    fn next_entity_id(&self) -> EntityId {
        return self.entity_registry.len() as EntityId;
    }

    #[inline]
    fn get_column(&self, comp_id: ComponentId) -> TraitBox<component::Column> {
        self.component_registry.get_trait::<component::Column>(&comp_id)
    }

    #[inline]
    fn get_component_id<T: 'static>(&self) -> ComponentId {
        self.component_ids[&TypeId::of::<T>()]
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
        system::SystemEntry::new(system, &self.component_registry, &self.component_ids)
    }

    /// Register the supplied system with the world.
    pub fn register_system<T>(&mut self, system: T)
    where
        T: 'static + system::System,
    {
        let id = SystemId::new::<T>(self.system_registry.len());
        let runtime = self.create_runtime(system);
        self.system_registry.insert(id, Box::new(runtime));
        self.system_ids.insert(TypeId::of::<T>(), id);
    }

    /// Process all currently registered systems.
    #[inline]
    pub fn process_systems(&mut self) {
        for (_, system) in self.system_registry.iter_mut() {
            system.run(&self.entity_registry, &self.component_ids);
        }
    }

    /// Notify each relevant system that a shard was added
    fn notify_systems_add_shard(&mut self, shard: &component::Shard, shard_key: component::ShardKey) {
        self.system_registry
            .values_mut()
            .filter(|sys| sys.check_shard(shard_key))
            .for_each(|sys| sys.add_shard(shard));
    }

    /// Notify each relevant system that a shard was added
    fn notify_systems_remove_shard(&mut self, shard_id: ShardId, shard_key: component::ShardKey) {
        self.system_registry
            .values_mut()
            .filter(|sys| sys.check_shard(shard_key))
            .for_each(|sys| sys.remove_shard(shard_id));
    }
}

impl World {
    /// Register the supplied component type.
    pub fn register_component<T>(&mut self)
    where
        T: 'static,
    {
        let id = ComponentId::new::<T>(self.component_registry.len());
        let store = component::ShardedColumn::<T>::new();

        self.component_registry.register(id, store);
        self.component_ids.insert(TypeId::of::<T>(), id);
    }
}
