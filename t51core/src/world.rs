use crate::component;
use crate::entity;
use crate::object::{ShardId, ComponentId, EntityId, SystemId};
use crate::registry::Registry;
use crate::system;
use hashbrown::HashMap;
use indexmap::IndexMap;

pub struct World {
    component_registry: Registry<ComponentId>,
    entity_registry: HashMap<EntityId, entity::Entity>,
    system_registry: IndexMap<SystemId, Box<system::SystemRuntime>>,
    shards: HashMap<ShardId, component::Shard>,
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
    /// Drains all the system transactions into the common transaction queue
    fn collect_transactions(&mut self) {
        let transactions = self.transactions.as_mut().unwrap();

        for (_, system) in self.system_registry.iter_mut() {
            transactions.append(system.get_transactions());
        }
    }

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

    fn apply_add(&mut self, ent_def: entity::EntityDef) {

    }

    fn apply_edit(&mut self, id: EntityId, ent_def: entity::EntityDef) {

    }

    fn apply_remove(&mut self, id: EntityId) {

    }
}

impl World {
    #[inline]
    pub fn create_runtime<T>(&self, system: T) -> system::SystemEntry<T>
    where
        T: system::System,
    {
        system::SystemEntry::new(system, &self.component_registry)
    }

    pub fn register_system<T>(&mut self, system: T)
    where
        T: 'static + system::System,
    {
        let id = SystemId::new::<T>();
        let runtime = self.create_runtime(system);

        self.system_registry.insert(id, Box::new(runtime));
    }

    pub fn process_systems(&mut self) {
        unimplemented!()
    }
}

impl World {
    pub fn register_component<T>(&mut self)
    where
        T: 'static,
    {
        let id = ComponentId::new::<T>();
        let store = component::ShardedColumn::<T>::new();

        self.component_registry.register(id, store);
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
*/
