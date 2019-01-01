use crate::net::channel::Channel;
use crate::net::shared::Serialize;
use hashbrown::HashSet;
use std::net::{TcpStream, TcpListener};

pub type ChannelId = usize;

pub struct Endpoint {
    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    time_synch: Vec<Timing>,

    // Ids of unused channels
    slots: Vec<ChannelId>,

    frame_time: u64,

    connecting: HashSet<ChannelId>,
}

impl Endpoint {
    pub fn sync(&mut self, frame_time: u64) {
        self.frame_time = frame_time;
        // Send data on all channels until wouldblock is reached.
        // Run the connection init poll
        // Run the connected channel poll
    }

    pub fn push<S: Serialize>(&mut self, data: &S, channel_id: ChannelId) {
        // Writes the given payload to the channel and adds the id to the write_ready list.
    }

    pub fn pull(&mut self) -> impl Iterator<Item = (ChannelId, &mut Channel)> {
        self.channels
            .iter_mut()
            .filter(|channel| channel.pull_ready())
            .enumerate()
    }

    #[inline]
    pub fn new_channel(&mut self, stream: TcpStream) -> ChannelId {
        let id = match self.slots.pop() {
            Some(id) => {
                self.channels[id]
                    .open(stream)
                    .expect("Pooled channels must be closed");
                id
            }
            None => {
                let id = self.channels.len();
                self.channels
                    .push(Channel::new(stream, self.version, self.protocol));
                id
            }
        };

        // Reset the time synch of the channel
        self.time_synch[id] = Timing {
            incoming: self.frame_time,
            outgoing: self.frame_time,
        };

        id
    }

    #[inline]
    pub fn reclaim_channel(&mut self, channel_id: ChannelId) {
        self.channels[channel_id]
            .close()
            .expect("Channel must be closeable for reclamation");
        self.slots.push(channel_id);
    }
}

pub struct Timing {
    pub incoming: u64,
    pub outgoing: u64,
}
