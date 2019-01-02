use crate::net::channel::{Channel, Connected, AwaitToken};
use crate::net::result::Result;
use crate::net::shared::{Serialize, ClientId};
use hashbrown::HashSet;
use std::net::{TcpListener, TcpStream};
use std::time;
use crate::net::frame::Frame;

pub type ChannelId = usize;

pub struct Endpoint {
    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    timestamps: Vec<Timing>,

    // Connection handshake flag
    connecting: Vec<bool>,

    // Ids of unused channels
    free_slots: Vec<ChannelId>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,

    // List of newly connected clients
    handshakes: Vec<(ClientId, ChannelId)>
}

impl Endpoint {
    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(5);
    const TIMEOUT: time::Duration = time::Duration::from_secs(30);

    pub fn push<S: Serialize>(&mut self, data: S, channel_id: ChannelId) -> Result<()> {
        self.channels[channel_id].write(Frame::Payload(data))?;
        // Update the outgoing timestamp for the channel
        self.timestamps[channel_id].outgoing = self.current_time;
        Ok(())
    }

    pub fn pull(&mut self) -> impl Iterator<Item = (ChannelId, &mut Channel)> {
        self.channels
            .iter_mut()
            .filter(|channel| channel.pull_ready())
            .enumerate()
    }

    pub fn sync(&mut self, current_time: time::Instant) {
        self.current_time = current_time;

        if current_time.duration_since(self.housekeeping_time) >= Self::HOUSEKEEPING_INTERVAL {
            // Check if handshakes timed out
            // Check if connections timed out
            // Send keepalives
            self.housekeeping_time = current_time;
        }
        // Send data on all channels until wouldblock is reached.
        // Run the connection init poll
        // Run the connected channel poll
    }

    /// Drains all outstanding handshakes
    pub fn drain_handshakes(&mut self) -> impl Iterator<Item = (ClientId, ChannelId)> + '_ {
        self.handshakes.drain(..)
    }

    #[inline]
    pub fn new_channel(&mut self, stream: TcpStream) -> ChannelId {
        let id = match self.free_slots.pop() {
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
        self.timestamps[id] = Timing {
            incoming: self.current_time,
            outgoing: self.current_time,
        };

        id
    }

    #[inline]
    pub fn reclaim_channel(&mut self, channel_id: ChannelId) {
        self.channels[channel_id]
            .close()
            .expect("Channel must be closeable for reclamation");
        self.free_slots.push(channel_id);
    }
}

pub struct Timing {
    pub incoming: time::Instant,
    pub outgoing: time::Instant,
}
