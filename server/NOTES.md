
### End State
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
 - Upon successful registration, the server receives a secret key from the master that will
   be used to decrypt client authentication information.

### V1
* Infrastructure *
The first iteration will be for a limited audience granted access using serial keys. Each serial
key allows one connection to the game server. This sidesteps the problem of maintaining user
accounts and registrations from the get go. That complexity can be tackled later.

In the first implementation, the authenticator will just live in-process

* Game Logic *
Use TCP. UDP could be made more optimal, but would require significantly more babysitting.

E.g. many things, like terraforming or atmospherics could result in desynch if things are dropped
or received unordered (e.g. the packet that clears some fire arrives before the last fire packet
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

```
   (inet) <-> Authenticator
  /
Client (ConnectionToken)
  \
   (inet) <-> Endpoint (Replicator)
```

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
 - Use mio to poll read/write on all connected streams and the listener
 - The mio poll will run with a zero timeout, returning immediately if there are no events
 - Writeability events will drain any buffered data in channels
 - Channels will buffer all data that cannot be immediately written.
 - One payload packet structure will be reused for accumulating all payload messages for
   all clients. The data will be cleared between each client of course, but this avoids
   having to allocate.
 - Replicator (subcomponent)
   - Handles authorization (or a sub-component does)
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
 - Wraps a TCPStream and a Vec<u8> encryption/decryption buffer.
 - Contains a ReadBuffer and WriteBuffer, for recieving and sending data (respectively).
 - Handles the sequencing of incoming data and the transmission of outgoing data.
 - fn write(&mut self, serializable: &S) writes the supplied serializable item into the buffer and
   attempts to send as much of it as possible.
 - fn receive(&mut self) -> Result<()> receive all the data it can from the socket and write it into the
   read buffer. If there is an error, the connection will be immediately terminated, since all
   recoverable errors (e.g. WouldBlock) are handled by the channel/buffer.
 - fn read(&mut self) -> Result<Frame> returns a frame if one is available. The error results
   will be either some fatal error resulting in disconnect, or a simple note that there isn't
   enough data for the full frame yet.

   Parsing will happen by using serde to deserialize a Header object and then decrypt the contents
   into the decrypt buffer.

Frame
 - Header
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
 - Contains a Deque<Chunk> with at least one chunk always present.
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
 - receive() is called to get all available data
 - read() is called until NoData is returned.

* Error Conditions *
 - If the header is malformed in any way the connection is severed due to corruption.
 - If the payload fails to decrypt for any reason the connection is severed due to corruption.
 - If a packet with a sequence number lower or equal to the current sequence arrives, and it is not
   a wraparound (ie. current sequence number is not maxval(u16) and new sequence is not 0), the
   connection is immediately severed due to possible replay attack.

* Packet Layout *
 <header>
 - class: u8,
 - sequence: u64,
 - size: u16,
 <data>
 - Payload
 - HMAC: 16 bytes

The protocol id, version, sequence and size are all used as additional information in encrypting
the payload, meaning that tampering with any of these will result in the message being invalid.

* Control Packets *
ConnectionToken
 - protocol: u16 (#0a55)
 - version: u16 (#0001)
 - expire timestamp: unix timestamp
 - challenge sequence: u64
 - private data

Disconnect
 - Reason code: u8

* Payload Packets *
Payload<P>
 - This will be just a wrapper over Vec<P> containing individual payload messages
 - We'll use the inplace deserialization in serde to avoid allocating a new vector for each packet.

!!! 21.12.2018 !!!
 - Each state for a channel: created, challenge, connected will be implemented as a trait. The
   endpoint will have a vector for each, and as the handshake process progresses, the channel
   will be moved between collections.
 - There will be a separate poll for each state. Each poll will be run in turn.
 - This ensures that a single channel object can handle all states. They'll be distinguished
   by each state being handled as a separate trait.
 - The Frame structure will be used for both control and payload messages, along with the header.
   The endpoint will simply know what sort of control message to expect at each stage, which will
   be decrypted and then deserialized using bincode. If the deserialization fails, the connection
   will be dropped.

(net) -> ReadBuffer -> Header -> (size check) -> CryptoBuf -> (decrypt) -> Frame -> NetSys
Payload Packet -> Serialize -> Frame -> CryptoBuf -> WriteBuffer -> (net)


!!! 25.12.2018 !!!
(net) -> ReadBuffer -> Header -> (size check) -> (decrypt to frame) -> Frame -> NetSys
Payload Packet -> (serialize) -> SendBuffer -> Header -> (encrypt in-place) -> (net)

 - Putting the networking and state system in separate threads using message passing will incur
   way too much overhead that is probably not worth it. We'll stick to keeping everything in
   one Network System. Eventually we can separate out state delta calculation into a separate system
   and then they can communicate through a special resource that flips between two state objects based
   on a frame counter (e.g. system A accesses buffer 1 on even frames, and system B accesses it on odd frames).
 - If the client stays unreachable for 30 seconds or more, it will be disconnected.

Disconnect Logic
 - If we receive 0 bytes, we assume the connection died and the client is dropped.
 - If no disconnection message comes in, and the last transmission is non-zero, the client
   will get dropped by the time-out handler

!!! 26.12.2018 !!!
 - Use manual serialization/deserialization. This simplifies the buffer interface greatly and we
   can just operate directly on the buffer slices and move the head/tail in batches.
 - Can't use bincode. The client side most likely won't run rust and needs to be able to communicate.
 - The State manager will buffer state changes if one cannot be sent due to downtime on the connection.

 Serialization:
 1. Serialize the payload packet/buffer into the frame
 2. Set the header
 3. Serialize the frame into the write buffer.
   1. Serialize the header
   2. Encrypt the frame bytes in to the write buffer

 The channel will only return the actual packet payload, not a Frame or similar thing since there
 is no need for downstream things to deal with headers et. al.

 The frame extraction will be done by the channel. It will read the header, check that all the
 data is available in the buffer, and then decrypt the data into the data buffer (formerly frame).

!!! 1.1.2019 !!!
*** UNITS OF MEASURE ***
Use algebraic typing to create an SI derived units of measure system. Under the hood, eveything will be represented
as core metric/SI units.

Distance = meter
Time = second
Velocity = Distance/Time
Acceleration = Velocity/Time

The individual units will support factory functions to feed in various measures. e.g.
Velocity::mps(meters_per_second)
Velocity::kms(kilometers_per_second)
Velocity::kmh(kilometers_per_hour)
These will all return a Velocity object that measures things in mps and thus the factory function just converts
stuff.

There'll be the special case of the from_string(&str) function (e.g. Velocity::from_string(&str)) which will
accept a standardized string format, e.g. "50 m/s" for velocity (whitespace will be ignored).

Then if we have
speed = Velocity::mps(10)
distance_travelled = speed * Time:secs(50) // Get the distance travelled in 50 seconds.

Or we have
accel = Accel:mps2(10)
speed = accel * Time::secs(10) // Speed after 10 seconds of acceleration is 100 meters per second

*** Networking ***
Replication
- All entities to be replicated will be marked with a Replicated component containing an Option<ChannelId>. Replicator
  subsystems will simply add this to their query and funnel state data into special Resources bucketed by ChannelId.
- The Replicator Hub will have write access to the Replicator component and will set the field to None for entities
  whose client disconnected.
- This concept works for both persistent and transient worlds. Persistent worlds will delete the client entities when
  the client is not logged in, so they don't use resources. Transient worlds can keep the client entity around until
  the round end when everything gets recycled.

Endpoint
- pull(channel_id, payload_batch) - pulls all messages from the given channel into the batch
  Internally, the endpoint will disconnect the channel if the error was anything other than Error::Wait and
  add an entry to the change queue. The method returns nothing, if there was a fatal error, a disconnect
  entry is added for the channel.
- push(channel_id, payload_batch) - puts as many messages as possible from the given batch on the channel. 
  Returns nothing, but any fatal error messages will result in a disconnect entry on the channel change queue. 
- sync() - Carries out the actual transmissions. Loop through all live channels and force send any that have data
  available.
  Any errors (apart from Error:Wait) result in disconnection.
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

Replicator Hub
- Map UserId -> EntityId (all users ever connected in this session).
- Vec<Client{payload, entity_id}> indexed by ChannelId (thus replicating the Channel vector).
- IndexSet<ChannelId> of connected channels.
- Each Client instance keeps a payload buffer and the entity id of the associated entity.
- Write access to Replicator resources, which are drained into the Client instances' payload buffer.

Replicator Subsystems
- Read access to the Replicated component and other Components specific to the subsystem.
- Write access to it's own resource, where messages are bucketed into a vector indexed by ChannelId and replicating the
  length of the Hub's client vector.

Use cases
- Endpoint: Pull incoming messages for clients.
- Endpoint: Push state changes to clients.
- Endpoint: Iterate through all
- Endpoint: Iterate through disconnected clients and mark them as disconnected. The payload buffer for the client will be
  flushed.
- Iterate through new connections and either wire them up to an existing client instance or create a new one.

https://uterrains.com/demo/
https://assetstore.unity.com/packages/tools/modeling/ruaumoko-8176
https://assetstore.unity.com/packages/tools/modeling/voxelab-complete-edition-58423
https://assetstore.unity.com/packages/tools/terrain/ultimate-terrains-voxel-terrain-engine-31100

# Overall Architecture

```
                                +---------------+  +--------------+  +-----------------+
                                |               |  |              |  | Internal Dash   |
                                |  Public Dash  |  | Mgmt Console |  | Grafana/Kibana  |
+-----------------+             |               |  |              |  |                 |
|                 |             +---^-----------+  +----------^---+  +--------+--------+
|  Authenticator  <---------+       |                         |               |
|                 |         |       |    +---------------+    |      +--------|--------+
+--------+--------+         |       |    |               |    |      |  Log Store      |
         |                  |       +----> Master Server <----+      |  Elasticsearch  |
         |                  |            |               |           |                 |
         |                  |            +-------^-------+           +--------+--------+
         |                  |                    |                            |
         |          Server  |                    |                   +--------|--------+
         |          Auth    |                    |                   | Log Aggregator  |
         |    User          |                    |                   | Kafka           |
         |    Auth          |                    |                   |                 |
         |                  |                    |                   +--------^--------+
         |                  |                    |                            |
         |                  |                    |                            |
         |                  |                    |                            |
         |             +----|---------------+    |                            |
         |             |Game Servers        |    |                            |
         |             |                    |    |   Tail log files           |
         |-------------- +----------------+ <----+   Fluentd/Logstash         |
         |             | |    Server 1    | |                                 |
         |             | +----------------+ |                                 |
         |             | +----------------+ |                                 |
     +---|---+         | |   Server ...   | |                                 |
     |       |         | +----------------+ |                                 |
     | Users |         | +----------------+ ----------------------------------+
     |       |         | |    Server N    | |
     +-------+         | +----------------+ |
                       +--------------------+
```
## Server Session Proces
1. Account holder creates server token in the management console
2. Token added to the game server config
3. On startup, game server authenticates with the Authenticator using the token
3. Game server recieves unique server id and session key
4. Game server forwards server id and session key to the master server
5. Session key is used to share private user session data between Authenticator and Game Server
6. Session key is used to share heartbeat data with master server 

## User Session Process
1. User selects server from the browser
2. User application sends Server Id and credentials to Authenticator
3. Authenticator sends connection token to user application encrypting the private part with the session key

## Authenticator
- Authenticates users and game servers (two separate URIs)
- Initial implementation just uses serial keys for users to authenticate and skips server authentication

## Users
- Select server from the server browser
- Authenticate with the Authenticator
- Receive connection token
- Forward token to the game server

## Game Servers
- Initially run purely in-house, so no need for server authentication
- Receive connection token from the users
- Authenticate server with the Authenticator
- Receive session key and server id from the Authenticator. This will be the shared secret key between Authenticator
  and the game server used to decrypt the private part of the user connection tokens.  
- Create session on Master Server (private data contains the session key, it can only be decrypted by the internal
  secret key and the dashboard can thus validate that the session key is valid).
- Send heartbeats to public dashboard using session key
- Log everything to files

## Master Server
- Game servers register here to get listed using the session key and server id received from the Authenticator
- Provides the ServerId to the server browser in clients so they can include it in their authentication request
- Provides APIs for the Public Dashboard
- Porivdes APIs for the Management Console to administer servers and authorization info
- Maintains authorization data for servers and users
- All server management goes through this service, as it can provide a central place for managing all servers belonging
  to an account.

## Log Aggregator
- Tails all relevant log files and aggregates them into one log store

## Log Store
- Collects logs from various sources for analytics

## Internal Dash
- Internal log analytics

## Public Dash
- Maintains public facing server information like connected users, uptime etc..

## Management Console
- Administer a group of servers belonging to a specific account
- Manage bans
- Manage user privileges
  - This has to be flexible, e.g. one can define any groups and assign privileges
  - Built-in groups like Everyone
  - The game servers will load and refresh the authorization info periodically, in addition to receiving
    authorization change notifications.
- Etc...

## User Data Store
- Store global user info (name, contacts, payment info)

## User Authorization Store
- Store global bans, etc..
- Store authorization structures defined for server groups
- Store user memberships in the authorization structures. This should be done such that removing structures doesn't
  leave dangling data - in a RDBMS it should cascade the deletes. In a document store, the mappings should be stored
  around the structure and not the user data.

## Internal Game Server Architecture
The following high level components are present:
- Controller: Runs on it's own tokio runtime and relays messages between the endpoint and the master server
- Endpoint
- Replicator System
- Game systems

# V1 Simplifications
- Game Server authentication is skipped. A session key is pre-shared through a config file. 