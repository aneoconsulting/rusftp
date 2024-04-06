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
    pin::Pin,
    task::{ready, Poll},
};

use super::{File, OperationResult, PendingOperation};

impl tokio::io::AsyncSeek for File {
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
