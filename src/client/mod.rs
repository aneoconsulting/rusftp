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

use std::collections::HashMap;
use std::future::Future;

use async_trait::async_trait;
use bytes::Buf;
use russh::client::Msg;
use russh::Channel;
use russh::ChannelMsg;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::StatusCode;
use crate::{message, Message};

/// SFTP client
///
/// ```no_run
/// # use std::sync::Arc;
/// # use async_trait::async_trait;
/// struct Handler;
///
/// #[async_trait]
/// impl russh::client::Handler for Handler {
///     type Error = russh::Error;
///     // ...
/// }
///
/// # async fn dummy() -> Result<(), Box<dyn std::error::Error>> {
/// let config = Arc::new(russh::client::Config::default());
/// let mut ssh = russh::client::connect(config, ("localhost", 2222), Handler).await.unwrap();
/// ssh.authenticate_password("user", "pass").await.unwrap();
///
/// let sftp = rusftp::SftpClient::new(&ssh).await.unwrap();
/// let stat = sftp.send(rusftp::Stat{path: ".".into()}).await.unwrap();
/// println!("stat '.': {stat:?}");
/// # Ok(())
/// # }
/// ```
pub struct SftpClient {
    inner: Option<SftpClientInner>,
}

struct SftpClientInner {
    commands: mpsc::UnboundedSender<(Message, oneshot::Sender<Message>)>,
    request_processor: JoinHandle<()>,
}

impl SftpClient {
    pub async fn new<T: ToSftpChannel>(ssh: T) -> Result<Self, std::io::Error> {
        Self::with_channel(ssh.to_sftp_channel().await?).await
    }
    pub async fn with_channel(mut channel: Channel<Msg>) -> Result<Self, std::io::Error> {
        // Start SFTP subsystem
        match channel.request_subsystem(false, "sftp").await {
            Ok(_) => (),
            Err(russh::Error::IO(err)) => {
                return Err(err);
            }
            Err(err) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, err));
            }
        }

        // Init SFTP handshake
        let init_message = Message::Init(message::Init {
            version: 3,
            extensions: Default::default(),
        });
        let init_frame = init_message
            .encode(0)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        channel.data(init_frame.as_ref()).await.map_err(|e| {
            if let russh::Error::IO(io_err) = e {
                io_err
            } else {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            }
        })?;

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
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Invalid sftp version",
                            ));
                        }
                        Ok(_) => {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Bad SFTP init",
                            ));
                        }
                        Err(err) => {
                            return Err(std::io::Error::new(std::io::ErrorKind::Other, err));
                        }
                    }
                }
                // Unrelated event has been received, looping is required
                Some(_) => (),
                // Channel has been closed
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Failed to start SFTP subsystem",
                    ));
                }
            }
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<(Message, oneshot::Sender<Message>)>();

        let request_processor = tokio::spawn(async move {
            let mut onflight = HashMap::<u32, oneshot::Sender<Message>>::new();
            let mut id = 0u32;

            loop {
                tokio::select! {
                    // New request to send
                    request = rx.recv() => {
                        let Some((message, tx)) = request else {
                            _ = channel.close().await;
                            break;
                        };

                        id += 1;
                        //eprintln!("Request #{id}: {message:?}");
                        match message.encode(id) {
                            Ok(frame) => {
                                if let Err(err) = channel.data(frame.as_ref()).await {
                                    _ = tx.send(err.into());
                                } else {
                                    onflight.insert(id, tx);
                                }
                            }
                            Err(err) => {
                                _ = tx.send(err.into());
                            }
                        }
                    },

                    // New response received
                    response = channel.wait() => {
                        let Some(ChannelMsg::Data { data }) = response else {
                            rx.close();
                            break;
                        };

                        match Message::decode(data.as_ref()) {
                            Ok((id, message)) => {
                                //eprintln!("Response #{id}: {message:?}");
                                if let Some(tx) = onflight.remove(&id) {
                                    _ = tx.send(message);
                                } else {
                                    eprintln!("SFTP Error: Received a reply with an invalid id");
                                }
                            }
                            Err(err) => {
                                if let Some(mut buf) = data.as_ref().get(5..9){

                                let id = buf.get_u32();
                                if let Some(tx) = onflight.remove(&id) {
                                    _ = tx.send(Message::Status(crate::Status {
                                        code: StatusCode::BadMessage as u32,
                                        error: err.to_string().into(),
                                        language: "en".into(),
                                    }));
                                } else {
                                    eprintln!("SFTP Error: Received a reply with an invalid id");
                                }
                                } else {
                                    eprintln!("SFTP Error: Received a bad reply");
                                }
                            }
                        }
                    },
                }
            }
        });

        Ok(Self {
            inner: Some(SftpClientInner {
                commands: tx,
                request_processor,
            }),
        })
    }

    pub fn send<R: SftpSend>(
        &self,
        request: R,
    ) -> impl Future<Output = R::Output> + Send + 'static {
        let sent = if let Some(inner) = &self.inner {
            let msg = request.to_message();

            if let Message::Status(status) = msg {
                if status.is_ok() {
                    Err(StatusCode::BadMessage
                        .to_message("Tried to send an OK status message to the server".into()))
                } else {
                    Err(Message::Status(status))
                }
            } else {
                let (tx, rx) = oneshot::channel();
                match inner.commands.send((msg, tx)) {
                    Ok(()) => Ok(rx),
                    Err(err) => Err(StatusCode::Failure.to_message(err.to_string().into())),
                }
            }
        } else {
            Err(StatusCode::Failure.to_message("SFTP client has been closed".into()))
        };

        async move {
            let msg = match sent {
                Ok(rx) => rx.await.unwrap_or(
                    StatusCode::Failure.to_message("Could not get reply from SFTP client".into()),
                ),
                Err(err) => err,
            };
            R::from_message(msg)
        }
    }

    pub async fn stop(&mut self) {
        if let Some(inner) = std::mem::take(&mut self.inner) {
            std::mem::drop(inner.commands);
            _ = inner.request_processor.await;
        }
    }
}

