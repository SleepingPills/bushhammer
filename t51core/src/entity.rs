use crate::component::ComponentCoords;
use crate::object::{ShardId, ComponentId, EntityId};
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::any::Any;

/// Entity root object. Maintains a registry of components and indices, along with the systems
/// it is registerered with.
#[derive(Debug)]
pub struct Entity {
    pub id: EntityId,
    pub shard_id: ShardId,
    pub components: HashMap<ComponentId, ComponentCoords>,
}

impl Entity {
    #[inline]
    pub(crate) fn get_coords(&self, comp_id: &ComponentId) -> ComponentCoords {
        self.components[comp_id]
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
            components: entity.components.keys().map(|cid| (*cid, CompDef::Nop())).collect(),
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
    queue: &'a mut Vec<Transaction>,
}

impl<'a> Builder<'a> {
    #[inline]
    pub fn new(queue: &'a mut Vec<Transaction>) -> Self {
        Builder {
            ent_def: EntityDef::new(),
            queue,
        }
    }

    #[inline]
    pub fn with<T: 'static>(mut self, instance: T) -> Self {
        self.record_component(ComponentId::new::<T>(), CompDef::Boxed(Box::new(instance)));
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
}

#[derive(Debug)]
pub struct Editor<'a> {
    id: EntityId,
    builder: Builder<'a>,
}

impl<'a> Editor<'a> {
    pub fn new(entity: &Entity, queue: &'a mut Vec<Transaction>) -> Self {
        let builder = Builder {
            ent_def: entity.into(),
            queue,
        };

        Editor { id: entity.id, builder }
    }

    #[inline]
    pub fn with<T: 'static>(mut self, instance: T) -> Self {
        self.builder
            .record_component(ComponentId::new::<T>(), CompDef::Boxed(Box::new(instance)));
        self
    }

    #[inline]
    pub fn with_json(mut self, comp_id: ComponentId, json: String) -> Self {
        self.builder.record_component(comp_id, CompDef::Json(json));
        self
    }

    #[inline]
    pub fn remove<T: 'static>(mut self) -> Self {
        self.builder.ent_def.components.remove(&ComponentId::new::<T>());
        self
    }

    #[inline]
    pub fn remove_id(mut self, comp_id: ComponentId) -> Self {
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
    queue: &'a mut Vec<Transaction>,
}

impl<'a> EntityStore<'a> {
    #[inline]
    pub fn new(entity_map: &'a HashMap<EntityId, Entity>, queue: &'a mut Vec<Transaction>) -> EntityStore<'a> {
        EntityStore { entity_map, queue }
    }

    #[inline]
    pub fn create(&mut self) -> Builder {
        Builder::new(self.queue)
    }

    #[inline]
    pub fn edit(&mut self, id: EntityId) -> Result<Editor, TransactionError> {
        match self.entity_map.get(&id) {
            Some(entity) => Ok(Editor::new(entity, self.queue)),
            _ => Err(TransactionError::EntityNotFound(id)),
        }
    }

    #[inline]
    pub fn remove(&mut self, id: usize) {
        self.queue.push(Transaction::RemoveEnt(id));
    }
}
