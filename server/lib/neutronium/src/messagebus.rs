use crate::alloc::{DynVec, DynVecOps};
use crate::identity::{TopicBundle, TopicId};
use std::fmt::Debug;

/// Designates a struct as a topic for the message bus
pub trait Message: Clone + Debug {
    fn acquire_topic_id() -> TopicId;
    fn get_topic_id() -> TopicId;

    #[inline]
    fn get_indexer() -> usize {
        Self::get_topic_id().indexer()
    }

    #[inline]
    fn get_topic_name() -> &'static str {
        unsafe { TopicId::get_name_vec()[Self::get_indexer()] }
    }
}

/// Appendable and cloneable message queue
pub trait MessageQueue: DynVecOps {
    fn get_topic_id(&self) -> TopicId;
    fn append(&mut self, other: &mut DynVec<MessageQueue>);
    fn clone_box(&self) -> Box<MessageQueue>;
}

impl<T> MessageQueue for Vec<T>
where
    T: 'static + Message,
{
    fn get_topic_id(&self) -> TopicId {
        T::get_topic_id()
    }

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
    activity: TopicBundle,
}

impl Bus {
    #[inline]
    pub fn new() -> Bus {
        Bus {
            topics: Vec::new(),
            activity: TopicBundle::empty(),
        }
    }

    /// Register a new topic on the bus.
    #[inline]
    pub fn register<T>(&mut self)
    where
        T: 'static + Message,
    {
        if T::get_indexer() != self.topics.len() {
            panic!("Indexer mismatch - topics must be registered in lockstep with the world")
        }

        self.topics.push(DynVec::empty::<T>());
    }

    /// Restructure the current bus to match the setup of the template.
    #[inline]
    pub fn restructure(&mut self, template: &Bus) {
        self.activity = TopicBundle::empty();
        self.topics.clear();

        for dyn_vec in template.topics.iter() {
            self.topics.push(dyn_vec.clone());
        }
    }

    /// Transfer the messages in the `other` `Bus` into the current `Bus`.
    #[inline]
    pub fn transfer(&mut self, other: &mut Bus) {
        // Iter all the active topics in the other bus and move over the messages to the current.
        for topic_id in other.activity.decompose() {
            self.topics[topic_id.indexer()].append(&mut other.topics[topic_id.indexer()]);
        }
        self.activity = other.activity;

        // Clear out the activity in the other bus
        other.activity = TopicBundle::empty();
    }

    /// Read the messages for a particular topic.
    #[inline]
    pub fn read<T>(&self) -> &[T]
    where
        T: 'static + Message,
    {
        self.topics[T::get_indexer()].cast_vector::<T>()
    }

    /// Publish the supplied message on the bus.
    #[inline]
    pub fn publish<T>(&mut self, message: T)
    where
        T: 'static + Message,
    {
        self.activity += T::get_topic_id();
        self.topics[T::get_indexer()].cast_mut_vector::<T>().push(message);
    }

    /// Batch publish messages of a given type.
    #[inline]
    pub fn batch<T>(&mut self) -> Batcher<T>
    where
        T: 'static + Message,
    {
        self.activity += T::get_topic_id();
        Batcher::new(self.topics[T::get_indexer()].cast_mut_vector::<T>())
    }

    /// Clear out all the messages from this bus.
    #[inline]
    pub fn clear(&mut self) {
        for topic in self.activity.decompose() {
            self.topics[topic.indexer()].clear();
        }

        self.activity = TopicBundle::empty();
    }
}

pub struct Batcher<'a, T>
where
    T: Message,
{
    buffer: &'a mut Vec<T>,
}

