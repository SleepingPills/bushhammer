use indexmap::IndexMap;
use object::{ComponentId, SystemId};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use sync::ReadGuard;
use sync::RwGuard;

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: usize,
    pub components: HashMap<ComponentId, usize>,
    pub systems: HashSet<SystemId>,
}

impl Entity {
    pub(crate) fn new(id: usize) -> Entity {
        Entity {
            id,
            components: HashMap::new(),
            systems: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TransactionError {
    ComponentMissing(String),
    EntityMissing(String),
    ComponentRequired(String),
}

pub enum Step {
    AddSys(SystemId),
    RemoveSys(SystemId),
    AddComp((ComponentId, *const ())),
    RemoveComp(ComponentId),
}

pub struct Composite {
    pub(crate) steps: Vec<Step>,
}

pub enum Transaction {
    AddEnt(Composite),
    EditEnt(Composite),
    RemoveEnt(usize),
}

pub struct Builder<'a> {
    components: HashMap<ComponentId, *const ()>,
    systems: HashSet<SystemId>,
    entities: &'a IndexMap<usize, Entity>,
    sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
    comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
    tx: &'a mut Vec<Transaction>,
}

impl<'a> Builder<'a> {
    pub fn new(
        entities: &'a IndexMap<usize, Entity>,
        sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
        comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
        tx: &'a mut Vec<Transaction>,
    ) -> Self {
        Builder {
            components: HashMap::new(),
            systems: HashSet::new(),
            entities,
            sys_comp,
            comp_sys,
            tx,
        }
    }
}

pub struct EntityStore<'a> {
    entities: &'a IndexMap<usize, Entity>,
    sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
    comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
    tx: &'a mut RwGuard<Vec<Transaction>>,
}

impl<'a> EntityStore<'a> {
    pub fn new(
        entities: &'a IndexMap<usize, Entity>,
        sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
        comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
        tx: &'a mut RwGuard<Vec<Transaction>>,
    ) -> Self {
        EntityStore {
            entities,
            sys_comp,
            comp_sys,
            tx,
        }
    }
}

impl<'a> EntityStore<'a> {
    pub fn add(&mut self) -> Builder {
        Builder::new(self.entities, self.sys_comp, self.comp_sys, self.tx)
    }

    pub fn edit(&mut self, id: usize) -> Builder {
        Builder::new(self.entities, self.sys_comp, self.comp_sys, self.tx)
    }

    pub fn remove(&mut self, id: usize) {
        self.tx.push(Transaction::RemoveEnt(id));
    }
}

/*
pub struct EntityBuilder<'a> {
    entity: Rc<RefCell<Entity>>,
    world: &'a mut World,
}

impl<'a> EntityBuilder<'a> {
    pub fn add_component<T: 'static>(self, component: T) -> EntityBuilder<'a> {
        let comp_id = self.world.store_component(component);
        self.entity.borrow_mut().components.insert(self.world.get_comp_id::<T>(), comp_id);
        self
    }

    pub fn add_component_type(self, comp_id: ComponentId, component: Box<Any>) -> EntityBuilder<'a> {
        let comp_idx = self.world.store_component_type(comp_id, component);
        self.entity.borrow_mut().components.insert(comp_id, comp_idx);
        self
    }

    pub fn add_component_json(self, comp_id: ComponentId, json: String) -> EntityBuilder<'a> {
        let comp_idx = self.world.store_component_json(comp_id, json);
        self.entity.borrow_mut().components.insert(comp_id, comp_idx);
        self
    }

    pub fn remove_component<T: 'static>(self) -> Result<EntityBuilder<'a>, SystemError> {
        {
            let component_id = self.world.get_comp_id::<T>();
            self.world.remove_component_from_entity(component_id, &self.entity.borrow())?;
        }
        Ok(self)
    }

    pub fn remove_component_type(self, comp_id: ComponentId) -> Result<EntityBuilder<'a>, SystemError> {
        {
            self.world.remove_component_from_entity(comp_id, &self.entity.borrow())?;
        }
        Ok(self)
    }
}

impl<'a> EntityBuilder<'a> {
    pub fn add_system<T: 'static>(self) -> Result<EntityBuilder<'a>, SystemError> {
        let sys_id = self.world.get_sys_id::<T>();
        {
            let mut entity = self.entity.borrow_mut();
            self.world.add_entity_to_system(sys_id, &entity)?;
            entity.systems.insert(sys_id);
        }
        Ok(self)
    }

    pub fn add_system_type(self, sys_id: SystemId) -> Result<EntityBuilder<'a>, SystemError> {
        {
            let mut entity = self.entity.borrow_mut();
            self.world.add_entity_to_system(sys_id, &entity)?;
            entity.systems.insert(sys_id);
        }
        Ok(self)
    }

    pub fn remove_system<T: 'static>(self) -> Result<EntityBuilder<'a>, SystemError> {
        let sys_id = self.world.get_sys_id::<T>();
        {
            let mut entity = self.entity.borrow_mut();
            self.world.remove_entity_from_system(sys_id, entity.id)?;
            entity.systems.remove(&sys_id);
        }
        Ok(self)
    }

    pub fn remove_system_type(self, sys_id: SystemId) -> Result<EntityBuilder<'a>, SystemError> {
        {
            let mut entity = self.entity.borrow_mut();
            self.world.remove_entity_from_system(sys_id, entity.id)?;
            entity.systems.remove(&sys_id);
        }
        Ok(self)
    }
}
*/