impl Drop for SftpClient {
    fn drop(&mut self) {
        futures::executor::block_on(self.stop());
    }
}

#[async_trait]
pub trait ToSftpChannel {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, std::io::Error>;
}

#[async_trait]
impl ToSftpChannel for Channel<Msg> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, std::io::Error> {
        Ok(self)
    }
}

#[async_trait]
impl<H: russh::client::Handler> ToSftpChannel for &russh::client::Handle<H> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, std::io::Error> {
        match self.channel_open_session().await {
            Ok(channel) => Ok(channel),
            Err(russh::Error::IO(err)) => Err(err),
            Err(err) => Err(std::io::Error::new(std::io::ErrorKind::Other, err)),
        }
    }
}

#[async_trait]
impl<H: russh::client::Handler> ToSftpChannel for russh::client::Handle<H> {
    async fn to_sftp_channel(self) -> Result<Channel<Msg>, std::io::Error> {
        (&self).to_sftp_channel().await
    }
}

pub trait SftpSend {
    type Output;
    fn to_message(self) -> Message;
    fn from_message(msg: Message) -> Self::Output;
}

impl SftpSend for Message {
    type Output = Message;

    fn to_message(self) -> Message {
        self
    }

    fn from_message(msg: Message) -> Self::Output {
        msg
    }
}

macro_rules! send_impl {
    ($input:ident) => {
        impl SftpSend for message::$input {
            type Output = Result<(), message::Status>;

            fn to_message(self) -> Message {
                self.into()
            }

            fn from_message(msg: Message) -> Self::Output {
                match msg {
                    Message::Status(status) => status.to_result(()),
                    _ => Err(message::StatusCode::BadMessage
                        .to_status("Expected a status".into())),
                }
            }
        }
    };
    ($input:ident -> $output:ident) => {
        impl SftpSend for message::$input {
            type Output = Result<message::$output, message::Status>;

            fn to_message(self) -> Message {
                self.into()
            }

            fn from_message(msg: Message) -> Self::Output {
                match msg {
                    Message::$output(response) => Ok(response),
                    Message::Status(status) => Err(status),
                    _ => Err(message::StatusCode::BadMessage
                        .to_status(std::stringify!(Expected a $output or a status).into())),
                }
            }
        }
    };
}

send_impl!(Open -> Handle);
send_impl!(Close);
send_impl!(Read -> Data);
send_impl!(Write);
send_impl!(LStat -> Attrs);
send_impl!(FStat -> Attrs);
send_impl!(SetStat);
send_impl!(FSetStat);
send_impl!(OpenDir -> Handle);
send_impl!(ReadDir -> Name);
send_impl!(Remove);
send_impl!(MkDir);
send_impl!(RmDir);
send_impl!(RealPath -> Name);
send_impl!(Stat -> Attrs);
send_impl!(Rename);
send_impl!(ReadLink -> Name);
send_impl!(Symlink);
