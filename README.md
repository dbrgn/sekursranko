# Sekurŝranko

![Icon](safe.png)

An efficient and memory-safe Threema Safe server implementation
written in Rust.

This is a private project, not developed nor endorsed by Threema GmbH.

The server spec can be found in the [Cryptography
Whitepaper](https://threema.ch/press-files/2_documentation/cryptography_whitepaper.pdf).


## Status

Work in progress. The core functionality is implemented, but some additional
things like throttling or expiration still need to be handled. Currently, the
server needs to be manually compiled, but in the future binary builds will be
provided.

- [x] Request config
- [x] Download backups
- [x] Upload backups
- [x] Delete backups
- [ ] Settings configurable by user
- [ ] Throttling
- [ ] Automatic cleanup of expired backups


## Building

Sekurŝranko requires at least Rust 1.31.

To make a release build:

    cargo build --release

You will find the binary at `target/release/sekursranko`.


## Testing

Sekurŝranko is thoroughly covered by unit tests and integration tests.

To run the tests:

    cargo test

In case you want to enable logging:

    RUST_LOG=sekursranko=trace cargo test


## Running

Either build & run the binary directly with Cargo:

    cargo run --release

...or build and execute the generated binary.

Configure logging using the `RUST_LOG` env var:

    RUST_LOG=sekursranko=debug cargo run --release


## Name

The name of this project is the Esperanto word for "safe". English-speaking
people might recognize the "sekur-" prefix (-> secure), and German-speaking
people might recognize the "-ŝranko" suffix (-> "Schrank", a cabinet).


## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT) at your option.

**Contributing**

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
