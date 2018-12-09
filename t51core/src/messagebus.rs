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

/*
### Messaging ###
Use Cases:
 - System <-> System
  - Can be frame lock step
 - Network <-> System
  - No lock step (messages can arrive at any time)

Required Features
 - Send Message
 - Send Message Batch (can be optimized to bulk distribute)
 - Recieve Message
 - Drain all Messages. This blocks the queue until all messages are drained into another vector. Can be used to
   absorb all outstanding messages into an internal buffer and then process them.

Option #1
Disruptor queue with pool allocated message buffer. When sending, the publisher requests a message buffer from
the pool, fills it and sends it to a destination. When the buffer is dropped, the internal vector gets returned
to the pool.

The pool needs to be a concurrent stack/queue, whichever is more performant.

Check the disruptor queue or crossbeam queues and grab the one that allows draining all messages efficiently.

Option #2
Message bus built as a MPSC queue connected to a vector of SPSC queues to broadcast the message to each consumer.

Make the message bus owned by the world and finalized when the world is built to avoid new subscriptions. There
is no need for extra synchronisation then.

Option #3
Inter system comms where systems write messages in a local buffer that then gets redistributed by the world each frame.

Has the benefit of inherently batching messages per frame and requiring no locking mechanisms at all.

Messages:
 Q: Use sequential IDs so that we can preallocate vector storages, or just use hashmaps? Former is faster for lookups
    but slower for collection (have to loop through all possible types, as opposed to only those submitted by system)

System Side:
BatchNode
 - Collects messages into DynVecs by type
Consumers
 - Tuple of pointers to vectors (make sure the vectors are boxed)
 - Pointers are extracted at initialization (like for resources)
 - At execution time, the pointers get dereffed into slices (to avoid mutation)

World Side:
 - Collect all messages from system BatchNodes into a central BatchNode before each frame
 - After systems finish processing, clear all vectors in the central BatchNode

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