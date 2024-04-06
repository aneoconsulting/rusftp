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

//! [`Dir`] module.

use std::{future::Future, pin::Pin};

use crate::client::{Error, SftpClient};
use crate::message::{Handle, Name};

mod close;
mod stream;

/// Directory accessible remotely with SFTP
pub struct Dir {
    client: SftpClient,
    handle: Option<Handle>,
    buffer: Option<Name>,
    pending: Option<PendingOperation>,
}

type PendingOperation = Pin<Box<dyn Future<Output = Result<Name, Error>> + Send + Sync + 'static>>;

impl Dir {
    /// Create a directory from a raw [`Handle`].
    ///
    /// The handle must come from `SftpClient::opendir`.
    ///
    /// The remote dir will be closed when the object is dropped.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open directory
    pub fn new(client: SftpClient, handle: Handle) -> Self {
        Dir {
            client,
            handle: Some(handle),
            buffer: Some(Default::default()),
            pending: None,
        }
    }

    /// Create a closed directory.
    ///
    /// The directory cannot be opened by any means.
    pub const fn new_closed() -> Self {
        Dir {
            client: SftpClient::new_stopped(),
            handle: None,
            buffer: None,
            pending: None,
        }
    }
}

pub static DIR_CLOSED: Dir = Dir::new_closed();

impl std::fmt::Debug for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dir")
            .field("client", &self.client)
            .field("handle", &self.handle)
            .field("buffer", &self.buffer)
            .field("pending", &self.pending.as_ref().map(|_| "..."))
            .finish()
    }
}
