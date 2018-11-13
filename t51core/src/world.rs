use crate::component;
use crate::entity;
use crate::object::{ComponentId, EntityId, ShardId, SystemId};
use crate::registry::Registry;
use crate::registry::TraitBox;
use crate::sentinel;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;
use itertools::Itertools;
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
        let transactions = &mut self.transactions.take();

        for transaction in transactions.drain(..) {
            match transaction {
                entity::Transaction::AddEnt(ent_def) => self.apply_add(ent_def),
                entity::Transaction::EditEnt(id, ent_def) => self.apply_edit(id, ent_def),
                entity::Transaction::RemoveEnt(id) => self.apply_remove(id),
            }
        }
    }

    /// Add a new entity to the world.
    fn apply_add(&mut self, ent_def: entity::EntityDef) {
        self.add_entity_core(self.next_entity_id(), ent_def);
    }

    /// Edit an existing entity.
    fn apply_edit(&mut self, id: EntityId, mut ent_def: entity::EntityDef) {
        if let Some(mut entity) = self.entity_registry.remove(&id) {
            // If the entity composition didn't change, just update as necessary
            if component::composite_key(entity.comp_sections.keys()) == component::composite_key(ent_def.components.keys()) {
                let shard = &self.shards[&entity.shard_id];

                for (comp_id, comp_def) in ent_def.components.drain(..) {
                    let column = &mut self.get_column(comp_id).write();
                    let section = shard.get_section(comp_id);

                    if let entity::CompDef::Boxed(boxed) = comp_def {
                        column.update_box(boxed, section, entity.shard_loc);
                    } else if let entity::CompDef::Json(json) = comp_def {
                        column.update_json(json, section, entity.shard_loc);
                    }
                }

                self.entity_registry.insert(id, entity);
            } else {
                // Remove all the components from the entity, stashing away those that need to be transferred
                // due to Nops on the new definition
                let mut transfer = Vec::new();
                for (comp_id, section) in entity.comp_sections.drain(..) {
                    let mut column = self.component_registry.get_trait::<component::Column>(&comp_id).write();

                    match ent_def.components.get(&comp_id) {
                        Some(entity::CompDef::Nop()) => {
                            let boxed = Box::new(column.swap_remove_return(section, entity.shard_loc));
                            transfer.push((comp_id, entity::CompDef::Boxed(boxed)))
                        }
                        _ => column.swap_remove(section, entity.shard_loc),
                    }
                }
                ent_def.components.extend(transfer);

                // Handle the swapped entity
                self.handle_swapped(&entity);

                // Add the new definition under the old id
                self.add_entity_core(id, ent_def);
            }
        }
    }

    /// Remove an existing entity from the world.
    fn apply_remove(&mut self, id: EntityId) {
        if let Some(entity) = self.entity_registry.remove(&id) {
            // Remove all the components assigned to the entity, swapping in the last entry into the now vacant slot
            for (comp_id, section) in entity.comp_sections.iter() {
                let mut column = self.component_registry.get_trait::<component::Column>(comp_id).write();
                column.swap_remove(*section, entity.shard_loc);
            }

            self.handle_swapped(&entity)
        }
    }

    /// Adds the supplied entity definition under the given Id.
    fn add_entity_core(&mut self, id: EntityId, mut ent_def: entity::EntityDef) {
        let entity_id_comp = self.get_component_id::<EntityId>();

        // Add the id as a mandatory extra component (if not present yet)
        if !ent_def.components.contains_key(&entity_id_comp) {
            ent_def
                .components
                .insert(entity_id_comp, entity::CompDef::Boxed(Box::new(id)));
        }

        let shard_id = self.get_shard_id(&ent_def);
        let shard = &self.shards[&shard_id];

        // Ingest all components and stash away the coordinates.
        let mut components = IndexMap::new();
        let shard_loc = ent_def
            .components
            .drain(..)
            .map(|(comp_id, comp_def)| {
                let column = &mut self.get_column(comp_id).write();
                let section = shard.get_section(comp_id);
                components.insert(comp_id, section);

                match comp_def {
                    entity::CompDef::Boxed(boxed) => column.ingest_box(boxed, section),
                    entity::CompDef::Json(json) => column.ingest_json(json, section),
                    _ => panic!("No-op component definition on a new entity"),
                }
            })
            .fold1(|acc, loc| {
                if acc != loc {
                    panic!("Divergent section locations")
                }

                loc
            })
            .unwrap();

        let entity = entity::Entity {
            id,
            shard_id: shard.id,
            shard_loc,
            comp_sections: components,
        };

        self.entity_registry.insert(entity.id, entity);
    }

    /// Removing entries from columns can result in swaps, handle these by updating the affected entity.
    fn handle_swapped(&mut self, entity: &entity::Entity) -> () {
        let entity_id_comp = self.get_component_id::<EntityId>();
        let column = self
            .component_registry
            .get::<component::ShardedColumn<EntityId>>(&entity_id_comp)
            .read();

        let entity_id_section = entity.comp_sections[&entity_id_comp];
        if column.section_len(entity_id_section) > 0 {
            // Try and get the id of the entity swapped into the slot of the removed entity. If the removed
            // entity was the tail of the array, there is nothing to swap.
            if let Some(id) = column.get(entity_id_section, entity.shard_loc) {
                let swapped_entity = self.entity_registry.get_mut(id).unwrap();
                swapped_entity.set_loc(entity.shard_loc);
            }
        } else {
            self.notify_systems_remove_shard(entity.shard_id, self.shards[&entity.shard_id].shard_key)
        }
    }

    /// Gets the id of an existing shard (or creates a new one) based on the component
    /// composition of an entity definition.
    fn get_shard_id(&mut self, ent_def: &entity::EntityDef) -> ShardId {
        let shard_key = component::composite_key(ent_def.components.keys());

        match self.shards_map.get(&shard_key) {
            Some(&id) => id,
            _ => self.create_shard(ent_def.components.keys(), shard_key),
        }
    }

    /// Create a new shard based on the supplied component combination and return it's ID.
    fn create_shard<'a>(&mut self, components: impl Iterator<Item = &'a ComponentId>, shard_key: component::ShardKey) -> ShardId {
        let sections: HashMap<_, _> = components
            .map(|&cid| (cid, self.get_column(cid).write().new_section()))
            .collect();

        let id = self.shards.len() as ShardId;
        let shard = component::Shard::new(id, shard_key, sections);
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