impl<'a, T> Batcher<'a, T>
where
    T: Message,
{
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
    use neutronium_proc::Message;
    use std::sync::MutexGuard;

    #[derive(Message, Debug, Clone)]
    pub struct T1(i32);

    #[derive(Message, Debug, Clone)]
    pub struct T2(i32);

    fn setup() -> (TopicId, TopicId, MutexGuard<'static, ()>) {
        let lock = TopicId::static_init();

        (
            T1::acquire_topic_id(),
            T2::acquire_topic_id(),
            lock
        )
    }


    #[test]
    fn test_register_topic() {
        let _state = setup();

        let mut bus = Bus::new();

        bus.register::<T1>();
        bus.register::<T2>();

        assert_eq!(bus.topics.len(), 2);
    }

    #[test]
    #[should_panic(expected = "Indexer mismatch - topics must be registered in lockstep with the world")]
    fn test_register_fail_lockstep() {
        let _state = setup();

        let mut bus = Bus::new();

        bus.register::<T2>();
        bus.register::<T1>();
    }

    #[test]
    fn test_restructure() {
        let _state = setup();

        let mut bus1 = Bus::new();
        bus1.register::<T1>();
        bus1.publish(T1(1));
        bus1.publish(T1(2));

        let mut bus2 = Bus::new();
        bus2.register::<T1>();
        bus2.register::<T2>();

        bus1.restructure(&bus2);
        bus1.publish(T1(4));
        bus1.publish(T2(5));

        assert_eq!(bus1.topics.len(), 2);
        assert_eq!(bus1.topics[0].cast_vector::<T1>().len(), 1);
        assert_eq!(bus1.topics[0].cast_vector::<T1>()[0].0, 4);
        assert_eq!(bus1.topics[1].cast_vector::<T2>().len(), 1);
        assert_eq!(bus1.topics[1].cast_vector::<T2>()[0].0, 5);

        assert_eq!(bus2.topics[0].cast_vector::<T1>().len(), 0);
        assert_eq!(bus2.topics[1].cast_vector::<T2>().len(), 0);
    }

    #[test]
    fn test_transfer() {
        let _state = setup();

        let mut bus1 = Bus::new();
        bus1.register::<T1>();
        bus1.register::<T2>();

        let mut bus2 = Bus::new();
        bus2.register::<T1>();
        bus2.register::<T2>();

        bus1.publish(T1(0));
        bus1.publish(T1(1));
        bus1.publish(T2(1));

        bus2.transfer(&mut bus1);

        assert_eq!(bus1.topics[0].len(), 0);
        assert_eq!(bus1.topics[1].len(), 0);
        assert_eq!(bus1.activity, TopicBundle::empty());

        assert_eq!(bus2.topics[0].cast_vector::<T1>()[0].0, 0);
        assert_eq!(bus2.topics[0].cast_vector::<T1>()[1].0, 1);
        assert_eq!(bus2.topics[1].cast_vector::<T2>()[0].0, 1);
        assert_eq!(bus2.activity, T1::get_topic_id() + T2::get_topic_id());
    }

    #[test]
    fn test_read() {
        let _state = setup();

        let mut bus = Bus::new();
        bus.register::<T1>();

        bus.publish(T1(0));
        bus.publish(T1(1));
        bus.publish(T1(2));

        let messages = bus.read::<T1>();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].0, 0);
        assert_eq!(messages[1].0, 1);
        assert_eq!(messages[2].0, 2);
    }

    #[test]
    fn test_publish() {
        let _state = setup();

        let mut bus = Bus::new();
        bus.register::<T1>();

        bus.publish(T1(0));

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, T1::get_topic_id().into());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0, 0);
    }

    #[test]
    fn test_batch() {
        let _state = setup();

        let mut bus = Bus::new();
        bus.register::<T1>();

        let mut batch = bus.batch::<T1>();
        batch.publish(T1(0));
        batch.publish(T1(1));
        batch.publish(T1(2));

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, T1::get_topic_id().into());

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].0, 0);
        assert_eq!(messages[1].0, 1);
        assert_eq!(messages[2].0, 2);
    }

    #[test]
    fn test_clear() {
        let _state = setup();

        let mut bus = Bus::new();
        bus.register::<T1>();

        bus.publish(T1(0));

        bus.clear();

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, TopicBundle::empty());
        assert_eq!(messages.len(), 0);
    }
}