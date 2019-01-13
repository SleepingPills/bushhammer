//! The networking module handles all the communication between clients and the server. The core principle
//! is to avoid allocations at all costs and minimize copying data between buffers.
//!
//! The following main components comprise the networking module:
//!
//! - `Endpoint`, responsible for the client communications lifecycle and channel management.
//! - `Channel`, responsible for buffering, cryptography and ultimately transmission of data.
//! - `Buffer`, ring buffer using virtual memory paging tricks.
//!
//! The process is broadly built upon the [Netcode.io framework](https://github.com/networkprotocol/netcode.io).
//!
//! The principles are the following:
//!
//! 1. The server (`Endpoint`) and and authenticator service (the `Authenticator`) share a secret key that
//!    they can use to securely relay messages through the client.
//! 2. All communication is encrypted and signed. Tampering with any part of a message will result in
//!    the encryption checks failing.
//! 3. Extremely strict validation of data in a fail fast manner. Any data inconsistency immediately results
//!    in the server closing the connection.
//! 4. The tansmission protocol is TCP. The vast majority of messages need to have guaranteed, in-order
//!    delivery. The scaling and back-off parameters of the protocol will be tuned for maximal rampup speed
//!    and aggressive retransmission timings.
//!
//!    See:
//!    - [Akamai TCP](https://developer.akamai.com/legacy/learn/Optimization/TCP_Optimizations.html)
//!    - [TCP Vegas](https://en.wikipedia.org/wiki/TCP_Vegas)
//!    - [TCP Tuning](https://en.wikipedia.org/wiki/TCP_tuning)
//!
//! The client observes the following workflow when connecting:
//!
//! 1. Connect to an external authentication service (the `Authenticator`) and authenticate themselves.
//! 2. Upon successful authentication the `Authenticator` responds with a connection token.
//! 3. The client connects to the `Endpoint` listen server and forwards the encrypted part of the token.
//! 4. The `Endpoint` receives the connection token and decrypts it with the secret key.
//! 5. Upon validation, the `Endpoint` sends the client an acceptance notification.
//!
//! The connection is then considered fully established. If the `Endpoint` receives any data out of order
//! or in an unexpected form, content or type, the connection is immediately severed without notice to the
//! client.
//!
//! Once the connection is operational, the communication happens as follows:
//!
//! 1. Messages from a `PayloadBuffer` can be pushed (using `push()`) to a specific channel in the `Endpoint`.
//! 2. Messages from a specific channel in the endpoint can be pulled (using `pull()`) into a `PayloadBuffer`.
//! 3. `sync()` performs all synchronisation operations on the channel:
//!   a. Send all outstanding data on all channels.
//!   c. Poll for incoming connections.
//!   d. Poll for incoming handshakes (connection tokens).
//!   e. Poll for data available for reading on live channels
//!   f. Perform housekeeping operations: close dead channels and send keepalive messages.
//! 4. Channel connectivity changes are recorded in a queue and can be consumed by downstream systems.
//!
//! The `Endpoint` exposes an API for downstream systems to perform these operations. Consumers of the API
//! can perform (amortized) zero allocation communication using pooled `PayloadBuffer` instances.
//!
//! Channels and the respective clients can be identified by a pair of ChannelId and UserId instances.
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::new_without_default)]
#![allow(clippy::new_without_default_derive)]

pub mod buffer;
pub mod channel;
pub mod crypto;
pub mod endpoint;
pub mod frame;
pub mod shared;
