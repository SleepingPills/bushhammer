use crate::net::channel::{Handshake, Channel, Connected};
use crate::net::frame::Frame;
use crate::net::result::Result;
use crate::net::shared::{Serialize, UserId};
use hashbrown::HashSet;
use std::net::{TcpListener, TcpStream};
use std::time;

pub type ChannelId = usize;

enum ConnectionState {
    Handshake {
        created: time::Instant,
    },
    Connected {
        last_ingress: time::Instant,
        last_egress: time::Instant,
        user_id: UserId,
    },
}

pub struct Endpoint {
    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    channel_states: Vec<ConnectionState>,

    // Ids of unused channels
    free_slots: Vec<ChannelId>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,
}

impl Endpoint {
    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(5);
    const TIMEOUT: time::Duration = time::Duration::from_secs(30);

    pub fn push<S: Serialize>(&mut self, data: S, channel_id: ChannelId) -> Result<()> {
        // Update the outgoing timestamp for the channel
        match self.channel_states[channel_id] {
            ConnectionState::Connected { ref mut last_egress, .. } => *last_egress = self.current_time,
            _ => panic!("Attempting to write to an unconnected channel"),
        }
        // Write the data
        self.channels[channel_id].write(Frame::Payload(data))?;
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

    #[inline]
    pub fn new_channel(&mut self, stream: TcpStream) -> ChannelId {
        let id = match self.free_slots.pop() {
            Some(id) => {
                self.channels[id]
                    .open(stream)
                    .expect("Pooled channels must be closed");
                self.channel_states[id] = ConnectionState::Handshake {
                    created: self.current_time,
                };
                id
            }
            None => {
                let id = self.channels.len();
                self.channels
                    .push(Channel::new(stream, self.version, self.protocol));
                self.channel_states.push(ConnectionState::Handshake {
                    created: self.current_time,
                });
                id
            }
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
