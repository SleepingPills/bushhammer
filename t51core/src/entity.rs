use crate::object::{ComponentId, SystemId, EntityId};
use crate::sync::RwGuard;
use crate::alloc::SlotPool;
use std::collections::{HashMap, HashSet};

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: EntityId,
    pub components: HashMap<ComponentId, usize>,
    pub systems: HashSet<SystemId>,
}

impl Entity {
    #[inline]
    pub(crate) fn new(id: EntityId) -> Entity {
        Entity {
            id,
            components: HashMap::new(),
            systems: HashSet::new(),
        }
    }

    #[inline]
    pub(crate) fn add_component(&mut self, id: ComponentId, index: usize) {
        self.components.insert(id, index);
    }

    #[inline]
    pub(crate) fn add_system(&mut self, id: SystemId) {
        self.systems.insert(id);
    }

    #[inline]
    pub(crate) fn remove_component(&mut self, id: ComponentId) -> Option<usize> {
        self.components.remove(&id)
    }

    #[inline]
    pub(crate) fn remove_system(&mut self, id: SystemId) -> bool{
        self.systems.remove(&id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TransactionError {
    ComponentMissing(Vec<ComponentId>),
    EntityNotFound(EntityId),
    ComponentRequired(SystemId, ComponentId),
    ComponentNotFound(ComponentId),
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
    EditEnt(EntityId, Composite),
    RemoveEnt(EntityId),
}

#[derive(Debug)]
pub struct Builder<'a> {
    tx: Composite,
    components: HashSet<ComponentId>,
    systems: HashSet<SystemId>,
    comp_sys: &'a HashMap<ComponentId, HashSet<SystemId>>,
    sys_comp: &'a HashMap<SystemId, HashSet<ComponentId>>,
    queue: &'a mut Vec<Transaction>,
}

impl<'a> Builder<'a> {
    pub fn new(
        comp_sys: &'a HashMap<ComponentId, HashSet<SystemId>>,
        sys_comp: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut Vec<Transaction>,
    ) -> Self {
        Builder {
            tx: Composite { steps: Vec::new() },
            components: HashSet::new(),
            systems: HashSet::new(),
            comp_sys,
            sys_comp,
            queue,
        }
    }

    #[inline]
    pub fn add_system<T: 'static>(self) -> Result<Builder<'a>, TransactionError> {
        self.add_system_type(SystemId::new::<T>())
    }

    pub fn add_system_type(mut self, sys_id: SystemId) -> Result<Builder<'a>, TransactionError> {
        if let Some(missing) = self.core_check_missing_comp(sys_id) {
            Err(TransactionError::ComponentMissing(missing))
        } else {
            self.core_record_sys_step(Step::AddSys(sys_id), sys_id);
            Ok(self)
        }
    }

    #[inline]
    pub fn add_component<T: 'static>(mut self, instance: T) -> Builder<'a> {
        self.core_add_component(instance);
        self
    }

    #[inline]
    pub fn add_component_json(mut self, comp_id: ComponentId, json: String) -> Builder<'a> {
        self.core_record_comp_step(Step::AddCompJson((comp_id, json)), comp_id);
        self
    }

