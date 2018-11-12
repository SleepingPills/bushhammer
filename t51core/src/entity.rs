use crate::component::ComponentCoords;
use crate::object::{ComponentId, EntityId, ShardId};
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::any::Any;
use std::any::TypeId;

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: EntityId,
    pub shard_id: ShardId,
    pub shard_loc: usize,
    pub comp_sections: IndexMap<ComponentId, usize>,
}

impl Entity {
    #[inline]
    pub(crate) fn get_coords(&self, comp_id: &ComponentId) -> ComponentCoords {
        (self.comp_sections[comp_id], self.shard_loc)
    }

    #[inline]
    pub(crate) fn set_section(&mut self, comp_id: ComponentId, section: usize) {
        self.comp_sections.insert(comp_id, section);
    }

    #[inline]
    pub(crate) fn set_loc(&mut self, loc: usize) {
        self.shard_loc = loc;
    }
}

/// Handles boxed, json and no-op components. No-op is a special case placeholder for components
/// that already exist on an entity.
#[derive(Debug)]
pub enum CompDef {
    Boxed(Box<Any>),
    Json(String),
    Nop(),
}

#[derive(Debug)]
pub struct EntityDef {
    pub components: IndexMap<ComponentId, CompDef>,
}

impl EntityDef {
    pub fn new() -> Self {
        EntityDef {
            components: IndexMap::new(),
        }
    }
}

impl From<&Entity> for EntityDef {
    fn from(entity: &Entity) -> Self {
        EntityDef {
            components: entity.comp_sections.keys().map(|cid| (*cid, CompDef::Nop())).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TransactionError {
    EntityNotFound(EntityId),
}

#[derive(Debug)]
pub enum Transaction {
    AddEnt(EntityDef),
    EditEnt(EntityId, EntityDef),
    RemoveEnt(EntityId),
}

#[derive(Debug)]
pub struct Builder<'a> {
    ent_def: EntityDef,
    component_ids: &'a HashMap<TypeId, ComponentId>,
    queue: &'a mut Vec<Transaction>,
}

impl<'a> Builder<'a> {
    #[inline]
    pub fn new(component_ids: &'a HashMap<TypeId, ComponentId>, queue: &'a mut Vec<Transaction>) -> Self {
        Builder {
            ent_def: EntityDef::new(),
            component_ids,
            queue,
        }
    }

    #[inline]
    pub fn with<T: 'static>(mut self, instance: T) -> Self {
        self.record_component(self.get_component_id::<T>(), CompDef::Boxed(Box::new(instance)));
        self
    }

    #[inline]
    pub fn with_json(mut self, type_id: ComponentId, json: String) -> Self {
        self.record_component(type_id, CompDef::Json(json));
        self
    }

    #[inline]
    pub fn build(self) {
        self.queue.push(Transaction::AddEnt(self.ent_def));
    }

    pub(crate) fn record_component(&mut self, type_id: ComponentId, def: CompDef) {
        self.ent_def.components.insert(type_id, def);
    }

    fn get_component_id<T: 'static>(&self) -> ComponentId {
        self.component_ids[&TypeId::of::<T>()]
    }
}

#[derive(Debug)]
pub struct Editor<'a> {
    id: EntityId,
    builder: Builder<'a>,
}

impl<'a> Editor<'a> {
    pub fn new(entity: &Entity, component_ids: &'a HashMap<TypeId, ComponentId>, queue: &'a mut Vec<Transaction>) -> Self {
        let builder = Builder {
            ent_def: entity.into(),
            component_ids,
            queue,
        };

        Editor { id: entity.id, builder }
    }

    #[inline]
    pub fn with<T: 'static>(mut self, instance: T) -> Self {
        let comp_id = self.builder.get_component_id::<T>();
        if comp_id == self.builder.get_component_id::<EntityId>() {
            panic!("Can't edit Entity Id component")
        }

        self.builder.record_component(comp_id, CompDef::Boxed(Box::new(instance)));
        self
    }

    #[inline]
    pub fn with_json(mut self, comp_id: ComponentId, json: String) -> Self {
        if comp_id == self.builder.get_component_id::<EntityId>() {
            panic!("Can't edit Entity Id component")
        }

        self.builder.record_component(comp_id, CompDef::Json(json));
        self
    }

    #[inline]
    pub fn remove<T: 'static>(mut self) -> Self {
        let comp_id = self.builder.get_component_id::<T>();
        if comp_id == self.builder.get_component_id::<EntityId>() {
            panic!("Can't edit Entity Id component")
        }

        self.builder.ent_def.components.remove(&comp_id);
        self
    }

    #[inline]
    pub fn remove_id(mut self, comp_id: ComponentId) -> Self {
        if comp_id == self.builder.get_component_id::<EntityId>() {
            panic!("Can't delete Entity Id component")
        }

        self.builder.ent_def.components.remove(&comp_id);
        self
    }

    #[inline]
    pub fn build(self) {
        self.builder.queue.push(Transaction::EditEnt(self.id, self.builder.ent_def));
    }
}

pub struct EntityStore<'a> {
    entity_map: &'a HashMap<EntityId, Entity>,
    component_ids: &'a HashMap<TypeId, ComponentId>,
    queue: &'a mut Vec<Transaction>,
}

impl<'a> EntityStore<'a> {
    #[inline]
    pub fn new(
        entity_map: &'a HashMap<EntityId, Entity>,
        component_ids: &'a HashMap<TypeId, ComponentId>,
        queue: &'a mut Vec<Transaction>,
    ) -> EntityStore<'a> {
        EntityStore {
            entity_map,
            component_ids,
            queue,
        }
    }

    #[inline]
    pub fn create(&mut self) -> Builder {
        Builder::new(self.component_ids, self.queue)
    }

    #[inline]
    pub fn edit(&mut self, id: EntityId) -> Result<Editor, TransactionError> {
        match self.entity_map.get(&id) {
            Some(entity) => Ok(Editor::new(entity, self.component_ids, self.queue)),
            _ => Err(TransactionError::EntityNotFound(id)),
        }
    }

    #[inline]
    pub fn remove(&mut self, id: EntityId) {
        self.queue.push(Transaction::RemoveEnt(id));
    }
}
