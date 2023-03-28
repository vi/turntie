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
* Unconnected channels expire relatively quickly
* After connecting, an endpoint cannot be moved to next host anymore (in this implementation)

## Installation

Download a pre-built executable from [Github releases](https://github.com/vi/turntie/releases) or install from source code with `cargo install --path crates/turntie-cli`  or `cargo install turntie-cli`.

## Example

```
hostA$ turntie tie 203.0.113.65:3478  myuser  mypassword
eJwtj?REDACTED?kvUYOw==
eJwtj?REDACTED?xjF6Y=

hostB$ turntie connect eJwtj?REDACTED?kvUYOw==
> 12345
< QQQQQ
^C

hostC$ turntie connect eJwtj?REDACTED?xjF6Y=
< 12345
> QQQQQ
^C
```

Both `turntie connect`s should be running simultanesouly to work.

Full unredacted specifiers are typically consist of about 150 base64 characters.

## CLI options

<details><summary> turntie --help output</summary>

```
Usage: turntie <command> [<args>]

Use TURN server as a communication channel with movable ends Top-level command.

Options:
  --help            display usage information

Commands:
  tie               Create two allocation and return two specifier lines for
                    their resumption
  connect           Connect to one of the lines


Usage: turntie tie <turn_server> <username> <password>

Create two tied allocations and print two specifier lines for usage with 'turntie connect'.

Positional Arguments:
  turn_server       address of TURN server to create allocations in
  username          username to authenticate on TURN server with
  password          password to authenticate on TURN server with

Options:
  --help            display usage information

Usage: turntie connect <specifier>

Connect to one of the endpoints created by 'turntie tie' and exchange stdin/stdout lines with the peer which connected to the other endpoint.

Positional Arguments:
  specifier         serialized data describing the channel end

Options:
  --help            display usage information


```
</details>
