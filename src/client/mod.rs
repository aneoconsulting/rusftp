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

use std::sync::Arc;

use async_trait::async_trait;
use russh::{client::Msg, Channel, ChannelMsg};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::{message, Message, StatusCode};

mod commands;
mod dir;
mod error;
mod file;
mod receiver;
mod request;
mod stop;

pub use dir::{Dir, DIR_CLOSED};
pub use error::ClientError;
pub use file::{File, FILE_CLOSED};
pub use request::SftpRequest;

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
/// let sftp = rusftp::SftpClient::new(&ssh).await?;
/// println!("stat '.': {:?}", sftp.stat(".").await?);
/// # Ok(())
/// # }
/// ```
#[derive(Default, Clone)]
pub struct SftpClient {
    commands: Option<mpsc::UnboundedSender<(Message, oneshot::Sender<Message>)>>,
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
    /// `ssh` can be a [`russh::Channel<Msg>`])
    /// or a [`russh::client::Handler`].
    /// In case of the handler, it can be moved or borrowed.
    pub async fn new<T: ToSftpChannel>(ssh: T) -> Result<Self, ClientError> {
        Self::with_channel(ssh.to_sftp_channel().await?).await
    }

    /// Creates a new client from a [`russh::Channel<Msg>`].
    pub async fn with_channel(mut channel: Channel<Msg>) -> Result<Self, ClientError> {
        // Start SFTP subsystem
        channel.request_subsystem(false, "sftp").await?;

        // Init SFTP handshake
        let init_message = Message::Init(message::Init {
            version: 3,
            extensions: Default::default(),
        });
        let init_frame = init_message.encode(0)?;
        channel.data(init_frame.as_ref()).await?;

        // Check handshake response
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    match Message::decode(data.as_ref()) {
                        // Valid response: continue
                        Ok((
                            _,
                            Message::Version(message::Version {
                                version: 3,
                                extensions: _,
                            }),
                        )) => break,

                        // Invalid responses: abort
                        Ok((_, Message::Version(_))) => {
                            return Err(StatusCode::BadMessage
                                .to_status("Invalid sftp version".into())
                                .into());
                        }
                        Ok(_) => {
                            return Err(StatusCode::BadMessage
                                .to_status("Bad SFTP init".into())
                                .into());
                        }
                        Err(err) => {
                            return Err(err.into());
                        }
                    }
                }
                // Unrelated event has been received, looping is required
                Some(_) => (),
                // Channel has been closed
                None => {
                    return Err(StatusCode::BadMessage
                        .to_status("Failed to start SFTP subsystem".into())
                        .into());
                }
            }
        }

        let (receiver, tx) = receiver::Receiver::new(channel);
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
pub trait ToSftpChannel {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, ClientError>;
}

#[async_trait]
impl ToSftpChannel for Channel<Msg> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, ClientError> {
        Ok(self)
    }
}

#[async_trait]
impl<H: russh::client::Handler> ToSftpChannel for &russh::client::Handle<H> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, ClientError> {
        self.channel_open_session().await.map_err(Into::into)
    }
}

#[async_trait]
impl<H: russh::client::Handler> ToSftpChannel for russh::client::Handle<H> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, ClientError> {
        (&self).to_sftp_channel().await
    }
}
