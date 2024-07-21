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

use crate::client::{Error, SftpClientStopping, SftpFuture};
use crate::message::Handle;

use super::Dir;

impl Dir {
    /// Check whether the dir is closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_none()
    }

    /// Close the remote directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn close(&self) -> Result<(), Error>;
    /// ```
    ///
    /// If the directory has already been closed,
    /// The future will return immediately.
    ///
    /// Closing the directory will also stop the underlying sftp client if the client is actually
    /// the last of the SFTP session.
    ///
    /// Pending Read operation is cancelled, if any.
    ///
    /// # Cancel safety
    ///
    /// The closing request is done before returning the future, including the pending operation.
    /// If the future is dropped before completion, it is safe to call it again
    /// to wait that the directory has actually been closed.
    pub fn close(&mut self) -> impl Future<Output = Result<(), Error>> + Drop + Send + Sync + '_ {
        DirClosing::new(self)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        DirClosing::new(self).forget()
    }
}

/// Future for closing a directory
struct DirClosing<'a>(DirClosingState<'a>);

enum DirClosingState<'a> {
    Closing {
        dir: &'a mut Dir,
        handle: Handle,
        pending: SftpFuture,
    },
    Stopping(SftpClientStopping<'a>),
    Closed,
}

impl<'a> DirClosing<'a> {
    fn new(dir: &'a mut Dir) -> Self {
        dir.buffer = None;
        dir.pending = None;
        if let Some(handle) = dir.handle.take() {
            log::trace!("wait for closing");
            let pending = dir.client.close(handle.clone());
            return DirClosing(DirClosingState::Closing {
                dir,
                handle,
                pending,
            });
        };

        let stop = SftpClientStopping::new(&mut dir.client);
        if stop.is_stopped() {
            log::trace!("closed and stopped");
            DirClosing(DirClosingState::Closed)
        } else {
            log::trace!("closed, wait for stopping");
            DirClosing(DirClosingState::Stopping(stop))
        }
    }

    fn forget(mut self) {
        match std::mem::replace(&mut self.0, DirClosingState::Closed) {
            DirClosingState::Closing {
                dir,
                handle: _,
                pending: _,
            } => {
                log::trace!("Directory dropped while not closed");
                SftpClientStopping::new(&mut dir.client).forget()
            }
            DirClosingState::Stopping(stopping) => stopping.forget(),
            DirClosingState::Closed => (),
        }
    }
}

impl Drop for DirClosing<'_> {
    fn drop(&mut self) {
        match &mut self.0 {
            DirClosingState::Closing { dir, handle, .. } => {
                dir.handle = Some(std::mem::take(handle))
            }
            DirClosingState::Stopping { .. } => (),
            DirClosingState::Closed => (),
        }
    }
}

impl Future for DirClosing<'_> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match &mut self.0 {
                DirClosingState::Closing { pending, .. } => {
                    if let Err(err) = ready!(Pin::new(pending).poll(cx)) {
                        return Poll::Ready(Err(err));
                    }

                    // Required to break the borrow chain.
                    // This can only work as DirClosingState is not Drop.
                    // Hence why having DirClosingState instead of directly DirClosing is required in the first place.
                    let DirClosingState::Closing { dir, .. } =
                        std::mem::replace(&mut self.0, DirClosingState::Closed)
                    else {
                        unreachable!()
                    };

                    self.0 = DirClosingState::Stopping(SftpClientStopping::new(&mut dir.client));
                }
                DirClosingState::Stopping(stop) => {
                    ready!(Pin::new(stop).poll(cx));
                    self.0 = DirClosingState::Closed;
                    return Poll::Ready(Ok(()));
                }
                DirClosingState::Closed => return Poll::Ready(Ok(())),
            }
        }
    }
}
