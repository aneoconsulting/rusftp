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
    sync::Arc,
    task::{ready, Poll},
};

use crate::{client::SftpClientStopping, ClientError, Handle};

use super::{File, PendingOperation};

impl File {
    /// Check whether the file is closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_none()
    }

    /// Close the remote file.
    ///
    /// Close the remote file if current [`File`] is the last reference to the remote file.
    /// If the file is not the last one reference, or the file has already been closed,
    /// The future will return immediately.
    ///
    /// Closing the file will also stop the underlying sftp client if the client is actually
    /// the last of the SFTP session.
    ///
    /// Pending Read/Write/Seek operation is cancelled, if any.
    ///
    /// # Cancel safety
    ///
    /// The closing request is done before returning the future, including the pending operation.
    /// If the future is dropped before completion, it is safe to call it again
    /// to wait that the file has actually been closed.
    pub fn close(
        &mut self,
    ) -> impl Future<Output = Result<(), ClientError>> + Drop + Send + Sync + '_ {
        FileClosing::new(self)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        match FileClosing::new(self) {
            FileClosing(FileClosingState::Closed) => (),
            future => _ = futures::executor::block_on(future),
        }
    }
}

/// Future for closing a file
struct FileClosing<'a>(FileClosingState<'a>);

enum FileClosingState<'a> {
    Closing {
        file: &'a mut File,
        handle: Handle,
        pending: Pin<Box<dyn Future<Output = Result<(), ClientError>> + Send + Sync + 'static>>,
    },
    Stopping(SftpClientStopping<'a>),
    Closed,
}

impl<'a> FileClosing<'a> {
    fn new(file: &'a mut File) -> Self {
        file.pending = PendingOperation::None;
        if let Some(handle) = file.handle.take() {
            if let Some(handle) = Arc::into_inner(handle) {
                let pending = file.client.close(handle.clone());
                return FileClosing(FileClosingState::Closing {
                    file,
                    handle,
                    pending: Box::pin(pending),
                });
            }
        };

        let stop = SftpClientStopping::new(&mut file.client);
        if stop.is_stopped() {
            FileClosing(FileClosingState::Closed)
        } else {
            FileClosing(FileClosingState::Stopping(stop))
        }
    }
}

impl Drop for FileClosing<'_> {
    fn drop(&mut self) {
        match &mut self.0 {
            FileClosingState::Closing { file, handle, .. } => {
                file.handle = Some(Arc::new(std::mem::take(handle)))
            }
            FileClosingState::Stopping { .. } => (),
            FileClosingState::Closed => (),
        }
    }
}

impl Future for FileClosing<'_> {
    type Output = Result<(), ClientError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match &mut self.0 {
                FileClosingState::Closing { pending, .. } => {
                    if let Err(err) = ready!(pending.as_mut().poll(cx)) {
                        return Poll::Ready(Err(err));
                    }

                    // Required to break the borrow chain.
                    // This can only work as FileClosingState is not Drop.
                    // Hence why having FileClosingState instead of directly FileClosing is required in the first place.
                    let FileClosingState::Closing { file, .. } =
                        std::mem::replace(&mut self.0, FileClosingState::Closed)
                    else {
                        unreachable!()
                    };

                    self.0 = FileClosingState::Stopping(SftpClientStopping::new(&mut file.client));
                }
                FileClosingState::Stopping(stop) => {
                    ready!(Pin::new(stop).poll(cx));
                    self.0 = FileClosingState::Closed;
                    return Poll::Ready(Ok(()));
                }
                FileClosingState::Closed => return Poll::Ready(Ok(())),
            }
        }
    }
}