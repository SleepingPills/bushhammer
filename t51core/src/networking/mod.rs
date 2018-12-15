/*
### Messaging ###

<-- (T) <-> |Security|
     ^  / \   |
     | /   \  |
--> (R) --> |NetSys|

Reciever
 - Recieve and assemble datagrams into messages
 - Preliminary validation on messages before passing on
 - To NetSys: payload messages
 - To Auth: connection related

Transmitter
 - Transmits packets to destination
 - Has a send<T>(...) function that gets a Vec<u8> from the pool, serializes the message
   and puts it on the send queue.
 - Maintains a pool of Vec<u8> used for containing the serialized data

Security
 - Maintains authentication and authorization info
 - Initially it stores everything in json files (one file per user account)
 - Periodically flushes data to disk
 - To Transmitter: connection related responses
 - To NetSys: successful authentication results

NetSys
 - Caches authorization info
 - Maintains connections
 - Routes payload messages to relevant systems
    - PlayerManager
       - Subcomponent of NetSys
       - Maintains player information like levels, attributes, stats etc...
       - Initially stores everything to JSON
 - To Transmitter: payload messages
 - To Auth: authentication/authorization changes

Pre-Connection
1. Client requests server's public key
2. Server responds with public key
4. Client generates secret key pair

Connection
1. Client sends connection request
2. Server validates request
3. Server decrypts password and authenticates the client
4. Server generates secret key pair for client
5. Server confirms connection
6. Auth sends authorization info to NetSys
4. NetSys creates client entry, loads player data, etc...
5. NetSys sends confirmation message to Transmitter

Pre-Connection Validation:
 - If the username is already connected, ignore
 - If the username does not exist, ignore

Pre-Connection Request
 - Version (string)
 - Protocol Id (u32)

Pre-Connection Response
 - Version (string)
 - Protocol Id (u32)
 - Server public key

Connection Request
 - Version (string)
 - Protocol Id (u32)
 - Username
 - Payload (encrypted with server's public key)
     - Client's public key
     - Password
 - Hash of everything using server's public key

Connection Confirmation
 - Version (string)
 - Protocol Id (u32)

*/