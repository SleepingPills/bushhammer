use indexmap::IndexMap;
use object::{ComponentId, SystemId};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::mem;
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TransactionError {
    ComponentMissing(ComponentId),
    EntityNotFound(usize),
    ComponentRequired(SystemId, ComponentId),
    SystemNotFound(SystemId),
}

#[derive(Debug)]
pub enum Step {
    AddSys(SystemId),
    RemoveSys(SystemId),
    AddComp((ComponentId, *const ())),
    AddCompJson((ComponentId, String)),
    RemoveComp(ComponentId),
}

#[derive(Debug)]
pub struct Composite {
    pub(crate) steps: Vec<Step>,
}

#[derive(Debug)]
pub enum Transaction {
    AddEnt(Composite),
    EditEnt(Composite),
    RemoveEnt(usize),
}

#[derive(Debug)]
pub struct Builder<'a> {
    tx: Composite,
    components: HashSet<ComponentId>,
    systems: HashSet<SystemId>,
    entities: &'a IndexMap<usize, Entity>,
    sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
    comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
    queue: &'a mut Vec<Transaction>,
}

impl<'a> Builder<'a> {
    pub fn new(
        entities: &'a IndexMap<usize, Entity>,
        sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
        comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut Vec<Transaction>,
    ) -> Self {
        Builder {
            tx: Composite { steps: Vec::new() },
            components: HashSet::new(),
            systems: HashSet::new(),
            entities,
            sys_comp,
            comp_sys,
            queue,
        }
    }

    pub fn add_system<T: 'static>(mut self) -> Result<Builder<'a>, TransactionError> {
        self.add_system_type(SystemId::new::<T>())
    }

    pub fn add_system_type(mut self, sys_id: SystemId) -> Result<Builder<'a>, TransactionError> {
        let requirements = self.comp_sys.get(&sys_id).expect("System not found");
        for cid in requirements {
            if !self.components.contains(&cid) {
                return Err(TransactionError::ComponentMissing(cid.clone()))
            }
        }
        self.tx.steps.push(Step::AddSys(sys_id));
        Ok(self)
    }

    pub fn add_component<T: 'static>(mut self, instance: T) -> Builder<'a> {
        let step = Builder::build_add_comp_step(instance);
        self.tx.steps.push(step);
        self
    }

    pub fn add_component_json(mut self, comp_id: ComponentId, json: String) -> Builder<'a> {
        self.tx.steps.push(Step::AddCompJson((comp_id, json)));
        self
    }

    fn build_add_comp_step<T: 'static>(instance: T) -> Step {
        // Stash away the pointer as a void type and leak the original box.
        // The type will be reinstated later by the component store.
        let ptr = Box::into_raw(Box::new(instance)) as *const ();
        Step::AddComp((ComponentId::new::<T>(), ptr))
    }
}

#[derive(Debug)]
pub struct Editor<'a> {
    entity: &'a Entity,
    builder: Builder<'a>,
}

impl<'a> Editor<'a> {
    #[inline]
    pub fn new(
        id: usize,
        entities: &'a IndexMap<usize, Entity>,
        sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
        comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut Vec<Transaction>,
    ) -> Result<Self, TransactionError> {
        if let Some(entity) = entities.get(&id) {
            let builder = Builder::new(entities, sys_comp, comp_sys, queue);
            Ok(Editor { entity, builder })
        } else {
            Err(TransactionError::EntityNotFound(id))
        }
    }

    pub fn add_system<T: 'static>(mut self) -> Result<Editor<'a>, TransactionError> {
        self.add_system_type(SystemId::new::<T>())
    }

    pub fn add_system_type(mut self, sys_id: SystemId) -> Result<Editor<'a>, TransactionError> {
        unimplemented!()
    }

    pub fn remove_system<T: 'static>(mut self) -> Result<Editor<'a>, TransactionError> {
        self.remove_system_type(SystemId::new::<T>())
    }

    pub fn remove_system_type(mut self, sys_id: SystemId) -> Result<Editor<'a>, TransactionError> {
        if self.entity.systems.contains(&sys_id) {
            self.builder.tx.steps.push(Step::RemoveSys(sys_id));
            Ok(self)
        } else {
            Err(TransactionError::SystemNotFound(sys_id))
        }
    }

    pub fn add_component<T: 'static>(mut self, instance: T) -> Editor<'a> {
        let step = Builder::build_add_comp_step(instance);
        self.builder.tx.steps.push(step);
        self
    }

    pub fn add_component_json(mut self, comp_id: ComponentId, json: String) -> Editor<'a> {
        self.builder.tx.steps.push(Step::AddCompJson((comp_id, json)));
        self
    }

    pub fn remove_component<T: 'static>(mut self) -> Result<Editor<'a>, TransactionError> {
        self.remove_component_type(ComponentId::new::<T>())
    }

    pub fn remove_component_type(mut self, comp_id: ComponentId) -> Result<Editor<'a>, TransactionError> {
        // Check all systems that require this component, and ensure none of them are registered
        // for this entity.
        unimplemented!()
    }
}

pub struct EntityStore<'a> {
    entities: &'a IndexMap<usize, Entity>,
    sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
    comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
    queue: &'a mut RwGuard<Vec<Transaction>>,
}

impl<'a> EntityStore<'a> {
    pub fn new(
        entities: &'a IndexMap<usize, Entity>,
        sys_comp: &'a HashMap<ComponentId, HashSet<SystemId>>,
        comp_sys: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut RwGuard<Vec<Transaction>>,
    ) -> Self {
        EntityStore {
            entities,
            sys_comp,
            comp_sys,
            queue,
        }
    }
}

impl<'a> EntityStore<'a> {
    pub fn add(&mut self) -> Builder {
        Builder::new(self.entities, self.sys_comp, self.comp_sys, self.queue)
    }

    pub fn edit(&mut self, id: usize) -> Result<Editor, TransactionError> {
        Editor::new(id, self.entities, self.sys_comp, self.comp_sys, self.queue)
    }

    pub fn remove(&mut self, id: usize) {
        self.queue.push(Transaction::RemoveEnt(id));
    }
}
