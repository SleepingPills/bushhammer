use crate::identity::TopicId;
use crate::alloc::{DynVecOps, DynVec};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub trait Topic: DeserializeOwned + Clone + Debug {
    fn acquire_topic_id() -> TopicId;
    fn get_topic_id() -> TopicId;

    #[inline]
    fn get_indexer() -> usize {
        Self::get_topic_id().id as usize
    }

    #[inline]
    fn get_topic_name() -> &'static str {
        unsafe { TopicId::get_name_vec()[Self::get_indexer()] }
    }
}

pub trait MessageBuffer : DynVecOps {
    fn append(&mut self, other: &mut DynVec<MessageBuffer>);
}

impl<T> MessageBuffer for Vec<T>
where
    T: 'static + Topic,
{
    fn append(&mut self, other: &mut DynVec<MessageBuffer>) {
        let other_vec = other.cast_mut_vector::<T>();
        self.append(other_vec);
    }
}

pub struct Bus {
    topics: Vec<DynVec<MessageBuffer>>,
}


impl Bus {
}