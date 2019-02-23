use crate::net::channel::{Channel, ChannelId, ChannelState};
use crate::net::frame::{ControlFrame, Frame};
use crate::net::support::{
    Deserialize, ErrorType, ErrorUtils, NetworkError, NetworkResult, PayloadBatch, Serialize,
};
use flux;
use flux::logging;
use flux::session::server::SessionKey;
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
    Connected(flux::UserId, ChannelId),
    Disconnected(ChannelId),
}

/// Handles all connection management and network transmission.
pub struct Endpoint {
    server: TcpListener,

    server_poll: mio::Poll,
    handshake_poll: mio::Poll,
    live_poll: mio::Poll,
    events: mio::Events,

    session_key: SessionKey,

    channels: Vec<Channel>,
    free: Vec<ChannelId>,
    live: IndexSet<ChannelId>,

    changes: Vec<ConnectionChange>,

    current_time: time::Instant,
    housekeeping_time: time::Instant,

    log: logging::Logger,
}

impl Endpoint {
    const HANDSHAKE_TIMEOUT: time::Duration = time::Duration::from_secs(5);
    const INGRESS_TIMEOUT: time::Duration = time::Duration::from_secs(30);
    const KEEPALIVE_INTERVAL: time::Duration = time::Duration::from_secs(3);
    const HOUSEKEEPING_INTERVAL: time::Duration = time::Duration::from_secs(3);
    const ZERO_TIME: time::Duration = time::Duration::from_secs(0);
    const SERVER_POLL_TOKEN: mio::Token = mio::Token(0);

    /// Construct a new `Endpoint`. The listener will be bound to the provided address in the
    /// format `<ip_or_domain>:<port>`.
    /// The `secret_key` is shared with an external authenticator service, so the initial client handshake
    /// can be decrypted.
    /// Finally, the `version` should denote unique and incompatible transmission protocol versions.
    #[inline]
    pub fn new(address: &str, secret_key: SessionKey, log: &logging::Logger) -> NetworkResult<Endpoint> {
        let now = time::Instant::now();

        let endpoint = Endpoint {
            server: TcpListener::bind(&address.parse::<SocketAddr>()?)?,
            server_poll: mio::Poll::new()?,
            handshake_poll: mio::Poll::new()?,
            live_poll: mio::Poll::new()?,
            events: mio::Events::with_capacity(8192),
            session_key: secret_key,
            channels: Vec::new(),
            free: Vec::new(),
            live: IndexSet::new(),
            changes: Vec::new(),
            current_time: now,
            housekeeping_time: now,
            log: log.new(logging::o!()),
        };

        Ok(endpoint)
    }

    #[inline]
    pub fn init(&self) {
        self.server_poll
            .register(
                &self.server,
                Self::SERVER_POLL_TOKEN,
                mio::Ready::readable(),
                mio::PollOpt::edge(),
            )
            .unwrap();
    }

    #[inline]
    pub fn push<P: Serialize>(&mut self, channel_id: ChannelId, data: &mut PayloadBatch<P>) {
        logging::trace!(self.log, "pushing payload to channel";
                        "context" => "push",
                        "channel_id" => channel_id,
                        "size" => data.len());

        let channel = &mut self.channels[channel_id];

        if channel.write_payload(data).has_failed() {
            panic!("Fatal write error");
        }
    }

    pub fn pull<P: Deserialize>(&mut self, channel_id: ChannelId, data: &mut PayloadBatch<P>) {
        logging::trace!(self.log, "pulling data into payload";
                        "context" => "pull",
                        "channel_id" => channel_id);

        let mut ctx = self.get_comm_ctx(channel_id);

        match ctx.channel.read() {
            Ok(frame) => {
                match frame {
                    Frame::Control(ctr) => {
                        match ctr {
                            // Disconnect notice sent by client. Close channel but don't send notice back.
                            ControlFrame::ConnectionClosed(_) => {
                                logging::debug!(ctx.log, "connection closed by client";
                                                "context" => "pull",
                                                "channel_id" => channel_id,
                                                "result" => "ok",
                                                "type" => "control",
                                                "message" => "ConnectionClosed");
                                ctx.disconnect(false)
                            }
                            // Connection accepted sent by client in error, close channel and notify.
                            ControlFrame::ConnectionAccepted(_) => {
                                logging::debug!(ctx.log, "erroneous connection acceptance message received";
                                                "context" => "pull",
                                                "channel_id" => channel_id,
                                                "result" => "error",
                                                "type" => "control",
                                                "message" => "ConnectionAccepted");
                                ctx.disconnect(true)
                            }
                            // Keepalive requests are ignored at this stage.
                            ControlFrame::Keepalive(_) => {
                                logging::debug!(ctx.log, "keepalive message received";
                                                "context" => "pull",
                                                "channel_id" => channel_id,
                                                "result" => "ok",
                                                "type" => "control",
                                                "message" => "KeepAlive");
                            }
                        };
                    }
                    Frame::Payload(pinfo) => {
                        logging::trace!(ctx.log, "payload message received";
                                        "context" => "pull",
                                        "channel_id" => channel_id,
                                        "result" => "ok",
                                        "type" => "payload",
                                        "payload_info" => ?pinfo);
                        if ctx.channel.read_payload(data, pinfo).has_failed() {
                            ctx.disconnect(true)
                        }
                    }
                }
            }
            Err(NetworkError::Fatal(err)) => {
                logging::error!(ctx.log, "fatal read error";
                                "context" => "pull",
                                "channel_id" => channel_id,
                                "result" => "error",
                                "error" => ?err);
                ctx.disconnect(true)
            }
            Err(NetworkError::Wait) => {
                logging::debug!(ctx.log, "pull";
                                "context" => "pull",
                                "channel_id" => channel_id,
                                "result" => "wait");
            }
        }
    }

