use crate::net::channel::{Handshake, Channel, Connected};
use crate::net::frame::{Frame, NoPayload};
use crate::net::result::{Result, Error};
use crate::net::shared::{Serialize, UserId};
use std::net::{TcpListener, TcpStream};
use std::time;
use crate::net::shared::PayloadBatch;

pub type ChannelId = usize;

#[derive(Debug, Copy, Clone)]
enum ChannelState {
    Handshake {
        created: time::Instant,
    },
    Connected {
        last_ingress: time::Instant,
        last_egress: time::Instant,
        user_id: UserId,
    },
    Disconnected,
}

#[derive(Debug, Copy, Clone)]
enum ConnectionChange {
    Connected(UserId, ChannelId),
    Disconnected(ChannelId)
}

/*
- pull(channel_id) - pulls a message from the given channel. Returns Ok(frame) if there is one, or None if there
  isn't. Internally, the endpoint will disconnect the channel if the error was anything other than Error::Wait and
  add an entry to the channel disconnect list.
- push(data, channel_id) - puts a message on the given channel. Returns true if the message was accepted for
  transmission. Serialization is triggered by the channel, so even though the serializable object would check upfront
  whether there is enough space to contain it, the Error:Wait will be propagated through the channel. If the result
  is false, the message was not accepted (either due to an error or because the buffer is full).
  If there is any other error than Error::Wait, the channel will be disconnected and put on the disconnect list.
- sync() - Carries out the actual transmissions. Any errors (apart from Error:Wait) result in disconnection.
  Calls the housekeeping function periodically.
- housekeeping() - Go through each channel and depending on it's state:
  Handshake - checks if the timeout elapsed, if yes, disconnect.
  Connected - check if the comms timeout elapsed, if yes, disconnect. Check if any comms happened since the last
              housekeeping round, and if not, plop a keepalive message on the channel.
- disconnect() - Attempts to put a disconnect message on the channel and send it immediately. Irrespective of that
  succeeding, it closes the channel.
- send_disconnect() -> Result<()> - Creates a disconnect message, puts it in the buffer and flushes the channel.
- changes() -> ConnectionChange: Iterates through a vector containing ConnectionChange enums. These reflect all the
  connections/disconnections that happened on the Endpoint so that they can be exactly replicated into the world state.
*/
pub struct Endpoint {
    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    channel_states: Vec<ChannelState>,

    // Ids of unused channels
    free_slots: Vec<ChannelId>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,
}

impl Endpoint {
    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(5);
    const TIMEOUT: time::Duration = time::Duration::from_secs(30);

    pub fn push<P: Serialize>(&mut self, data: &mut PayloadBatch<P>, channel_id: ChannelId) -> Result<()> {
        // Update the outgoing timestamp for the channel
        match self.channel_states[channel_id] {
            ChannelState::Connected { ref mut last_egress, .. } => *last_egress = self.current_time,
            _ => panic!("Attempting to write to an unconnected channel"),
        }
        // Write the data
        self.channels[channel_id].write_batch(data)?;
        Ok(())
    }

    pub fn pull(&mut self, channel_id: ChannelId) -> Option<Frame<&[u8]>> {
        {
            match self.channels[channel_id].read() {
                Ok(frame) => return Some(frame),
                Err(Error::Wait) => return None,
                Err(_) => ()
            }
        }

        self.disconnect(channel_id);

        None
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
                self.channel_states[id] = ChannelState::Handshake {
                    created: self.current_time,
                };
                id
            }
            None => {
                let id = self.channels.len();
                self.channels
                    .push(Channel::new(stream, self.version, self.protocol));
                self.channel_states.push(ChannelState::Handshake {
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

    #[inline]
    pub fn disconnect(&mut self, channel_id: ChannelId) {
        let channel = &mut self.channels[channel_id];

        channel.clear();

        // Attempt to send a disconnect message if the channel is connected
        if let ChannelState::Connected {user_id, ..} = self.channel_states[channel_id] {
            drop(channel.write::<NoPayload>(Frame::ConnectionClosed(user_id)));
            drop(channel.send());
        }

        self.channel_states[channel_id] = ChannelState::Disconnected;

        // Close the channel
        channel.close().expect("Channel shutdown successful");
    }
}