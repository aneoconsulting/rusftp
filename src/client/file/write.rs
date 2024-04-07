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

use crate::client::{Error, SftpFuture, SftpReply, SftpRequest};
use crate::message::{Close, Data, Handle, Write};

use super::{File, OperationResult, PendingOperation};

impl File {
    /// Write to a portion of the file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn write(&self, offset: u64, data: impl Into<Data>) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `offset`: Byte offset where the write should start
    /// * `data`: Bytes to be written to the file
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn write(&self, offset: u64, data: impl Into<Data>) -> SftpFuture {
        if let Some(handle) = &self.handle {
            self.client.write(Handle::clone(handle), offset, data)
        } else {
            SftpFuture::Error(Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "File was already closed",
            )))
        }
    }
}

impl tokio::io::AsyncWrite for File {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        // Poll the pending operation, if any
        let result = match ready!(self.pending.poll(cx)) {
            OperationResult::Write(write) => write,
            // The pending operation was not a write, so we must start writing
            _ => {
                // Get the current handle, valid only if the file is not closed
                let Some(handle) = &self.handle else {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "File was closed",
                    )));
                };
                let handle = Handle::clone(handle);
                let length = buf.len().min(32768); // write at most 32K

                // Spawn the write future
                self.pending = PendingOperation::Write(
                    self.client.request_with(
                        Write {
                            handle,
                            offset: self.offset,
                            data: buf[0..length].to_owned().into(),
                        }
                        .to_request_message(),
                        length,
                        |length, msg| {
                            <()>::from_reply_message(msg)?;
                            Ok(length)
                        },
                    ),
                );

                // Try polling immediately
                if let PendingOperation::Write(pending) = &mut self.pending {
                    ready!(Pin::new(pending).poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready, adjust the offset according to the number of bytes written
        match result {
            Ok(len) => {
                self.offset += len as u64;
                std::task::Poll::Ready(Ok(len))
            }
            Err(err) => Poll::Ready(Err(err.into())),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        match ready!(self.pending.poll(cx)) {
            OperationResult::Write(Ok(len)) => {
                self.pending = PendingOperation::None;
                self.offset += len as u64;

                Poll::Ready(Ok(()))
            }
            OperationResult::Write(Err(err)) => Poll::Ready(Err(err.into())),
            _ => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        // Poll the pending operation, if any
        let result = match ready!(self.pending.poll(cx)) {
            OperationResult::Close(close) => close,
            // The pending operation was not a close, so we must start closing
            _ => {
                // Get the current handle, valid only if the file is not closed
                let Some(handle) = &self.handle else {
                    return Poll::Ready(Ok(()));
                };
                let handle = Handle::clone(handle);

                // Spawn the close future
                self.pending = PendingOperation::Close(self.client.request(Close { handle }));

                // Try polling immediately
                if let PendingOperation::Close(pending) = &mut self.pending {
                    ready!(Pin::new(pending).poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready
        Poll::Ready(result.map_err(Into::into))
    }
}
