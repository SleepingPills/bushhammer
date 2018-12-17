/*
### End State ###
 - Users create accounts on the main platform
 - Users can purchase a server license, which gives them access to the server tooling
 - Users create server tokens, which are used for the dedicated server to register with the master
 - Each server instance requires a unique token
 - Users get unlimited tokens once a server license is purchased

Master Server
 - Serves as a registry of dedicated servers
 - Authenticates clients

Dedicated Server
 - Register with master using a configured token
 - Upon successful registration, the server recieves a secret key from the master that will
   be used to decrypt client authentication information.

### V1 ###
* Infrastructure *
The first iteration will be for a limited audience granted access using serial keys. Each serial
key allows one connection to the game server. This sidesteps the problem of maintaining user
accounts and registrations from the get go. That complexity can be tackled later.

In the first implementation, the authenticator will just live in-process

* Game Logic *
Use TCP. UDP could be made more optimal, but would require significantly more babysitting.

E.g. many things, like terraforming or atmospherics could result in desynch if things are dropped
or recieved unordered (e.g. the packet that clears some fire arrives before the last fire packet
will result in the fire getting hung).

Initially, all network related things and state replication will be handled by the endpoint.
State calculation could be done in parallel, but it is questionable whether the overhead of writing
the state to an intermediate structure, which the endpoint would have to effectively copy again
into the output streams is worth it.

For the sake of simplicity, the endpoint will handle everything initially, and we'll parallelize
it later optionally.

The core library will define an enum type for control messages. For payload messages, many of
the structs and traits will be parametrized so that the actual game logic can supply the payload
messages it requires. These will be batched per frame for each client, so that all the data for
that frame is transmitted in one packet.

* Objects *

   (inet) <-> Authenticator
  /
Client (ConnectionToken)
  \
   (inet) <-> Endpoint (Replicator)

Secret Key
 - Generated once and shared between in proc Authenticator and Endpoint

Authenticator
 - In process rocket instance with TLS (the client will trust the cert)
 - Client sends serial number
 - Authenticator validates serial number and sends ConnectionToken to client

Endpoint<R> (System)
 - R implements the replicator trait and brings in the custom payload message as asssociated type.
 - Manages all client communication
 - Maintains connections
 - Handles authorization
 - Use mio to poll read/write on all connected streams and the listener
 - The mio poll will run with a zero timeout, returning immediately if there are no events
 - Writeability events will drain any buffered data in channels
 - Channels will buffer all data that cannot be immediately written.
 - One payload packet structure will be reused for accumulating all payload messages for
   all clients. The data will be cleared between each client of course, but this avoids
   having to allocate.
 - Replicator (subcomponent)
   - Extracts replication data and writes it into relevant channels.
   - Writes data directly into the channels to avoid copying.

Replicator
 type Message: Serializable
 - fn record(&mut self, client: ClientId, buffer: &mut Payload<Message>) records payload messages
   into the supplied payload packet. The buffer is retained accross frames to avoid allocating. The
   endpoint will then batch these messages into one packet.

Channel
 - Control messages used internally will be a separate enum. The packet header can distinguish
   between the two types.
 - Wraps a TCPStream
 - Contains a ReadBuffer and WriteBuffer, for recieving and sending data (respectively).
 - Handles the sequencing of incoming data and the transmission of outgoing data.
 - fn write(&mut self, serializable: &S) writes the supplied serializable item into the buffer and
   attempts to send as much of it as possible.
 - fn recieve(&mut self) recieve all the data it can from the socket and write it into the
   read buffer.
 - fn read(&mut self) -> Result<Frame> returns a frame if one is available. The error results
   will be either some fatal error resulting in disconnect, or a simple note that there isn't
   enough data for the full frame yet.

Frame
 - Packet type: control or payload
 - Will contain the validated slice with all the data for a packet.
 - The frame has to be then immediately deserialized into either a control packet or
   payload packet.

Chunk
 - Contains a preset array of Box<[u8; BUF_SIZE]>. BUF_SIZE will be something like 8192 bytes.
 - Tracks the beginning of the data slice
 - Tracks the end of the data slice
 - Tracks the total capacity
 - If beginning reaches the end, it means all data has been read and the buffer is reset to
   empty state.

Buffer
 - Contains a Vec<Chunk> with at least one chunk always present.
 - Incoming data goes to the last chunk. If it fills up, a new chunk is retrieved from the pool.
 - Outgoing data is read from the first chunk, until it becomes empty, at which point it is put
   back in the pool, unless this is the last chunk in the buffer, in which case it remains.
 - Implements the Write and Read traits for efficient serialization.
 - fn read_into<R: Reader>(&mut self, reader: R, pool: &mut BufferPool) will read from the reader
   until it encounters an error.
 - fn write_into<W: Writer>(&mut self, writer: W, pool: &mut BufferPool) will write the buffer into
   the writer until it errors or all the data in the buffer is exhausted.

ChunkPool
 - A simple Vec<Chunk> wrapper that contains unused chunks.
 - When a chunk is requested, we check if there are any in the pool before allocating a new one.

* Connecting *
 - Client establishes TCP connection
 - Channel is created in AwaitingAuth state and the connection id is put in a set of awaitingconnection
 - Client sends ConnectionToken
 - Token is validated and client is moved to the connected state.

* Reception *
 - Readability is indicated for a token
 - Relevant channel is retrieved
 - recieve() is called to get all available data
 - read() is called until NoData is returned.

* Error Conditions *
 - If the header is malformed in any way the connection is severed due to corruption.
 - If the payload fails to decrypt for any reason the connection is severed due to corruption.
 - If a packet with a sequence number lower or equal to the current sequence arrives, and it is not
   a wraparound (ie. current sequence number is not maxval(u16) and new sequence is not 0), the
   connection is immediately severed due to possible replay attack.

* Packet Layout *
 <header>
 - Protocol Id: u16 (#0a55)
 - Version: u16 (#0001)
 - Type: u8 (control or payload)
 - Sequence: u16
 - Size in bytes: u16
 <data>
 - HMAC: 16 bytes
 - Payload

The protocol id, version, sequence and size are all used as additional information in encrypting
the payload, meaning that tampering with any of these will result in the message being invalid.

* Control Packets *
ConnectionToken
Disconnect

* Payload Packets *
Payload<P>
 - This will be just a wrapper over Vec<P> containing individual payload messages
 - We'll use the inplace deserialization to avoid allocating a new vector for each packet.
*/

pub mod chunk;
pub mod chunkpool;
pub mod endpoint;