    fn core_add_component<T: 'static>(&mut self, instance: T) {
        // Stash away the pointer as a void type and leak the original box.
        // The type will be reinstated later by the component store.
        let ptr = Box::into_raw(Box::new(instance)) as *const ();
        let comp_id = ComponentId::new::<T>();
        let step = Step::AddComp((comp_id, ptr));
        self.core_record_comp_step(step, comp_id);
    }

    #[inline]
    fn core_record_comp_step(&mut self, step: Step, comp_id: ComponentId) {
        self.tx.steps.push(step);
        self.components.insert(comp_id);
    }

    #[inline]
    fn core_record_sys_step(&mut self, step: Step, sys_id: SystemId) {
        self.tx.steps.push(step);
        self.systems.insert(sys_id);
    }

    fn core_check_missing_comp(&self, sys_id: SystemId) -> Option<Vec<ComponentId>> {
        let requirements = self.sys_comp.get(&sys_id).expect(&format!("System {} not found", sys_id));
        let missing: Vec<_> = requirements
            .iter()
            .filter_map(|cid| if !self.components.contains(&cid) { Some(*cid) } else { None })
            .collect();

        match missing.len() {
            0 => None,
            _ => Some(missing),
        }
    }

    #[inline]
    pub fn commit(self) {
        self.queue.push(Transaction::AddEnt(self.tx));
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
        entity: &'a Entity,
        comp_sys: &'a HashMap<ComponentId, HashSet<SystemId>>,
        sys_comp: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut Vec<Transaction>,
    ) -> Self {
        let builder = Builder::new(comp_sys, sys_comp, queue);
        Editor { entity, builder }
    }

    #[inline]
    pub fn add_system<T: 'static>(self) -> Result<Editor<'a>, TransactionError> {
        self.add_system_type(SystemId::new::<T>())
    }

    pub fn add_system_type(mut self, sys_id: SystemId) -> Result<Editor<'a>, TransactionError> {
        if let Some(missing) = self.builder.core_check_missing_comp(sys_id) {
            if missing.iter().all(|cid| self.entity.components.contains_key(cid)) {
                self.builder.core_record_sys_step(Step::AddSys(sys_id), sys_id);
                Ok(self)
            } else {
                Err(TransactionError::ComponentMissing(missing))
            }
        } else {
            self.builder.core_record_sys_step(Step::AddSys(sys_id), sys_id);
            Ok(self)
        }
    }

    #[inline]
    pub fn remove_system<T: 'static>(self) -> Result<Editor<'a>, TransactionError> {
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

    #[inline]
    pub fn add_component<T: 'static>(mut self, instance: T) -> Editor<'a> {
        self.builder.core_add_component(instance);
        self
    }

    #[inline]
    pub fn add_component_json(mut self, comp_id: ComponentId, json: String) -> Editor<'a> {
        self.builder
            .core_record_comp_step(Step::AddCompJson((comp_id, json)), comp_id);
        self
    }

    #[inline]
    pub fn remove_component<T: 'static>(self) -> Result<Editor<'a>, TransactionError> {
        self.remove_component_type(ComponentId::new::<T>())
    }

    pub fn remove_component_type(mut self, comp_id: ComponentId) -> Result<Editor<'a>, TransactionError> {
        if let Some(systems) = self.builder.comp_sys.get(&comp_id) {
            for sys_id in systems {
                if self.entity.systems.contains(sys_id) || self.builder.systems.contains(sys_id) {
                    return Err(TransactionError::ComponentRequired(*sys_id, comp_id));
                }
            }
            self.builder.tx.steps.push(Step::RemoveComp(comp_id));
            Ok(self)
        } else {
            Err(TransactionError::ComponentNotFound(comp_id))
        }
    }

    #[inline]
    pub fn commit(self) {
        self.builder.queue.push(Transaction::EditEnt(self.entity.id, self.builder.tx));
    }
}

pub struct EntityStore<'a> {
    entities: &'a SlotPool<Entity>,
    comp_sys: &'a HashMap<ComponentId, HashSet<SystemId>>,
    sys_comp: &'a HashMap<SystemId, HashSet<ComponentId>>,
    queue: &'a mut RwGuard<Vec<Transaction>>,
}

impl<'a> EntityStore<'a> {
    #[inline]
    pub fn new(
        entities: &'a SlotPool<Entity>,
        comp_sys: &'a HashMap<ComponentId, HashSet<SystemId>>,
        sys_comp: &'a HashMap<SystemId, HashSet<ComponentId>>,
        queue: &'a mut RwGuard<Vec<Transaction>>,
    ) -> Self {
        EntityStore {
            entities,
            comp_sys,
            sys_comp,
            queue,
        }
    }
}

impl<'a> EntityStore<'a> {
    #[inline]
    pub fn new(&mut self) -> Builder {
        Builder::new(self.comp_sys, self.sys_comp, self.queue)
    }

    #[inline]
    pub fn edit(&mut self, id: usize) -> Result<Editor, TransactionError> {
        match self.entities.get(id) {
            Some(entity) => Ok(Editor::new(
                entity,
                self.comp_sys,
                self.sys_comp,
                self.queue,
            )),
            _ => Err(TransactionError::EntityNotFound(id)),
        }
    }

    #[inline]
    pub fn remove(&mut self, id: usize) {
        self.queue.push(Transaction::RemoveEnt(id));
    }
}
