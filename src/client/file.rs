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

use std::{
    future::Future,
    pin::Pin,
    task::{ready, Poll},
};

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};

use crate::{message, Attrs, Close, Handle, SftpClient, Status, StatusCode, Write};

/// File accessible remotely with SFTP
#[derive(Debug)]
pub struct File {
    client: SftpClient,
    handle: Option<Handle>,
    offset: u64,
    pending: PendingOperation,
}

impl Drop for File {
    fn drop(&mut self) {
        _ = futures::executor::block_on(self.close());
    }
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
            handle: Some(handle),
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
    /// Check whether the file is closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_none()
    }

    /// Close the remote file
    pub fn close(&mut self) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        let future = if let Some(handle) = std::mem::take(&mut self.handle) {
            Some(self.client.request(message::Close { handle }))
        } else {
            // If the file was already closed, no need to close it
            None
        };

        async move {
            match future {
                Some(future) => future.await,
                None => Ok(()),
            }
        }
    }

    /// Read the attributes (metadata) of the file.
    pub fn stat(&self) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        let future = if let Some(handle) = &self.handle {
            Ok(self.client.request(message::FStat {
                handle: handle.clone(),
            }))
        } else {
            Err(StatusCode::Failure.to_status("File was already closed".into()))
        };

        async move {
            match future {
                Ok(future) => future.await,
                Err(err) => Err(err),
            }
        }
    }

    /// change the attributes (metadata) of the file.
    ///
    /// This request is used for operations such as changing the ownership,
    /// permissions or access times.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `attrs` - New attributes to apply (convertible to [`Attrs`])
    pub fn set_stat<A: Into<Attrs>>(
        &self,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        let future = if let Some(handle) = &self.handle {
            Ok(self.client.request(message::FSetStat {
                handle: handle.clone(),
                attrs: attrs.into(),
            }))
        } else {
            Err(StatusCode::Failure.to_status("File was already closed".into()))
        };

        async move {
            match future {
                Ok(future) => future.await,
                Err(err) => Err(err),
            }
        }
    }
}

impl AsyncRead for File {
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
                let Some(handle) = self.handle.clone() else {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "File was closed",
                    )));
                };

                // Spawn the read future
                let read = self.client.request(crate::Read {
                    handle,
                    offset: self.offset,
                    length: buf.remaining().min(32768) as u32, // read at most 32K
                });

                self.pending = PendingOperation::Read(Box::pin(async move {
                    match read.await {
                        Ok(data) => Ok(data.0),
                        Err(status) if status.code == StatusCode::Eof => Ok(Bytes::default()),
                        Err(status) => Err(std::io::Error::from(status)),
                    }
                }));

                // Try polling immediately
                if let PendingOperation::Read(pending) = &mut self.pending {
                    ready!(pending.as_mut().poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready, write to the buffer if it is a success
        match result {
            Ok(data) => {
                self.pending = PendingOperation::None;

                if data.is_empty() {
                    std::task::Poll::Ready(Ok(()))
                } else {
                    buf.put_slice(&data);
                    self.offset += data.len() as u64;
                    std::task::Poll::Ready(Ok(()))
                }
            }
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

impl AsyncSeek for File {
    fn start_seek(mut self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        if let PendingOperation::None = self.pending {
            match position {
                // Seek from start can be performed immediately
                std::io::SeekFrom::Start(n) => {
                    self.offset = n;
                }
                // Seek from end requires to stat the file first
                std::io::SeekFrom::End(i) => {
                    let stat = self.stat();
                    self.pending = PendingOperation::Seek(Box::pin(async move {
                        match stat.await?.size {
                            Some(n) => match n.checked_add_signed(i) {
                                Some(n) => Ok(n),
                                None => Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "Would seek to negative position",
                                )),
                            },
                            None => Err(std::io::Error::new(
                                std::io::ErrorKind::Unsupported,
                                "Unable to seek from the end of file: could not get file size",
                            )),
                        }
                    }));
                }
                // Seek from current can be performed immediately
                std::io::SeekFrom::Current(i) => match self.offset.checked_add_signed(i) {
                    Some(n) => self.offset = n,
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Would seek to negative position",
                        ))
                    }
                },
            }
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "A pending operation must complete before seek",
            ))
        }
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<u64>> {
        match ready!(self.pending.poll(cx)) {
            OperationResult::Seek(seek) => {
                if let Ok(n) = seek {
                    self.offset = n;
                }

                Poll::Ready(seek)
            }
            _ => Poll::Ready(Ok(self.offset)),
        }
    }
}

impl AsyncWrite for File {
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
                let Some(handle) = self.handle.clone() else {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "File was closed",
                    )));
                };
                let length = buf.len().min(32768); // write at most 32K

                // Spawn the write future
                let write = self.client.request(Write {
                    handle,
                    offset: self.offset,
                    data: buf[0..length].to_owned().into(),
                });

                self.pending = PendingOperation::Write(Box::pin(async move {
                    match write.await {
                        Ok(()) => Ok(length),
                        Err(status) => Err(std::io::Error::from(status)),
                    }
                }));

                // Try polling immediately
                if let PendingOperation::Write(pending) = &mut self.pending {
                    ready!(pending.as_mut().poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready, adjust the offset according to the number of bytes written
        match result {
            Ok(len) => {
                self.pending = PendingOperation::None;
                self.offset += len as u64;
                std::task::Poll::Ready(Ok(len))
            }
            Err(err) => Poll::Ready(Err(err)),
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
            OperationResult::Write(Err(err)) => Poll::Ready(Err(err)),
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
                let Some(handle) = self.handle.clone() else {
                    return Poll::Ready(Ok(()));
                };

                // Spawn the close future
                let close = self.client.request(Close { handle });

                self.pending = PendingOperation::Close(Box::pin(async move {
                    close.await.map_err(std::io::Error::from)
                }));

                // Try polling immediately
                if let PendingOperation::Close(pending) = &mut self.pending {
                    ready!(pending.as_mut().poll(cx))
                } else {
                    unreachable!()
                }
            }
        };

        // Poll is ready
        Poll::Ready(result)
    }
}

type PendingFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;

enum PendingOperation {
    None,
    Read(PendingFuture<std::io::Result<Bytes>>),
    Seek(PendingFuture<std::io::Result<u64>>),
    Write(PendingFuture<std::io::Result<usize>>),
    Close(PendingFuture<std::io::Result<()>>),
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
    Read(std::io::Result<Bytes>),
    Seek(std::io::Result<u64>),
    Write(std::io::Result<usize>),
    Close(std::io::Result<()>),
}

impl PendingOperation {
    fn poll(&mut self, cx: &mut std::task::Context<'_>) -> Poll<OperationResult> {
        let result = match self {
            PendingOperation::None => OperationResult::None,
            PendingOperation::Read(pending) => {
                OperationResult::Read(ready!(pending.as_mut().poll(cx)))
            }
            PendingOperation::Seek(pending) => {
                OperationResult::Seek(ready!(pending.as_mut().poll(cx)))
            }
            PendingOperation::Write(pending) => {
                OperationResult::Write(ready!(pending.as_mut().poll(cx)))
            }
            PendingOperation::Close(pending) => {
                OperationResult::Close(ready!(pending.as_mut().poll(cx)))
            }
        };

        Poll::Ready(result)
    }
}
