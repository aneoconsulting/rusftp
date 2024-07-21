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

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::ready;

use tokio::task::JoinHandle;

use crate::client::SftpClient;

impl SftpClient {
    /// Stop the SFTP client.
    ///
    /// Close the SFTP session if the client is the last one of the session.
    /// If the client is not the last one, or the client was already stopped,
    /// The future will return immediately.
    ///
    /// # Cancel safety
    ///
    /// The stopping request is done before returning the future.
    /// If the future is dropped before completion, it is safe to call it again
    /// to wait that the client has actually stopped.
    pub fn stop(&mut self) -> impl Future<Output = ()> + Drop + Send + Sync + '_ {
        SftpClientStopping::new(self)
    }

    /// Check whether the client is stopped.
    pub fn is_stopped(&self) -> bool {
        self.commands.is_none()
    }
}

impl Drop for SftpClient {
    fn drop(&mut self) {
        SftpClientStopping::new(self).forget();
    }
}

/// Future for stopping a SftpClient
pub(super) struct SftpClientStopping<'a> {
    client: &'a mut SftpClient,
    request_processor: Option<JoinHandle<()>>,
}

impl<'a> SftpClientStopping<'a> {
    pub(super) fn new(client: &'a mut SftpClient) -> SftpClientStopping<'_> {
        client.commands = None;

        // Try to unwrap the join handle into the future
        // This can happen only if the current client is the last client of the session
        if let Some(request_processor) = client.request_processor.take() {
            if let Some(request_processor) = Arc::into_inner(request_processor) {
                log::trace!("Waiting for client to stop");
                return SftpClientStopping {
                    client,
                    request_processor: Some(request_processor),
                };
            }

            log::trace!("Client still running");
        } else {
            log::trace!("stopped");
        }

        // If the current client is not the last of the session, nothing to wait
        SftpClientStopping {
            client,
            request_processor: None,
        }
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.request_processor.is_none()
    }

    pub(super) fn forget(mut self) {
        if self.request_processor.take().is_some() {
            log::trace!("SftpClient dropped while not stopped");
        }
    }
}

impl Future for SftpClientStopping<'_> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(request_processor) = &mut self.request_processor {
            _ = ready!(Pin::new(request_processor).poll(cx));
        }

        // Stopping has succeeded, so we can drop the JoinHandle
        self.request_processor = None;
        std::task::Poll::Ready(())
    }
}

impl Drop for SftpClientStopping<'_> {
    fn drop(&mut self) {
        // If the stopping request was processing, we need to put back the JoinHandle
        // into the client in case we need to await its stopping again
        if let Some(request_processor) = self.request_processor.take() {
            self.client.request_processor = Some(Arc::new(request_processor))
        }
    }
}
