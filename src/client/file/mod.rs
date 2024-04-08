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

//! [`File`] module.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{ready, Poll},
};

use crate::message::{self, Attrs, Handle};
use crate::{
    client::{Error, SftpClient},
    message::Data,
};

use super::SftpFuture;

mod close;
mod read;
mod seek;
mod write;

/// File accessible remotely with SFTP.
///
/// The file can be cloned, and the cloned file will point
/// to the same remote file, with the same native handle.
///
/// The remote file will be closed when all references to it have been dropped.
#[derive(Debug)]
pub struct File {
    client: SftpClient,
    handle: Option<Arc<Handle>>,
    offset: u64,
    pending: PendingOperation,
}

impl File {
    /// Create a file from a raw [`Handle`].
    ///
    /// The handle must come from `SftpClient::open`.
    ///
    /// The remote file will be closed when the object is dropped.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open file
    pub fn new(client: SftpClient, handle: Handle) -> Self {
        File {
            client,
            handle: Some(Arc::new(handle)),
            offset: 0,
            pending: PendingOperation::None,
        }
    }

    /// Create a closed file.
    ///
    /// The file cannot be opened by any means.
    pub const fn new_closed() -> Self {
        File {
            client: SftpClient::new_stopped(),
            handle: None,
            offset: 0,
            pending: PendingOperation::None,
        }
    }
}

pub static FILE_CLOSED: File = File {
    client: SftpClient::new_stopped(),
    handle: None,
    offset: 0,
    pending: PendingOperation::None,
};

impl File {
    /// Read the attributes (metadata) of the file.
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn stat(&self) -> SftpFuture<Attrs> {
        if let Some(handle) = &self.handle {
            self.client.request(message::FStat {
                handle: Handle::clone(handle),
            })
        } else {
            SftpFuture::Error(Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "File was already closed",
            )))
        }
    }

    /// Change the attributes (metadata) of the file.
    ///
    /// This request is used for operations such as changing the ownership,
    /// permissions or access times.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `attrs` - New attributes to apply
    pub fn set_stat(&self, attrs: Attrs) -> SftpFuture {
        if let Some(handle) = &self.handle {
            self.client.request(message::FSetStat {
                handle: Handle::clone(handle),
                attrs,
            })
        } else {
            SftpFuture::Error(Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "File was already closed",
            )))
        }
    }
}

impl Clone for File {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            handle: self.handle.clone(),
            offset: self.offset,
            pending: PendingOperation::None,
        }
    }
}

enum PendingOperation {
    None,
    Read(SftpFuture<Data>),
    Seek(SftpFuture<u64, i64>),
    Write(SftpFuture<usize, usize>),
    Close(SftpFuture),
}

impl std::fmt::Debug for PendingOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Read(_) => write!(f, "Read(...)"),
            Self::Seek(_) => write!(f, "Seek(...)"),
            Self::Write(_) => write!(f, "Write(...)"),
            Self::Close(_) => write!(f, "Close(...)"),
        }
    }
}

enum OperationResult {
    None,
    Read(Result<Data, Error>),
    Seek(Result<u64, Error>),
    Write(Result<usize, Error>),
    Close(Result<(), Error>),
}

impl PendingOperation {
    fn poll(&mut self, cx: &mut std::task::Context<'_>) -> Poll<OperationResult> {
        let result = match self {
            PendingOperation::None => OperationResult::None,
            PendingOperation::Read(pending) => {
                OperationResult::Read(ready!(Pin::new(pending).poll(cx)))
            }
            PendingOperation::Seek(pending) => {
                OperationResult::Seek(ready!(Pin::new(pending).poll(cx)))
            }
            PendingOperation::Write(pending) => {
                OperationResult::Write(ready!(Pin::new(pending).poll(cx)))
            }
            PendingOperation::Close(pending) => {
                OperationResult::Close(ready!(Pin::new(pending).poll(cx)))
            }
        };

        // Polling has finished, resetting pending
        *self = PendingOperation::None;

        Poll::Ready(result)
    }
}
