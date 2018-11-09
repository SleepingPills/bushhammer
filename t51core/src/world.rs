use crate::entity;
use crate::object::{ComponentId, EntityId, SystemId};
use crate::registry::Registry;
use crate::sync::RwCell;
use crate::system::System;
use crate::system::SystemEntry;
use crate::system::SystemRuntime;
use hashbrown::HashMap;
use hashbrown::HashSet;
use indexmap::IndexMap;
use std::sync::Arc;

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: IndexMap<SystemId, Box<SystemRuntime>>,
    transactions: Vec<entity::Transaction>,
}

impl World {
    #[inline]
    pub fn entities(&mut self) -> entity::EntityStore {
        entity::EntityStore::new(&self.entity_registry, &mut self.transactions)
    }

    #[inline]
    pub fn create_runtime<T>(&self, system: T) -> SystemEntry<T>
    where
        T: System,
    {
        SystemEntry::new(system, &self.component_registry)
    }
}

/*
impl World {
    pub fn new() -> World {
        World {
            entities: SlotPool::new(),
            components: Registry::new(),
            systems: IndexMap::new(),
            tx_queues: Arc::new(IndexMap::new()),
            main_queue: Vec::new(),
            comp_sys: HashMap::new(),
            sys_comp: HashMap::new(),
        }
    }
}

impl World {
    pub fn run_systems(&mut self) {
        for (id, cell) in self.systems.iter() {
            let mut sys = cell.write();
            if let Some(tx_queue) = self.tx_queues.get(id) {
                let mut tx = tx_queue.write();
                sys.run(entity::EntityStore::new(
                    &self.entities,
                    &self.comp_sys,
                    &self.sys_comp,
                    &mut tx,
                ))
            } else {
                panic!("System {} not found", id)
            }
        }
    }

    pub fn apply_transactions(&mut self) {
        for etx in self.tx_queues.clone().values() {
            let mut tx_queue = etx.write();
            for tx in tx_queue.drain(..) {
                self.apply_transaction(tx);
            }
        }
        for _ in 0..self.main_queue.len() {
            match self.main_queue.pop() {
                Some(tx) => self.apply_transaction(tx),
                _ => break,
            }
        }
    }

    fn apply_transaction(&mut self, tx: entity::Transaction) {
        match tx {
            entity::Transaction::AddEnt(steps) => {
                let id = self.create_entity_instance();

                for step in steps.steps {
                    self.apply_step(id, step);
                }
            }
            entity::Transaction::EditEnt(id, steps) => {
                for step in steps.steps {
                    self.apply_step(id, step)
                }
            }
            entity::Transaction::RemoveEnt(id) => {
                if let Some(entity) = self.entities.reclaim(id) {
                    for sys_id in entity.systems.iter() {
                        let mut system = self.systems[sys_id].write();
                        system.remove_entity(entity.id)
                    }
                    for (comp_id, index) in entity.components.iter() {
                        let mut comp_mgr = self
                            .components
                            .try_get_trait::<ComponentManager>(comp_id)
                            .expect("Component manager not found")
                            .write();
                        comp_mgr.reclaim(*index);
                    }
                }
            }
        }
    }

    fn apply_step(&mut self, id: EntityId, step: entity::Step) {
        if let Some(entity) = self.entities.get_mut(id) {
            match step {
                entity::Step::AddComp((comp_id, ptr)) => {
                    let mut comp_manager = self.components.get_trait::<ComponentManager>(&comp_id).write();
                    let index = comp_manager.add_component(comp_id, ptr);
                    entity.components.insert(comp_id, index);
                }
                entity::Step::AddCompJson((comp_id, json)) => {
                    // TODO: Change so that we notify the system when the entity changes bundles.
                    let mut comp_manager = self.components.get_trait::<ComponentManager>(&comp_id).write();
                    let index = comp_manager.add_component_json(comp_id, json);
                    entity.add_component(comp_id, index);
                }
                entity::Step::AddSys(sys_id) => {
                    for comp_id in &self.sys_comp[&sys_id] {
                        if !entity.components.contains_key(&comp_id) {
                            panic!(
                                "Can't add system {} to entity {}, requiredcomponent {} missing",
                                sys_id, entity.id, comp_id
                            );
                        }
                    }

                    let mut system = self.systems[&sys_id].write();
                    system.add_entity(entity);
                    entity.add_system(sys_id);
                }
                entity::Step::RemoveComp(comp_id) => {
                    // TODO: Change so that we notify the system when the entity changes bundles.
                    // Panic in case the component to be removed is required by a system
                    for sys_id in &self.comp_sys[&comp_id] {
                        if entity.systems.contains(&sys_id) {
                            panic!(
                                "Can't remove component {} for entity {}, system {} depends on it",
                                comp_id, entity.id, sys_id
                            );
                        }
                    }

                    if let Some(comp_index) = entity.remove_component(comp_id) {
                        let mut comp_manager = self.components.get_trait::<ComponentManager>(&comp_id).write();
                        comp_manager.reclaim(comp_index);
                    }
                }
                entity::Step::RemoveSys(sys_id) => {
                    if entity.remove_system(sys_id) {
                        let mut system = self.systems[&sys_id].write();
                        system.remove_entity(entity.id);
                    }
                }
            }
        }
    }

    #[inline]
    fn create_entity_instance(&mut self) -> EntityId {
        let id = self.entities.peek_index();
        self.entities.push(entity::Entity::new(id))
    }
}


impl World {
    #[allow(unused_variables)]
    pub fn register_component<T>(&mut self, id: ComponentId) {
        /*
        Creates an instance of a componentstore and registers it in the registry
        */
unimplemented!()
}

#[allow(unused_variables)]
pub fn register_system<T>(&mut self, id: SystemId) {
let sys_id = SystemId::new::<T>();

// Build the system and run the init callback
let mut system = T::new(&self.components);
system.init(&self.components, &self.systems);

// Register the system and core trait
self.systems.register(sys_id, system);
self.systems.register_trait::<T, ManagedSystem>(&sys_id);

// Add system dependencies
let required_components = T::required_components();

for &component_id in required_components.iter() {
let entry = self.comp_sys.entry(component_id).or_insert_with(HashSet::new);
entry.insert(sys_id);
}

self.sys_comp.insert(sys_id, HashSet::from_iter(required_components));
}
}
*/
