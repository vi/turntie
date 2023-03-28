# turntie

This library allows tying two TURN allocations with Mobility extension enabled together, for endpoints to be moved elsewhere and used for communicatinon.

## Features

* Creating a pair of allocations on a TURN server specified by IP, port, username and password and adding relayed addresses as permissions to each other.
* Serializing TURN client state (including credentials) as not too long base64 lines.
* Restoring TURN client from those lines and sending stdin lines to the other counterpart as UDP packets to TURN server.
* Serialized TURN client state (specifier) can be moved to another host / network.

# Limitations

* Security is iffy: specified line contains TURN credentials in plaintext, communication channel is unreliable and insecure (basically raw UDP packets)
* TURN client implementation is simplified - does not check authentity of TURN server and may be no production-ready. I haven't read the RFC in full while implementing it.
* Unconnected channels expire relatively quickly
* After connecting, an endpoint cannot be moved to next host anymore (in this implementation)
