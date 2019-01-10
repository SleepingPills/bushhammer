use crate::net::channel::{Channel, ChannelState};
use crate::net::frame::{ControlFrame, Frame};
use crate::net::shared;
use crate::net::shared::ErrorUtils;
use indexmap::IndexSet;
use mio;
use mio::net::{TcpListener, TcpStream};
use std::io;
use std::net::SocketAddr;
use std::time;

pub type ChannelId = usize;

#[derive(Debug, Copy, Clone)]
pub enum ConnectionChange {
    Connected(shared::UserId, ChannelId),
    Disconnected(ChannelId),
}

pub struct Endpoint {
    server: TcpListener,

    server_poll: mio::Poll,
    handshake_poll: mio::Poll,
    live_poll: mio::Poll,

    events: mio::Events,

    secret_key: [u8; 32],

    // Validation
    version: [u8; 16],
    protocol: u16,

    // Storage
    channels: Vec<Channel>,
    // Ids of unused channels
    free: Vec<ChannelId>,
    live: IndexSet<ChannelId>,

    changes: Vec<ConnectionChange>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,
}

impl Endpoint {
    const HANDSHAKE_TIMEOUT: time::Duration = time::Duration::from_secs(5);
    const INGRESS_TIMEOUT: time::Duration = time::Duration::from_secs(30);
    const KEEPALIVE_INTERVAL: time::Duration = time::Duration::from_secs(3);
    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(3);
    const SERVER_POLL_TOKEN: mio::Token = mio::Token(0);

    #[inline]
    pub fn new(
        address: &str,
        secret_key: [u8; 32],
        version: [u8; 16],
        protocol: u16,
    ) -> shared::NetworkResult<Endpoint> {
        let server_poll = mio::Poll::new()?;
        let server = TcpListener::bind(&address.parse::<SocketAddr>()?)?;

        server_poll.register(
            &server,
            Self::SERVER_POLL_TOKEN,
            mio::Ready::readable(),
            mio::PollOpt::edge(),
        )?;

        let now = time::Instant::now();

        Ok(Endpoint {
            server,
            server_poll,
            handshake_poll: mio::Poll::new()?,
            live_poll: mio::Poll::new()?,
            events: mio::Events::with_capacity(8192),
            secret_key,
            version,
            protocol,
            channels: Vec::new(),
            free: Vec::new(),
            live: IndexSet::new(),
            changes: Vec::new(),
            current_time: now,
            housekeeping_time: now,
        })
    }

    #[inline]
    pub fn push<P: shared::Serialize>(&mut self, channel_id: ChannelId, data: &mut shared::PayloadBatch<P>) {
        let channel = &mut self.channels[channel_id];

        if channel.write_payload(data).has_failed() {
            panic!("Fatal write error");
        }
    }

    pub fn pull<P: shared::Deserialize>(
        &mut self,
        channel_id: ChannelId,
        data: &mut shared::PayloadBatch<P>,
    ) {
        let mut ctx = self.get_comm_ctx(channel_id);

        match ctx.channel.read() {
            Ok(frame) => {
                match frame {
                    Frame::Control(ctr) => {
                        match ctr {
                            // Disconnect notice sent by client. Close channel but don't send notice back.
                            ControlFrame::ConnectionClosed(_) => ctx.disconnect(false),
                            // Connection accepted sent by client in error, close channel and notify.
                            ControlFrame::ConnectionAccepted(_) => ctx.disconnect(true),
                            // Keepalive requests are ignored at this stage.
                            ControlFrame::Keepalive(_) => (),
                        };
                    }
                    Frame::Payload(pinfo) => {
                        if ctx.channel.read_payload(data, pinfo).has_failed() {
                            ctx.disconnect(true)
                        }
                    }
                }
            }
            Err(shared::NetworkError::Fatal(_)) => ctx.disconnect(true),
            _ => (),
        }
    }

