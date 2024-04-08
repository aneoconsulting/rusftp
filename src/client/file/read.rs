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

use std::{future::Future, pin::Pin, task::ready, task::Poll};

use bytes::Bytes;

use crate::client::{Error, SftpFuture};
use crate::message::{Handle, Read, Status, StatusCode};

use super::{File, OperationResult, PendingOperation};

impl File {
    /// Read a portion of the file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn read(&self, offset: u64, length: u32) -> Result<Bytes, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `offset`: Byte offset where the read should start
    /// * `length`: Number of bytes to read
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn read(&self, offset: u64, length: u32) -> SftpFuture<Bytes> {
        if let Some(handle) = &self.handle {
            self.client.read(Handle::clone(handle), offset, length)
        } else {
            SftpFuture::Error(Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "File was already closed",
            )))
        }
    }
}

impl tokio::io::AsyncRead for File {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::result::Result<(), std::io::Error>> {
        // Poll the pending operation, if any
        let result = match ready!(self.pending.poll(cx)) {
            OperationResult::Read(read) => read,
            // The pending operation was not a read, so we must start reading
            _ => {
                // Get the current handle, valid only if the file is not closed
                let Some(handle) = &self.handle else {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "File was closed",
                    )));
                };
                let handle = Handle::clone(handle);

                // Spawn the read future
                self.pending = PendingOperation::Read(self.client.request(Read {
                    handle,
                    offset: self.offset,
                    length: buf.remaining().min(32768) as u32, // read at most 32K
                }));

                // Try polling immediately
                if let PendingOperation::Read(pending) = &mut self.pending {
                    ready!(Pin::new(pending).poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready, write to the buffer if it is a success
        match result {
            Ok(data) => {
                buf.put_slice(&data);
                self.offset += data.len() as u64;
                std::task::Poll::Ready(Ok(()))
            }
            Err(Error::Sftp(Status {
                code: StatusCode::Eof,
                ..
            })) => std::task::Poll::Ready(Ok(())),
            Err(err) => Poll::Ready(Err(err.into())),
        }
    }
}
