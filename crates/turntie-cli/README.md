# turntie-cli

This tool demostrates usage of `turntie` library that allows tying two TURN allocations with Mobility extension enabled together, for endpoints to be moved elsewhere and used for communicatinon.

## Features

* Creating a pair of allocations on a TURN server specified by IP, port, username and password and adding relayed addresses as permissions to each other.
* Serializing TURN client state (including credentials) as not too long base64 lines.
* Restoring TURN client from those lines and sending stdin lines to the other counterpart as UDP packets to TURN server.
* Serialized TURN client state (specifier) can be moved to another host / network.
* Available as a Rust library (for Tokio)

# Limitations

* Security is iffy: specified line contains TURN credentials in plaintext, communication channel is unreliable and insecure (basically raw UDP packets)
* TURN client implementation is simplified - does not check authentity of TURN server and may be no production-ready. I haven't read the RFC in full while implementing it.

## Installation

Download a pre-built executable from [Github releases](https://github.com/vi/turntie/releases) or install from source code with `cargo install --path crates/turntie-cli`  or `cargo install turntie-cli`.

## CLI options

<details><summary> turntie-cli --help output</summary>

```
TODO
```
</details>
