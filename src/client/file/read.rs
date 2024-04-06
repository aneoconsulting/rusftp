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

use std::{task::ready, task::Poll};

use bytes::Bytes;

use crate::{ClientError, Status, StatusCode};

use super::{File, OperationResult, PendingOperation};

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
                        Err(ClientError::Sftp(Status {
                            code: StatusCode::Eof,
                            ..
                        })) => Ok(Bytes::default()),
                        Err(status) => Err(status.into()),
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