    pub fn sync(&mut self, now: time::Instant) {
        self.current_time = now;

        if now.duration_since(self.housekeeping_time) >= Self::HOUSEKEEPING_INTERVAL {
            self.housekeeping();
            self.housekeeping_time = now;
        }

        let live_set = &mut self.live;
        let free_set = &mut self.free;
        let channels = &mut self.channels;
        let changes = &mut self.changes;

        // Force send data on all live channels
        live_set.retain(|&channel_id| {
            let channel = &mut channels[channel_id];

            let retain = match channel.has_egress() {
                true => !channel.send(now).has_failed(),
                _ => true,
            };

            // Close the channel in case of a send error. No point in trying to send a notice.
            if !retain {
                channel.close(false);
                free_set.push(channel_id);
                changes.push(ConnectionChange::Disconnected(channel_id));
            }

            retain
        });

        // Run listen poll
        self.server_poll
            .poll(&mut self.events, Some(time::Duration::from_secs(0)))
            .expect("Listen poll failed");

        for event in &self.events {
            if event.readiness().is_readable() {
                match self.server.accept() {
                    Ok((stream, _)) => {
                        let id = match self.free.pop() {
                            Some(id) => id,
                            None => {
                                let id = self.channels.len();
                                self.channels.push(Channel::new(self.version, self.protocol));
                                id
                            }
                        };

                        self.handshake_poll
                            .register(
                                &stream,
                                id.into(),
                                mio::Ready::readable() | mio::Ready::writable(),
                                mio::PollOpt::edge(),
                            )
                            .expect("Stream registration failed");
                    }
                    Err(err) => {
                        if err.kind() != io::ErrorKind::WouldBlock {
                            panic!("Failure accepting connection {:?}", err);
                        }
                    }
                }
            }
        }

        // Run handshake poll
        self.handshake_poll
            .poll(&mut self.events, Some(time::Duration::from_secs(0)))
            .expect("Handshake poll failed");

        for event in &self.events {
            if event.readiness().is_readable() {
                let id: usize = event.token().into();
                let channel = &mut self.channels[id];
                match channel.read_connection_token(&self.secret_key) {
                    Ok(user_id) => (),
                    Err(err) => {

                    }
                }
            }
        }

        // Run connected poll
    }

    #[inline]
    pub fn changes(&mut self) -> impl Iterator<Item = ConnectionChange> + '_ {
        self.changes.drain(..)
    }

    fn housekeeping(&mut self) {
        let now = self.current_time;
        let live_set = &mut self.live;
        let free_set = &mut self.free;
        let channels = &mut self.channels;
        let changes = &mut self.changes;

        live_set.retain(|&channel_id| {
            let channel = &mut channels[channel_id];

            let retain = match channel.get_state() {
                ChannelState::Handshake(timestamp) => now.duration_since(timestamp) < Self::HANDSHAKE_TIMEOUT,
                ChannelState::Connected(user_id) => {
                    if channel.last_ingress_elapsed(now) >= Self::INGRESS_TIMEOUT {
                        return false;
                    }

                    if channel.last_egress_elapsed(now) >= Self::KEEPALIVE_INTERVAL {
                        if channel
                            .write_control(ControlFrame::Keepalive(user_id))
                            .has_failed()
                        {
                            panic!("Fatal write error")
                        }
                    }

                    true
                }
                ChannelState::Disconnected => panic!("Disconnected channel in live set"),
            };

            // Close the channel in case of a timeout. Don't send a notification since the connection is
            // most likely dead.
            if !retain {
                channel.close(false);
                free_set.push(channel_id);
                changes.push(ConnectionChange::Disconnected(channel_id));
            }

            retain
        });
    }

    #[inline]
    fn get_comm_ctx(&mut self, channel_id: ChannelId) -> CommCtx {
        CommCtx {
            id: channel_id,
            channel: &mut self.channels[channel_id],
            changes: &mut self.changes,
            live: &mut self.live,
            free: &mut self.free,
        }
    }
}

struct CommCtx<'a> {
    id: ChannelId,
    channel: &'a mut Channel,
    changes: &'a mut Vec<ConnectionChange>,
    live: &'a mut IndexSet<ChannelId>,
    free: &'a mut Vec<ChannelId>,
}

impl<'a> CommCtx<'a> {
    #[inline]
    fn disconnect(&mut self, notify: bool) {
        self.channel.close(notify);
        self.changes.push(ConnectionChange::Disconnected(self.id));
        self.live.remove(&self.id);
        self.free.push(self.id);
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
