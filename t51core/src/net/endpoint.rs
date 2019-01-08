use crate::net::channel::Channel;
use crate::net::frame::{ControlFrame, Frame};
use crate::net::shared::{Deserialize, NetworkError, NetworkResult, PayloadBatch, Serialize, UserId};
use indexmap::IndexSet;
use std::net::{TcpListener, TcpStream};
use std::time;

pub type ChannelId = usize;

#[derive(Debug, Copy, Clone)]
enum ConnectionChange {
    Connected(UserId, ChannelId),
    Disconnected(ChannelId),
}

pub struct Endpoint {
    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    // Ids of unused channels
    free_slots: Vec<ChannelId>,
    connected: IndexSet<ChannelId>,

    changes: Vec<ConnectionChange>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,
}

impl Endpoint {
    #[inline]
    pub fn push<P: Serialize>(&mut self, channel_id: ChannelId, data: &mut PayloadBatch<P>) {
        let channel = &mut self.channels[channel_id];

        channel.write_payload(data).unwrap_or_else(|err| {
            if let NetworkError::Fatal(_) = err {
                panic!("Fatal error during write")
            }
        });
        channel.send(self.current_time).unwrap_or_else(|err| {
            if let NetworkError::Fatal(_) = err {
                self.disconnect(channel_id, false);
            }
        });
    }

    pub fn pull<P: Deserialize>(&mut self, channel_id: ChannelId, data: &mut PayloadBatch<P>) {
        let channel = &mut self.channels[channel_id];

        channel
            .read()
            .and_then(|frame| match frame {
                Frame::Control(ctr) => {
                    match ctr {
                        ControlFrame::ConnectionClosed(_) => channel.close(false),
                        _ => ()
                    };
                    Ok(())
                }
                Frame::Payload(pinfo) => {
                    channel.read_payload(data, pinfo)
                }
            })
            .unwrap_or_else(|err| {
                if let NetworkError::Fatal(_) = err {
                    self.disconnect(channel_id, true);
                }
            });
    }

    pub fn sync(&mut self, now: time::Instant) {
//        self.channels[0].send(now).unwrap_or_else(|err| {
//            if let NetworkError::Fatal(_) = err {
//                self.disconnect(0, false);
//            }
//        });

        self.current_time = now;
    }

    #[inline]
    fn disconnect(&mut self, channel_id: ChannelId, notify: bool) {
        self.channels[channel_id].close(notify);
        self.changes.push(ConnectionChange::Disconnected(channel_id));
        self.connected.remove(&channel_id);
    }
}

//impl Endpoint {
//    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(5);
//    const TIMEOUT: time::Duration = time::Duration::from_secs(30);
//
////    pub fn push<P: Serialize>(&mut self, data: &mut PayloadBatch<P>, channel_id: ChannelId) -> Result<()> {
////        // Write the data
////        self.channels[channel_id].write_batch(data)?;
////        Ok(())
////    }
//
////    pub fn pull(&mut self, channel_id: ChannelId) -> Option<Frame<&[u8]>> {
////        unimplemented!()
////    }
//
//    pub fn sync(&mut self, current_time: time::Instant) {
//        self.current_time = current_time;
//
//        if current_time.duration_since(self.housekeeping_time) >= Self::HOUSEKEEPING_INTERVAL {
//            // Check if handshakes timed out
//            // Check if connections timed out
//            // Send keepalives
//            self.housekeeping_time = current_time;
//        }
//        // Send data on all channels until wouldblock is reached.
//        // Run the connection init poll
//        // Run the connected channel poll
//    }
//
//    #[inline]
//    pub fn new_channel(&mut self, stream: TcpStream) -> ChannelId {
//        let id = match self.free_slots.pop() {
//            Some(id) => {
//                self.channels[id].open(stream);
//                id
//            }
//            None => {
//                let id = self.channels.len();
//                self.channels
//                    .push(Channel::new(stream, self.version, self.protocol));
//                id
//            }
//        };
//
//        id
//    }
//
//    //    #[inline]
//    //    pub fn reclaim_channel(&mut self, channel_id: ChannelId) {
//    //        self.channels[channel_id]
//    //            .close()
//    //            .expect("Channel must be closeable for reclamation");
//    //        self.free_slots.push(channel_id);
//    //    }
//}
