use crate::alloc::{DynVec, DynVecOps};
use crate::identity::{TopicId, TopicBundle};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

/// Designates a struct as a topic for the message bus
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

/// Appendable and cloneable message queue
pub trait MessageQueue: DynVecOps {
    fn append(&mut self, other: &mut DynVec<MessageQueue>);
    fn clone_box(&self) -> Box<MessageQueue>;
}

impl<T> MessageQueue for Vec<T>
where
    T: 'static + Topic,
{
    #[inline]
    fn append(&mut self, other: &mut DynVec<MessageQueue>) {
        let other_vec = other.cast_mut_vector::<T>();
        self.append(other_vec);
    }

    #[inline]
    fn clone_box(&self) -> Box<MessageQueue> {
        Box::new(Vec::<T>::new())
    }
}

impl Clone for DynVec<MessageQueue> {
    #[inline]
    fn clone(&self) -> Self {
        DynVec::from_box(self.clone_box())
    }
}

/// A message bus based on a directly indexable registry of queues
#[derive(Clone)]
pub struct Bus {
    topics: Vec<DynVec<MessageQueue>>,
    activity: TopicBundle
}

impl Bus {


    #[inline]
    pub fn register<T>(&mut self)
    where
        T: 'static + Topic,
    {
        if T::get_indexer() != self.topics.len() {
            panic!("Indexer mismatch - topics must be registered in lockstep with the world")
        }

        self.topics.push(DynVec::empty::<T>());
    }

    /// Transfer the messages in the `other` `Bus` into the current `Bus`.
    #[inline]
    pub fn transfer(&mut self, other: &mut Bus) {
        // Iter all the active topics in the other bus and move over the messages to the current.
        for topic in other.activity.decompose() {
            self.activity += topic;
            self.topics[topic.id as usize].append(&mut other.topics[topic.id as usize]);
        }
        // Clear out the activity in the other bus
        other.activity = TopicBundle::empty();
    }

    /// Read the messages for a particular topic
    #[inline]
    pub fn read<T>(&self) -> &[T]
    where
        T: 'static + Topic,
    {
        self.topics[T::get_indexer()].cast_vector::<T>()
    }

    /// Publish the supplied message on the bus.
    #[inline]
    pub fn publish<T>(&mut self, message: T)
    where
        T: 'static + Topic,
    {
        self.activity += T::get_topic_id();
        self.topics[T::get_indexer()].cast_mut_vector::<T>().push(message);
    }

    /// Batch publish messages of a given type.
    #[inline]
    pub fn batch<T>(&mut self) -> Batcher<T>
    where
        T: 'static + Topic,
    {
        self.activity += T::get_topic_id();
        Batcher::new(self.topics[T::get_indexer()].cast_mut_vector::<T>())
    }

    #[inline]
    pub fn clear(&mut self) {
        for topic in self.activity.decompose() {
            self.topics[topic.id as usize].clear();
        }

        self.activity = TopicBundle::empty();
    }
}

pub struct Batcher<'a, T> where T: Topic{
    buffer: &'a mut Vec<T>,
}

impl<'a, T> Batcher<'a, T> where T: Topic {
    #[inline]
    fn new(buffer: &'a mut Vec<T>) -> Batcher<'a, T> {
        Batcher { buffer }
    }

    /// Publish the supplied message on the bus.
    #[inline]
    pub fn publish(&mut self, message: T) {
        self.buffer.push(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_register_topic() {

    }

    fn test_register_fail_lockstep() {

    }

    fn test_transfer() {

    }

    fn test_read() {

    }

    fn test_publish() {

    }

    fn test_batch() {

    }

    fn test_clear() {

    }
}

/*
### Messaging ###
Inter system comms where systems write messages in a local buffer that then gets redistributed by the world each frame.

Has the benefit of inherently batching messages per frame and requiring no locking mechanisms at all.

Messages:
 Q: Use sequential IDs so that we can preallocate vector storages, or just use hashmaps? Former is faster for lookups
    but slower for collection (have to loop through all possible types, as opposed to only those submitted by system)

System Side:
 - Collect all messages into local bus (passed as mutable ref)
 - Read from common bus (passed as immutable ref)

World Side:
 - Pass in common bus to systems for frame processing
 - Frame processing
 - Clear out common bus
 - Transfer all system bus contents to common bus

The above setup gets us completely allocation free broadcast.

Requires special system that handles networking... but this could be a benefit. There'd be a networking/authentication
 and authorization system, which recieves messages into a queue and then broadcasts them for other systems.

The final architecture is thus as follows:

|-      Network Thread    -|                   |-            World             -|
(network) <-> Network Manager <-> (crossbeam) <-> Networking System -> (message bus)

* Network Manager *
 - Maintains connections
 - Ensures basic message protocol adherence

* Networking System *
 - Authentication (read/save)
 - Authorization (read/save)
 - Submission to message bus
 - Recieve messages by admin system about permission changes and bans

Q: Where do we deserialize messages?
Q: Can we avoid allocating for messages - e.g. use a pool for each type?
*/
