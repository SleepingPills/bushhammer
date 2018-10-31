use crate::component::ComponentManager;
use crate::entity;
use crate::object::{ComponentId, SystemId};
use crate::registry::Registry;
use crate::sync::RwCell;
use crate::system::{BuildableSystem, ManagedSystem};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc;

pub struct World {
    id_counter: usize,
    // TODO: Replace this with a variety of the VecPool that uses Option<T> for items and can therefore
    // accurately represent holes and missing items. It can also swap out the content of the option when
    // reclaiming, so all existing functionality can be mimiced neatly.
    entities: IndexMap<entity::EntityId, entity::Entity>,
    components: Registry<ComponentId>,
    systems: Registry<SystemId>,
    tx_queues: Arc<IndexMap<SystemId, RwCell<Vec<entity::Transaction>>>>,
    main_queue: Vec<entity::Transaction>,
    comp_sys: HashMap<ComponentId, HashSet<SystemId>>,
    sys_comp: HashMap<SystemId, HashSet<ComponentId>>,
}

impl World {
    pub fn new() -> World {
        World {
            id_counter: 0,
            entities: IndexMap::new(),
            components: Registry::new(),
            systems: Registry::new(),
            tx_queues: Arc::new(IndexMap::new()),
            main_queue: Vec::new(),
            comp_sys: HashMap::new(),
            sys_comp: HashMap::new(),
        }
    }
}

impl World {
    pub fn run_systems(&mut self) {
        let systems = self.systems.iter_mut::<ManagedSystem>();

        // TODO: Turn this into a parallelized SEDA execution
        for (id, mut sys) in systems {
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
                let id = self.create_entity();

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
                if let Some(entity) = self.entities.swap_remove(&id) {
                    for sys_id in entity.systems.iter() {
                        let mut system = self.systems.get_trait::<ManagedSystem>(sys_id).write();
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

    fn apply_step(&mut self, id: entity::EntityId, step: entity::Step) {
        if let Some(entity) = self.entities.get_mut(&id) {
            match step {
                entity::Step::AddComp((comp_id, ptr)) => {
                    let mut comp_manager = self.components.get_trait::<ComponentManager>(&comp_id).write();
                    let index = comp_manager.add_component(comp_id, ptr);
                    entity.components.insert(comp_id, index);
                }
                entity::Step::AddCompJson((comp_id, json)) => {
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

                    let mut system = self.systems.get_trait::<ManagedSystem>(&sys_id).write();
                    system.add_entity(entity);
                    entity.add_system(sys_id);
                }
                entity::Step::RemoveComp(comp_id) => {
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
                        let mut system = self.systems.get_trait::<ManagedSystem>(&sys_id).write();
                        system.remove_entity(entity.id);
                    }
                }
            }
        }
    }

    #[inline]
    fn create_entity(&mut self) -> entity::EntityId {
        let id = self.id_counter;
        self.id_counter += 1;
        self.entities.insert(id, entity::Entity::new(id));
        id
    }
}

impl World {
    #[inline]
    pub fn add_entity(&mut self) -> entity::Builder {
        entity::Builder::new(&self.comp_sys, &self.sys_comp, &mut self.main_queue)
    }

    pub fn edit_entity(&mut self, id: usize) -> Result<entity::Editor, entity::TransactionError> {
        match self.entities.get(&id) {
            Some(entity) => Ok(entity::Editor::new(
                entity,
                &self.comp_sys,
                &self.sys_comp,
                &mut self.main_queue,
            )),
            _ => Err(entity::TransactionError::EntityNotFound(id)),
        }
    }

    #[inline]
    pub fn remove_entity(&mut self, id: usize) {
        self.main_queue.push(entity::Transaction::RemoveEnt(id));
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
    pub fn register_system<T>(&mut self, id: SystemId)
    where
        T: 'static + BuildableSystem,
    {
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
