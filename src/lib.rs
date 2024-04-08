// This file is part of the rusftp project
//
// Copyright (C) ANEO, 2024-2024. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License")
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! SFTP library based on [`russh`].
//!
//! For now, only the [`client`] side is implemented.
//!
//! It implements the SFTP protocol RFC version 3.
//! See: <https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-02>
//!
//! # Design principles
//!
//! [`rusftp`](crate) is designed using the following principles:
//! - No panics
//! - No locking
//! - Shared client
//! - No borrowing for the user facing types
//! - Most futures are [`Send`] + [`Sync`] + `'static`
//! - All futures are eager
//!
//! So you can take a [`SftpClient`](crate::client::SftpClient), clone it, and use it behind a shared referenced.
//! You can start multiple SFTP requests concurrently, even from multiple threads.
//!
//! # Examples
//!
//! See <https://github.com/aneoconsulting/rusftp/blob/main/examples/simple_client.rs>

pub use russh;

pub mod client;
pub mod message;
pub mod wire;

pub mod utils;
