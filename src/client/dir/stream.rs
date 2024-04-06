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

use std::task::ready;

use crate::{ClientError, NameEntry, ReadDir, Status, StatusCode};

use super::Dir;

impl futures::Stream for Dir {
    type Item = Result<NameEntry, ClientError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // If end of file reached, return None
        let Some(buffer) = &mut self.buffer else {
            return std::task::Poll::Ready(None);
        };

        // If still some entries in the buffer, get next
        if let Some(entry) = buffer.0.pop() {
            return std::task::Poll::Ready(Some(Ok(entry)));
        }

        let result = match &mut self.pending {
            Some(pending) => {
                ready!(pending.as_mut().poll(cx))
            }
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

        // Polling has finished, resetting pending
        self.pending = None;

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
