use entity;
use entity::Entity;
use indexmap::IndexMap;
use object::{ComponentId, SystemId};
use registry::Registry;
use std::any::Any;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use sync::RwCell;
use system::System;

pub struct World {
    counter: usize,
    entities: IndexMap<usize, entity::Entity>,
    components: Registry<ComponentId>,
    systems: Registry<SystemId>,
    tx_queues: Rc<IndexMap<SystemId, RwCell<Vec<entity::Transaction>>>>,
    main_queue: Vec<entity::Transaction>,
    comp_sys: HashMap<ComponentId, HashSet<SystemId>>,
    sys_comp: HashMap<SystemId, HashSet<ComponentId>>,
}

impl World {
    pub fn run_systems(&mut self) {
        let systems = self.systems.iter_mut::<System>();

        // TODO: Turn this into a parallelized SEDA execution
        for (id, mut sys) in systems {
            if let Some(tx_queue) = self.tx_queues.get(id) {
                let mut tx = tx_queue.write();
                sys.run(entity::EntityStore::new(&self.entities, &self.comp_sys, &self.sys_comp, &mut tx))
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
                _ => break
            }
        }
    }

    fn apply_transaction(&mut self, tx: entity::Transaction) {
        match tx {
            entity::Transaction::AddEnt(steps) => {},
            entity::Transaction::EditEnt(steps) => {},
            entity::Transaction::RemoveEnt(id) => {
                let entity = self.entities.swap_remove(&id);
                //TODO: Remove from systems and components
            }
        }
    }

    pub fn add_entity(&mut self) -> entity::Builder {
        entity::Builder::new(&self.entities, &self.comp_sys, &self.sys_comp, &mut self.main_queue)
    }

    pub fn edit_entity(&mut self, id: usize) -> Result<entity::Editor, entity::TransactionError> {
        entity::Editor::new(id, &self.entities, &self.comp_sys, &self.sys_comp, &mut self.main_queue)
    }

    pub fn remove_entity(&mut self, id: usize) {
        self.main_queue.push(entity::Transaction::RemoveEnt(id));
    }
}

impl World {
    pub fn register_component<T>(&mut self, id: ComponentId) {
        /*
        Creates an instance of a componentstore and registers it in the registry
        */
        unimplemented!()
    }

    pub fn register_system<T>(&mut self, id: SystemId) {
        /*
        Creates an instance of system.
        Require: Default and System
        
        trait System {
            fn new(component_registry)
            fn init(&mut self, world)
            fn required_components(&self) -> &[ComponentIds]
        }
        */
        unimplemented!()
    }
}
