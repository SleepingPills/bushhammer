use crate::component;
use crate::entity;
use crate::object::{ComponentId, EntityId, ShardId, SystemId};
use crate::registry::Registry;
use crate::registry::TraitBox;
use crate::sentinel;
use crate::sync::RwCell;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use std::any::TypeId;
use std::sync::Arc;

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: Registry<SystemId>,
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
            system_registry: Registry::new(),
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

        for (_, mut system) in self.system_registry.iter_mut::<system::SystemRuntime>() {
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

        self.transactions.put(transactions);
    }

    /// Add a new entity to the world.
    fn apply_add(&mut self, ent_def: entity::EntityDef) {
        self.add_entity_core(self.next_entity_id(), ent_def);
    }

    /// Edit an existing entity.
    fn apply_edit(&mut self, id: EntityId, mut ent_def: entity::EntityDef) {
        if let Some(ent) = self.entity_registry.remove(&id) {
            // If the entity composition didn't change, just update as necessary
            if component::composite_key(ent.comp_sections.keys()) == component::composite_key(ent_def.components.keys()) {
                let shard = &self.shards[&ent.shard_id];

                for (comp_id, comp_def) in ent_def.components.drain(..) {
                    let mut column = self.get_column(comp_id).write();
                    let section = shard.get_section(comp_id);

                    if let entity::CompDef::Boxed(boxed) = comp_def {
                        column.update_box(boxed, section, ent.shard_loc);
                    } else if let entity::CompDef::Json(json) = comp_def {
                        column.update_json(json, section, ent.shard_loc);
                    }
                }

                self.entity_registry.insert(id, ent);
            } else {
                // Remove all the components from the entity, stashing away those that need to be transferred
                // due to Nops on the new definition
                let mut transfer = Vec::new();
                for (&comp_id, &section) in ent.comp_sections.iter() {
                    self.get_column(comp_id)
                        .apply_mut(|col| match ent_def.components.get(&comp_id) {
                            Some(entity::CompDef::Nop()) => {
                                let boxed = col.swap_remove_return(section, ent.shard_loc);
                                transfer.push((comp_id, entity::CompDef::Boxed(boxed)))
                            }
                            _ => col.swap_remove(section, ent.shard_loc),
                        })
                }
                ent_def.components.extend(transfer);

                // Handle the swapped entity
                self.handle_swapped(&ent);

                // Add the new definition under the old id
                self.add_entity_core(id, ent_def);
            }
        }
    }

    /// Remove an existing entity from the world.
    fn apply_remove(&mut self, id: EntityId) {
        if let Some(ent) = self.entity_registry.remove(&id) {
            // Remove all the components assigned to the entity, swapping in the last entry into the now vacant slot
            for (&comp_id, section) in ent.comp_sections.iter() {
                self.get_column(comp_id)
                    .apply_mut(|col| col.swap_remove(*section, ent.shard_loc));
            }

            self.handle_swapped(&ent)
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

        let shard_key = component::composite_key(ent_def.components.keys());
        let shard_id = self.get_shard_id(shard_key, &ent_def);
        let shard = &self.shards[&shard_id];

        // Ingest all components and stash away the coordinates.
        let mut components = IndexMap::new();

        let shard_loc = ent_def
            .components
            .drain(..)
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

        // In case the entity is at position 0, the shard was empty before and has to be added to relevant systems.
        if shard_loc == 0 {
            // Notify each relevant system that a shard was added
            self.system_registry
                .iter_mut::<system::SystemRuntime>()
                .filter(|(_, sys)| sys.check_shard(shard_key))
                .for_each(|(_, mut sys)| sys.add_shard(shard));
        }

        let ent = entity::Entity {
            id,
            shard_id,
            shard_loc,
            comp_sections: components,
        };

        self.entity_registry.insert(ent.id, ent);
    }

    /// Removing entries from columns can result in swaps, handle these by updating the affected entity.
    fn handle_swapped(&mut self, ent: &entity::Entity) -> () {
        let entity_id_comp = self.get_component_id::<EntityId>();
        self.component_registry
            .get::<component::ShardedColumn<EntityId>>(&entity_id_comp)
            .apply(|col| {
                let entity_id_section = ent.comp_sections[&entity_id_comp];
                if col.section_len(entity_id_section) > 0 {
                    // Try and get the id of the entity swapped into the slot of the removed entity. If the removed
                    // entity was the tail of the array, there is nothing to swap.
                    if let Some(id) = col.get(entity_id_section, ent.shard_loc) {
                        let swapped_entity = self.entity_registry.get_mut(id).unwrap();
                        swapped_entity.set_loc(ent.shard_loc);
                    }
                } else {
                    // Notify each relevant system that a shard was added
                    self.system_registry
                        .iter_mut::<system::SystemRuntime>()
                        .filter(|(_, sys)| sys.check_shard(self.shards[&ent.shard_id].shard_key))
                        .for_each(|(_, mut sys)| sys.remove_shard(ent.shard_id));
                }
            });
    }

    /// Gets the id of an existing shard (or creates a new one) based on the component
    /// composition of an entity definition.
    fn get_shard_id(&mut self, shard_key: component::ShardKey, ent_def: &entity::EntityDef) -> ShardId {
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
        self.shards.insert(id, shard);
        self.shards_map.insert(shard_key, id);
        id
    }

    /// Get the next entity id.
    #[inline]
    fn next_entity_id(&self) -> EntityId {
        return self.entity_registry.len() as EntityId;
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
    pub fn register_system<T>(&mut self, system: T) -> SystemId
    where
        T: 'static + system::System,
    {
        let id = SystemId::new::<T>(self.system_registry.len());
        let runtime = self.create_runtime(system);
        self.system_registry.register(id, runtime);
        self.system_registry
            .register_trait::<system::SystemEntry<T>, system::SystemRuntime>(&id);
        self.system_ids.insert(TypeId::of::<T>(), id);
        id
    }

    /// Process all currently registered systems.
    #[inline]
    pub fn process_systems(&mut self) {
        for (_, mut system) in self.system_registry.iter_mut::<system::SystemRuntime>() {
            system.run(&self.entity_registry, &self.component_ids);
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn get_system<T>(&self, id: SystemId) -> Arc<RwCell<system::SystemEntry<T>>>
    where
        T: 'static + system::System,
    {
        self.system_registry.get::<system::SystemEntry<T>>(&id)
    }
}

impl World {
    /// Register the supplied component type.
    pub fn register_component<T>(&mut self)
    where
        T: 'static + DeserializeOwned,
    {
        let id = ComponentId::new::<T>(self.component_registry.len());
        let store = component::ShardedColumn::<T>::new();

        self.component_registry.register(id, store);
        self.component_registry
            .register_trait::<component::ShardedColumn<T>, component::Column>(&id);
        self.component_ids.insert(TypeId::of::<T>(), id);
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

#[cfg(test)]
mod tests {
    use super::entity;
    use crate::prelude::*;
    use serde_derive::{Deserialize, Serialize};
    use std::marker::PhantomData;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    struct SomeComponent {
        x: i32,
        y: i32,
    }

    #[test]
    fn test_add_entity() {
        let mut world = World::new();

        world.register_component::<SomeComponent>();
        world.register_component::<i32>();
        world.register_component::<f32>();

        world.entities().create().with(SomeComponent { x: 1, y: 1 }).with(1).build();
        world.entities().create().with(SomeComponent { x: 2, y: 2 }).with(2).build();
        world
            .entities()
            .create()
            .with(SomeComponent { x: 3, y: 3 })
            .with(4)
            .with(5f32)
            .build();

        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);
    }

    #[test]
    fn test_edit_entity() {
        let mut world = World::new();

        world.register_component::<SomeComponent>();
        world.register_component::<i32>();
        world.register_component::<f32>();

        world.entities().create().with(SomeComponent { x: 1, y: 1 }).with(1).build();
        world.entities().create().with(SomeComponent { x: 2, y: 2 }).with(2).build();
        world.entities().create().with(SomeComponent { x: 3, y: 3 }).with(3).build();

        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 1);

        // Add a new component to an existing entity, resulting in a new shard
        world.entities().edit(1).expect("Entity must exist").with(5f32).commit();
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);

        // Move an additional entity to the new shard
        world.entities().edit(0).expect("Entity must exist").with(5f32).commit();
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);

        // Edit entity in-place, not resulting in a new shard
        world
            .entities()
            .edit(2)
            .expect("Entity must exist")
            .with(SomeComponent { x: 1, y: 1 })
            .commit();
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);
    }

    #[test]
    fn test_remove_entity() {
        let mut world = World::new();

        world.register_component::<SomeComponent>();
        world.register_component::<i32>();

        world.entities().create().with(1).build();
        world.entities().create().with(SomeComponent { x: 1, y: 1 }).with(2).build();
        world.entities().create().with(SomeComponent { x: 2, y: 2 }).with(3).build();

        world.process_transactions();

        assert_eq!(world.component_registry.len(), 3);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);

        // Test removing entity in the middle
        world.entities().remove(1);
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 3);
        assert_eq!(world.entity_registry.len(), 2);
        assert_eq!(world.shards.len(), 2);

        // Test removing all entities
        world.entities().remove(0);
        world.entities().remove(2);
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 3);
        assert_eq!(world.entity_registry.len(), 0);
        assert_eq!(world.shards.len(), 2);
    }

    #[test]
    fn test_ingest_system_transactions() {
        // Create a system that edits an existing entity and added two new ones
        struct TestSystem<'a> {
            _p: PhantomData<&'a ()>,
        }

        impl<'a> System for TestSystem<'a> {
            require!(Read<'a, EntityId>, Read<'a, i32>, Write<'a, SomeComponent>);

            fn run(&mut self, _ctx: Context<Self::JoinItem>, mut entities: entity::EntityStore) {
                entities.edit(0).expect("Entity must exist").with(5f32).commit();
                entities.create().with(SomeComponent { x: 2, y: 2 }).with(2).build();
                entities.create().with(SomeComponent { x: 3, y: 3 }).with(3).build();
            }
        }

        let mut world = World::new();

        world.register_component::<SomeComponent>();
        world.register_component::<i32>();
        world.register_component::<f32>();

        world.register_system(TestSystem { _p: PhantomData });

        // Add a single entity initially and ensure the state is correct
        world.entities().create().with(SomeComponent { x: 1, y: 1 }).with(1).build();
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 1);
        assert_eq!(world.shards.len(), 1);

        // Run the system, triggering the edit and two additions
        world.run();
        world.process_transactions();

        assert_eq!(world.component_registry.len(), 4);
        assert_eq!(world.entity_registry.len(), 3);
        assert_eq!(world.shards.len(), 2);
    }

    #[test]
    fn test_run_systems() {
        struct TestSystem<'a> {
            collector: Vec<(EntityId, i32, SomeComponent)>,
            _p: PhantomData<&'a ()>,
        }

        impl<'a> System for TestSystem<'a> {
            require!(Read<'a, EntityId>, Read<'a, i32>, Write<'a, SomeComponent>);

            fn run(&mut self, mut ctx: Context<Self::JoinItem>, _entities: entity::EntityStore) {
                for (&id, &a, b) in ctx.iter() {
                    self.collector.push((id, a, b.clone()));
                }
            }
        }

        let mut world = World::new();

        // Base scenario
        world.register_component::<SomeComponent>();
        world.register_component::<i32>();
        world.register_component::<f32>();

        let system_id = world.register_system(TestSystem {
            collector: Vec::new(),
            _p: PhantomData,
        });
        let system = world.get_system::<TestSystem>(system_id);

        world
            .entities()
            .create()
            .with(SomeComponent { x: 0, y: 0 })
            .with(0i32)
            .build();
        world
            .entities()
            .create()
            .with(SomeComponent { x: 1, y: 1 })
            .with(1i32)
            .build();
        world
            .entities()
            .create()
            .with(SomeComponent { x: 2, y: 2 })
            .with(2i32)
            .build();
        world
            .entities()
            .create()
            .with(SomeComponent { x: 3, y: 3 })
            .with(3i32)
            .with(5f32)
            .build();

        // Run the system
        world.run();

        // Check state
        let mut state: Vec<_> = system.write().get_system_mut().collector.drain(..).collect();

        assert_eq!(state.len(), 4);
        assert_eq!(state[0], (0, 0, SomeComponent { x: 0, y: 0 }));
        assert_eq!(state[1], (1, 1, SomeComponent { x: 1, y: 1 }));
        assert_eq!(state[2], (2, 2, SomeComponent { x: 2, y: 2 }));
        assert_eq!(state[3], (3, 3, SomeComponent { x: 3, y: 3 }));
        state.clear();

        // Remove the entity that was in it's own shard
        world.entities().remove(3);

        // Run the system
        world.run();

        // Ensure removed component is not in the results
        let mut state: Vec<_> = system.write().get_system_mut().collector.drain(..).collect();

        assert_eq!(state.len(), 3);
        assert_eq!(state[0], (0, 0, SomeComponent { x: 0, y: 0 }));
        assert_eq!(state[1], (1, 1, SomeComponent { x: 1, y: 1 }));
        assert_eq!(state[2], (2, 2, SomeComponent { x: 2, y: 2 }));
        state.clear();

        // Edit entity, requiring a remove/add
        world
            .entities()
            .edit(1)
            .expect("Entity must exist")
            .with(5f32)
            .with(5i32)
            .commit();

        // Run the system
        world.run();

        // Ensure edited entity is in the results set
        let state: Vec<_> = system.write().get_system_mut().collector.drain(..).collect();

        assert_eq!(state.len(), 3);
        assert_eq!(state[0], (0, 0, SomeComponent { x: 0, y: 0 }));
        assert_eq!(state[1], (2, 2, SomeComponent { x: 2, y: 2 }));
        assert_eq!(state[2], (1, 5, SomeComponent { x: 1, y: 1 }));
    }
}
