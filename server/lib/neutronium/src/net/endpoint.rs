use crate::net::channel::{Channel, ChannelId, ChannelState};
use crate::net::frame::{ControlFrame, Frame};
use crate::net::shared;
use crate::net::shared::ErrorUtils;
use indexmap::IndexSet;
use mio;
use mio::net::TcpListener;
use std::io;
use std::net::SocketAddr;
use std::time;

/// Describes a change in the connectivity status of a channel. A newly connected channel
/// is described by the user id and channel id.
#[derive(Debug, Copy, Clone)]
pub enum ConnectionChange {
    Connected(shared::UserId, ChannelId),
    Disconnected(ChannelId),
}

/// Handles all connection management and network transmission.
pub struct Endpoint {
    server: TcpListener,

    server_poll: mio::Poll,
    handshake_poll: mio::Poll,
    live_poll: mio::Poll,
    events: mio::Events,

    secret_key: [u8; 32],
    version: [u8; 16],
    protocol: u16,

    channels: Vec<Channel>,
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
    const ZERO_TIME: time::Duration = time::Duration::from_secs(0);
    const SERVER_POLL_TOKEN: mio::Token = mio::Token(0);
    const PROTOCOL: u16 = 0x0a55;

    /// Construct a new `Endpoint`. The listener will be bound to the provided address in the
    /// format `<ip_or_domain>:<port>`.
    /// The `secret_key` is shared with an external authenticator service, so the initial client handshake
    /// can be decrypted.
    /// Finally, the `version` should denote unique and incompatible transmission protocol versions.
    #[inline]
    pub fn new(
        address: &str,
        secret_key: [u8; 32],
        version: [u8; 16],
    ) -> shared::NetworkResult<Endpoint> {
        let server_poll = mio::Poll::new()?;
        let server = TcpListener::bind(&address.parse::<SocketAddr>()?)?;

        server_poll.register(
            &server,
            Self::SERVER_POLL_TOKEN,
            mio::Ready::writable(),
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
            protocol: Self::PROTOCOL,
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
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Listen poll failed");

        for event in &self.events {
            // Writeable readiness indicates *possible* incoming connection
            if event.readiness().is_writable() {
                // See if there is a connection to be accepted
                match self.server.accept() {
                    Ok((stream, _)) => {
                        // Retrieve an existing channel instance or create a new one
                        let id = match free_set.pop() {
                            Some(id) => id,
                            None => {
                                let id = channels.len();
                                channels.push(Channel::new(self.version, self.protocol));
                                id
                            }
                        };

                        // Register the channel on the handshake poll. Clients must deliver a valid
                        // handshake message before the connection is fully accepted
                        self.handshake_poll
                            .register(
                                &stream,
                                id.into(),
                                mio::Ready::readable() | mio::Ready::writable(),
                                mio::PollOpt::edge(),
                            )
                            .expect("Stream registration failed");

                        // Open the channel
                        channels[id].open(stream, self.current_time);
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
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Handshake poll failed");

        for event in &self.events {
            if event.readiness().is_readable() {
                let channel_id: ChannelId = event.token().into();
                let channel = &mut channels[channel_id];
                match channel.read_connection_token(&self.secret_key) {
                    Ok(user_id) => {
                        // The channel is now fully connected. Deregister from the handshake poll and
                        // register on the live poll.
                        if channel.write_control(ControlFrame::ConnectionAccepted(user_id)).has_failed() {
                            panic!("Failure writing connection accepted frame")
                        }
                        changes.push(ConnectionChange::Connected(user_id, channel_id));
                        channel
                            .deregister(&self.handshake_poll)
                            .expect("Deregistration failed");
                        channel
                            .register(channel_id, &self.live_poll)
                            .expect("Registration failed");
                    }
                    Err(err) => {
                        // Disconnect the channel in case there is an error
                        if err != shared::NetworkError::Wait {
                            channel.close(false);
                            live_set.remove(&channel_id);
                            free_set.push(channel_id);
                            changes.push(ConnectionChange::Disconnected(channel_id));
                        }
                    }
                }
            }
        }

        // Run connected poll
        let live_poll = &self.live_poll;
        live_poll
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Live poll failed");

        for event in &self.events {
            let readiness = event.readiness();
            let channel_id: ChannelId = event.token().into();
            let channel = &mut channels[channel_id];

            // Perform both receive and send operations, disconnecting the channel if there is a fatal error.
            Self::ready_op(readiness.is_readable(), || channel.receive(now))
                .and_then(|()| Self::ready_op(readiness.is_writable(), || channel.send(now)))
                .unwrap_or_else(|_| {
                    channel.deregister(live_poll).expect("Deregistration failed");
                    channel.close(true);
                    live_set.remove(&channel_id);
                    free_set.push(channel_id);
                    changes.push(ConnectionChange::Disconnected(channel_id));
                });
        }
    }

    /// Drains all the changes accumulated since the last `sync`
    #[inline]
    pub fn changes(&mut self) -> impl Iterator<Item = ConnectionChange> + '_ {
        self.changes.drain(..)
    }

    #[inline]
    fn ready_op<F: FnMut() -> shared::NetworkResult<()>>(
        trigger: bool,
        mut op: F,
    ) -> Result<(), shared::ErrorType> {
        if trigger {
            loop {
                if let Err(err) = op() {
                    match err {
                        shared::NetworkError::Wait => break,
                        shared::NetworkError::Fatal(err_type) => return Err(err_type),
                    }
                }
            }
        }

        Ok(())
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
