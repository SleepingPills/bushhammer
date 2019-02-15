use crate::alloc::{DynVec, DynVecOps};
use crate::identity::{TopicBundle, Topic};
use std::fmt::Debug;

#[macro_export]
macro_rules! topic_init {
    ($name: ident) => {
        $crate::custom_type_id_init!($name, Topic, Message, get_topic);

        $crate::identity::paste::item! {
            #[allow(non_upper_case_globals)]
            #[allow(non_snake_case)]
            #[$crate::identity::ctor::ctor]
            fn [<_ $name _topic_init>]() {
                // Get lock
                let _lock = Topic::id_gen_lock();

                // Initialize the topic
                $name::custom_id_type_init();

                // Set up component builders
                unsafe {
                    $crate::messagebus::MSG_QUEUE_TPL.push($crate::alloc::DynVec::empty::<$name>())
                }
            }
        }
    };
}

pub static mut MSG_QUEUE_TPL: Vec<DynVec<MessageQueue>> = Vec::new();

/// Designates a struct as a topic for the message bus
pub trait Message: Clone + Debug {
    fn get_topic() -> Topic;

    #[inline]
    fn get_indexer() -> usize {
        Self::get_topic().indexer()
    }

    #[inline]
    fn get_topic_name() -> &'static str {
        unsafe { Topic::get_name_vec()[Self::get_indexer()] }
    }
}

/// Appendable and cloneable message queue
pub trait MessageQueue: DynVecOps {
    fn get_topic(&self) -> Topic;
    fn append(&mut self, other: &mut DynVec<MessageQueue>);
    fn clone_box(&self) -> Box<MessageQueue>;
}

impl<T> MessageQueue for Vec<T>
where
    T: 'static + Message,
{
    fn get_topic(&self) -> Topic {
        T::get_topic()
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
            topics: unsafe { MSG_QUEUE_TPL.clone() },
            activity: TopicBundle::empty(),
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
        self.activity += T::get_topic();
        self.topics[T::get_indexer()].cast_mut_vector::<T>().push(message);
    }

    /// Batch publish messages of a given type.
    #[inline]
    pub fn batch<T>(&mut self) -> Batcher<T>
    where
        T: 'static + Message,
    {
        self.activity += T::get_topic();
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
    use crate::topic_init;
    use super::*;

    #[derive(Debug, Clone)]
    pub struct T1(i32);

    topic_init!(T1);

    #[derive(Debug, Clone)]
    pub struct T2(i32);

    topic_init!(T2);

    #[test]
    fn test_auto_register_topics() {
        let bus = Bus::new();

        assert!(bus.topics.len() >= 2);
    }

    #[test]
    fn test_transfer() {
        let mut bus1 = Bus::new();
        let mut bus2 = Bus::new();

        bus1.publish(T1(0));
        bus1.publish(T1(1));
        bus1.publish(T2(1));

        bus2.transfer(&mut bus1);

        assert_eq!(bus1.topics[T1::get_indexer()].len(), 0);
        assert_eq!(bus1.topics[T2::get_indexer()].len(), 0);
        assert_eq!(bus1.activity, TopicBundle::empty());

        assert_eq!(bus2.topics[T1::get_indexer()].cast_vector::<T1>()[0].0, 0);
        assert_eq!(bus2.topics[T1::get_indexer()].cast_vector::<T1>()[1].0, 1);
        assert_eq!(bus2.topics[T2::get_indexer()].cast_vector::<T2>()[0].0, 1);
        assert_eq!(bus2.activity, T1::get_topic() + T2::get_topic());
    }

    #[test]
    fn test_read() {
        let mut bus = Bus::new();

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
        let mut bus = Bus::new();

        bus.publish(T1(0));

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, T1::get_topic().into());

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].0, 0);
    }

    #[test]
    fn test_batch() {
        let mut bus = Bus::new();

        let mut batch = bus.batch::<T1>();
        batch.publish(T1(0));
        batch.publish(T1(1));
        batch.publish(T1(2));

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, T1::get_topic().into());

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].0, 0);
        assert_eq!(messages[1].0, 1);
        assert_eq!(messages[2].0, 2);
    }

    #[test]
    fn test_clear() {
        let mut bus = Bus::new();

        bus.publish(T1(0));

        bus.clear();

        let messages = bus.read::<T1>();

        assert_eq!(bus.activity, TopicBundle::empty());
        assert_eq!(messages.len(), 0);
    }
}
