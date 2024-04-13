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

//! SFTP client module.
//!
//! See [`SftpClient`]

use std::sync::Arc;

use async_trait::async_trait;
use russh::ChannelStream;
use russh::{client::Msg, Channel};
use tokio::io::AsyncWrite;
use tokio::task::JoinHandle;
use tokio::{io::AsyncRead, sync::mpsc};

use crate::message::{Init, Message, StatusCode, Version};

mod commands;
mod dir;
mod error;
mod file;
mod receiver;
mod request;
mod stop;

pub use dir::{Dir, DIR_CLOSED};
pub use error::Error;
pub use file::{File, FILE_CLOSED};
pub use request::{SftpFuture, SftpReply, SftpRequest};
use stop::SftpClientStopping;

/// SFTP client
///
/// # Example
///
/// ```no_run
/// # use std::sync::Arc;
/// # use async_trait::async_trait;
/// # struct ClientHandler;
/// #
/// # #[async_trait]
/// # impl russh::client::Handler for ClientHandler {
/// #    type Error = russh::Error;
/// #    // ...
/// # }
/// #
/// # async fn dummy() -> Result<(), Box<dyn std::error::Error>> {
/// /// Create ssh client
/// let config = Arc::new(russh::client::Config::default());
/// let mut ssh = russh::client::connect(config, ("localhost", 2222), ClientHandler).await?;
/// ssh.authenticate_password("user", "pass").await?;
///
/// // Create SFTP client
/// let sftp = rusftp::client::SftpClient::new(&ssh).await?;
/// println!("stat '.': {:?}", sftp.stat(".").await?);
/// # Ok(())
/// # }
/// ```
#[derive(Default, Clone)]
pub struct SftpClient {
    commands: Option<mpsc::UnboundedSender<receiver::Request>>,
    request_processor: Option<Arc<JoinHandle<()>>>,
}

pub static SFTP_CLIENT_STOPPED: SftpClient = SftpClient::new_stopped();

impl SftpClient {
    /// Creates a stopped client.
    /// This client cannot be opened.
    pub const fn new_stopped() -> Self {
        Self {
            commands: None,
            request_processor: None,
        }
    }

    /// Creates a new client from a ssh connection.
    ///
    /// `ssh` can be a [`russh::Channel<Msg>`]
    /// or a [`russh::client::Handler`].
    /// In case of the handler, it can be moved or borrowed.
    pub async fn new<T: IntoSftpStream>(ssh: T) -> Result<Self, Error> {
        Self::with_stream(ssh.into_sftp_stream().await?).await
    }

    /// Creates a new client from a stream ([`AsyncRead`] + [`AsyncWrite`]).
    pub async fn with_stream(
        mut stream: impl AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        // Init SFTP handshake
        receiver::write_msg(
            &mut stream,
            Message::Init(Init {
                version: 3,
                extensions: Default::default(),
            }),
            3,
        )
        .await?;

        match receiver::read_msg(&mut stream).await? {
            // Valid response: continue
            (
                _,
                Message::Version(Version {
                    version: 3,
                    extensions: _,
                }),
            ) => (),

            // Invalid responses: abort
            (_, Message::Version(_)) => {
                return Err(StatusCode::BadMessage
                    .to_status("Invalid sftp version")
                    .into());
            }
            _ => {
                return Err(StatusCode::BadMessage.to_status("Bad SFTP init").into());
            }
        }

        let (receiver, tx) = receiver::Receiver::new(stream);
        let request_processor = tokio::spawn(receiver.run());

        Ok(Self {
            commands: Some(tx),
            request_processor: Some(Arc::new(request_processor)),
        })
    }
}

impl std::fmt::Debug for SftpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SftpClient")
    }
}

/// Convert the object to a SSH channel
#[async_trait]
pub trait IntoSftpStream {
    type Stream: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static;
    async fn into_sftp_stream(self) -> Result<Self::Stream, Error>;
}

#[async_trait]
impl IntoSftpStream for ChannelStream<Msg> {
    type Stream = ChannelStream<Msg>;
    async fn into_sftp_stream(self) -> Result<Self::Stream, Error> {
        Ok(self)
    }
}

#[async_trait]
impl IntoSftpStream for Channel<Msg> {
    type Stream = ChannelStream<Msg>;
    async fn into_sftp_stream(self) -> Result<Self::Stream, Error> {
        // Start SFTP subsystem
        self.request_subsystem(false, "sftp").await?;

        Ok(self.into_stream())
    }
}

#[async_trait]
impl<H: russh::client::Handler> IntoSftpStream for &russh::client::Handle<H> {
    type Stream = ChannelStream<Msg>;
    async fn into_sftp_stream(self) -> Result<Self::Stream, Error> {
        self.channel_open_session().await?.into_sftp_stream().await
    }
}

#[async_trait]
impl<H: russh::client::Handler> IntoSftpStream for russh::client::Handle<H> {
    type Stream = ChannelStream<Msg>;
    async fn into_sftp_stream(self) -> Result<Self::Stream, Error> {
        (&self).into_sftp_stream().await
    }
}
