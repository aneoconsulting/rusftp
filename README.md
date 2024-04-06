# RUSFTP

This is a SFTP implmentation based on [russh](https://crates.io/crates/russh).

Links:
- [github](https://github.com/aneoconsulting/rusftp)
- [crates](https://crates.io/crates/rusftp)
- [docs](https://docs.rs/rusftp/)

# Rationale

Why another SFTP library?

When I started to work on this project, there were no pure Rust async SFTP client library.


# Design principles

`rusftp` is designed using the following principles:
- No panics
- No locking
- Shared client
- User facing types have no dependent lifetimes
- Futures are `Send` + `Sync` + `'static`
- Futures are eager

So you can take a `SftpClient`, clone it, and use it behind a shared referenced.
You can start multiple SFTP requests concurrently, even from multiple threads.

# Features

- [x] Client
    - [x] Concurrent requests
    - [x] Cloneable `SftpClient` and `File`
    - [x] File (`tokio::io` abstraction)
    - [x] Dir (`futures::stream` abstraction)
    - [x] All supported requests and messages
    - [ ] Path abstraction (currently just a wrapper around Bytes)
    - [ ] Support for well known SFTP extensions
    - [ ] User defined extensions
    - [ ] Support for direct Byte stream (ie: no [`russh`])

- [ ] Server
- Protocol Version
    - [x] version 3
    - [ ] version 4
    - [ ] version 5
    - [ ] version 6
