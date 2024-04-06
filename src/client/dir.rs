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

use std::{future::Future, pin::Pin, task::ready};

use futures::Stream;

use crate::{ClientError, Close, Handle, Name, NameEntry, ReadDir, SftpClient, Status, StatusCode};

/// Directory accessible remotely with SFTP
pub struct Dir {
    client: SftpClient,
    handle: Option<Handle>,
    buffer: Option<Name>,
    pending: Option<PendingOperation>,
}

type PendingOperation =
    Pin<Box<dyn Future<Output = Result<Name, ClientError>> + Send + Sync + 'static>>;

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

impl Dir {
    /// Check whether the directory is closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_none()
    }

    /// Close the remote dir
    pub fn close(&mut self) -> impl Future<Output = Result<(), ClientError>> {
        let future = if let Some(handle) = std::mem::take(&mut self.handle) {
            Some(self.client.request(Close { handle }))
        } else {
            // If the dir was already closed, no need to close it
            None
        };
        let mut client = std::mem::replace(&mut self.client, SftpClient::new_stopped());

        async move {
            let response = match future {
                Some(future) => future.await,
                None => Ok(()),
            };

            // Avoid keeping the client alive until the directory is dropped
            client.stop().await;

            response
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        _ = futures::executor::block_on(self.close());
    }
}

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

impl Stream for Dir {
    type Item = Result<NameEntry, ClientError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // If end of file reached, resturn None
        let Some(buffer) = &mut self.buffer else {
            return std::task::Poll::Ready(None);
        };

        // If still some entries in the buffer, get next
        if let Some(entry) = buffer.0.pop() {
            return std::task::Poll::Ready(Some(Ok(entry)));
        }

        let result = match &mut self.pending {
            Some(pending) => ready!(pending.as_mut().poll(cx)),
            None => {
                let Some(handle) = &self.handle else {
                    // Force end of iteration
                    self.buffer = None;
                    return std::task::Poll::Ready(Some(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Dir was closed",
                    )
                    .into())));
                };

                let readdir = self.client.request(ReadDir {
                    handle: handle.clone(),
                });
                let pending = self.pending.insert(Box::pin(readdir));

                ready!(pending.as_mut().poll(cx))
            }
        };

        // If the read was successful, the buffer will be populated again
        // Stop the iteration otherwise
        self.buffer = None;

        let result = match result {
            Ok(mut entries) => {
                entries.reverse();

                if let Some(entry) = entries.0.pop() {
                    self.buffer = Some(entries);
                    Some(Ok(entry))
                } else {
                    Some(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Found no more directory entries while it was expecting some",
                    )
                    .into()))
                }
            }
            Err(ClientError::Sftp(Status {
                code: StatusCode::Eof,
                ..
            })) => None,
            Err(err) => Some(Err(err)),
        };

        std::task::Poll::Ready(result)
    }
}