    pub fn sync(&mut self, now: time::Instant) {
        self.current_time = now;
        logging::trace!(self.log, "starting network sync";
                        "context" => "sync",
                        "current_time" => ?self.current_time);

        if now.duration_since(self.housekeeping_time) >= Self::HOUSEKEEPING_INTERVAL {
            self.housekeeping();
            self.housekeeping_time = now;
        }

        let log = &self.log;
        let live_set = &mut self.live;
        let free_set = &mut self.free;
        let channels = &mut self.channels;
        let changes = &mut self.changes;

        logging::trace!(log, "current status";
                        "context" => "sync",
                        "live_count" => live_set.len(),
                        "free_count" => free_set.len(),
                        "channel_count" => channels.len());

        // Force send data on all live channels
        live_set.retain(|&channel_id| {
            logging::debug!(log, "sending data";
                            "context" => "sync",
                            "channel_id" => channel_id);

            let channel = &mut channels[channel_id];

            let result = if channel.has_egress() {
                channel.send(now)
            } else {
                Ok(0)
            };

            // Close the channel in case of a send error. No point in trying to send a notice.
            if result.has_failed() {
                let err = result.unwrap_err();

                logging::error!(log, "disconnecting channel due to write error";
                                "context" => "sync",
                                "channel_id" => channel_id,
                                "error" => ?err);

                channel.close(false);
                free_set.push(channel_id);
                changes.push(ConnectionChange::Disconnected(channel_id));
                return false;
            }

            true
        });

        logging::trace!(log, "running listen poll"; "context" => "sync");

        // Run listen poll
        self.server_poll
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Listen poll failed");

        for event in &self.events {
            // Writeable readiness indicates *possible* incoming connection
            if event.readiness().is_readable() {
                // See if there is a connection to be accepted
                match self.server.accept() {
                    Ok((stream, addr)) => {
                        // Retrieve an existing channel instance or create a new one
                        let id = match free_set.pop() {
                            Some(id) => id,
                            None => {
                                let id = channels.len();
                                channels.push(Channel::new(
                                    flux::VERSION_ID,
                                    flux::PROTOCOL_ID,
                                    Some(&self.log),
                                ));
                                id
                            }
                        };

                        logging::info!(log, "incoming connection";
                                       "context" => "sync",
                                       "address" => ?addr,
                                       "channel_id" => id);

                        logging::debug!(log, "registering channel with handshake poll";
                                        "context" => "sync",
                                        "channel_id" => id);

                        // Open the channel
                        let channel = &mut channels[id];
                        channel.open(id, stream, self.current_time);

                        // Register the channel on the handshake poll. Clients must deliver a valid
                        // handshake message before the connection is fully accepted.
                        channel
                            .register(id, &self.handshake_poll)
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
        self.events.clear();

        logging::trace!(log, "running handshake poll"; "context" => "sync");

        // Run handshake poll
        self.handshake_poll
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Handshake poll failed");

        let session_key = &self.session_key;
        let handshake_poll = &self.handshake_poll;
        let live_poll = &self.live_poll;

        for event in &self.events {
            if event.readiness().is_readable() {
                let channel_id: ChannelId = event.token().into();
                let channel = &mut channels[channel_id];

                logging::debug!(log, "reading handshake";
                                "context" => "sync",
                                "channel_id" => channel_id);

                channel
                    .receive(now)
                    .and_then(|_| channel.read_connection_token(session_key))
                    .and_then(|user_id| {
                        logging::info!(log, "handshake accepted";
                                       "context" => "sync",
                                       "channel_id" => channel_id,
                                       "user_id" => user_id);

                        if channel
                            .write_control(ControlFrame::ConnectionAccepted(user_id))
                            .has_failed()
                        {
                            panic!("Failure writing connection accepted frame")
                        }

                        logging::debug!(log, "moving channel to live poll";
                                        "context" => "sync",
                                        "channel_id" => channel_id);
                        changes.push(ConnectionChange::Connected(user_id, channel_id));

                        // The channel is now fully connected. Deregister from the handshake poll and
                        // register on the live poll.
                        channel
                            .deregister(handshake_poll)
                            .expect("Deregistration failed");
                        channel
                            .register(channel_id, live_poll)
                            .expect("Registration failed");
                        Ok(())
                    })
                    .unwrap_or_else(|err| {
                        // Disconnect the channel in case there is an error
                        if err != NetworkError::Wait {
                            logging::error!(log, "disconnecting channel due to handshake read error";
                                            "context" => "sync",
                                            "channel_id" => channel_id,
                                            "error" => ?err);
                            channel.close(false);
                            live_set.remove(&channel_id);
                            free_set.push(channel_id);
                            changes.push(ConnectionChange::Disconnected(channel_id));
                        } else {
                            logging::info!(log, "waiting to receive full handshake message";
                                           "context" => "sync",
                                           "channel_id" => channel_id);
                        }
                    });
            }
        }
        self.events.clear();

        logging::trace!(log, "running live poll"; "context" => "sync");

        // Run connected poll
        let live_poll = &self.live_poll;
        live_poll
            .poll(&mut self.events, Some(Self::ZERO_TIME))
            .expect("Live poll failed");

        for event in &self.events {
            let readiness = event.readiness();
            let channel_id: ChannelId = event.token().into();
            let channel = &mut channels[channel_id];

            logging::debug!(log, "live channel ready for send/receive";
                            "context" => "sync",
                            "channel_id" => channel_id);

            // Perform both receive and send operations, disconnecting the channel if there is a fatal error.
            Self::ready_op(readiness.is_readable(), || {
                let result = channel.receive(now);

                logging::debug!(log, "received data";
                                "context" => "sync",
                                "channel_id" => channel_id,
                                "result" => ?result);

                result.map(|_| ())
            })
            .and_then(|_| {
                Self::ready_op(readiness.is_writable(), || {
                    let result = channel.send(now);

                    logging::debug!(log, "sent data";
                                    "context" => "sync",
                                    "channel_id" => channel_id,
                                    "result" => ?result);

                    result.map(|_| ())
                })
            })
            .unwrap_or_else(|err| {
                logging::error!(log, "disconnecting live channel due to error";
                            "context" => "sync",
                            "channel_id" => channel_id,
                            "error" => ?err);

                channel.deregister(live_poll).expect("Deregistration failed");
                channel.close(true);
                live_set.remove(&channel_id);
                free_set.push(channel_id);
                changes.push(ConnectionChange::Disconnected(channel_id));
            });
        }
        self.events.clear();

        logging::trace!(log, "network sync finished";
                        "context" => "sync",
                        "change_count" => changes.len());
    }

    /// Drains all the changes accumulated since the last `sync`
    #[inline]
    pub fn changes(&mut self) -> impl Iterator<Item = ConnectionChange> + '_ {
        self.changes.drain(..)
    }

    #[inline]
    fn ready_op<F: FnMut() -> NetworkResult<()>>(trigger: bool, mut op: F) -> Result<(), ErrorType> {
        if trigger {
            loop {
                if let Err(err) = op() {
                    match err {
                        NetworkError::Wait => break,
                        NetworkError::Fatal(err_type) => return Err(err_type),
                    }
                }
            }
        }

        Ok(())
    }

    fn housekeeping(&mut self) {
        let log = &self.log;
        let now = self.current_time;
        let live_set = &mut self.live;
        let free_set = &mut self.free;
        let channels = &mut self.channels;
        let changes = &mut self.changes;

        logging::info!(log, "running housekeeping";
                       "context" => "housekeeping",
                       "current_time" => ?now,
                       "live_count" => live_set.len(),
                       "free_count" => free_set.len(),
                       "channel_count" => channels.len());

        live_set.retain(|&channel_id| {
            let channel = &mut channels[channel_id];

            logging::debug!(log, "processing channel";
                            "context" => "housekeeping",
                            "channel_id" => channel_id);

            let retain = match channel.get_state() {
                ChannelState::Handshake(timestamp) => now.duration_since(timestamp) < Self::HANDSHAKE_TIMEOUT,
                ChannelState::Connected(user_id) => {
                    if channel.last_ingress_elapsed(now) >= Self::INGRESS_TIMEOUT {
                        return false;
                    }

                    if channel.last_egress_elapsed(now) >= Self::KEEPALIVE_INTERVAL
                        && channel
                            .write_control(ControlFrame::Keepalive(user_id))
                            .has_failed()
                    {
                        panic!("Fatal write error")
                    }

                    true
                }
                ChannelState::Disconnected => panic!("Disconnected channel in live set"),
            };

            // Close the channel in case of a timeout. Don't send a notification since the connection is
            // most likely dead.
            if !retain {
                logging::warn!(log, "disconnecting channel due to timeout";
                              "context" => "housekeeping",
                              "channel_id" => channel_id);

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
            log: &self.log,
        }
    }
}

struct CommCtx<'a> {
    id: ChannelId,
    channel: &'a mut Channel,
    changes: &'a mut Vec<ConnectionChange>,
    live: &'a mut IndexSet<ChannelId>,
    free: &'a mut Vec<ChannelId>,
    log: &'a logging::Logger,
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
