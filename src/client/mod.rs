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
use bytes::{Buf, Bytes};
use russh::{client::Msg, Channel, ChannelMsg};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::{
    message, Attrs, Close, Data, Extended, FSetStat, FStat, Handle, LStat, Message, MkDir, Open,
    OpenDir, PFlags, Path, Read, ReadLink, RealPath, Remove, Rename, RmDir, SetStat, Stat, Status,
    StatusCode, Symlink, Write,
};

mod file;

pub use file::{File, FILE_CLOSED};

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

pub static SFTP_CLIENT_STOPPED: SftpClient = SftpClient::new_stopped();

impl SftpClient {
    pub const fn new_stopped() -> Self {
        Self { inner: None }
    }
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
    ) -> impl Future<Output = R::Output> + Send + Sync + 'static {
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
            Err(StatusCode::Failure.to_message("SFTP client has been stopped".into()))
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

    pub fn is_stopped(&self) -> bool {
        self.inner.is_none()
    }

    pub fn close<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(Close {
            handle: handle.into(),
        })
    }

    pub fn extended<R: Into<Bytes>, D: Into<Bytes>>(
        &self,
        request: R,
        data: D,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
        let request = self.send(Extended {
            request: request.into(),
            data: data.into(),
        });
        async move { Ok(request.await?.data) }
    }

    pub fn fsetstat<H: Into<Handle>, A: Into<Attrs>>(
        &self,
        handle: H,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(FSetStat {
            handle: handle.into(),
            attrs: attrs.into(),
        })
    }

    pub fn fstat<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.send(FStat {
            handle: handle.into(),
        })
    }

    pub fn lstat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.send(LStat { path: path.into() })
    }

    pub fn mkdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.mkdir_with_attrs(path, Attrs::default())
    }

    pub fn mkdir_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(MkDir {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    pub fn open_handle<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.send(Open {
            filename: filename.into(),
            pflags: pflags.into(),
            attrs: attrs.into(),
        })
    }

    pub fn open_with_flags_attrs<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + '_ {
        let request = self.open_handle(filename, pflags, attrs);

        async move { Ok(File::new(self, request.await?)) }
    }

    pub fn open_with_flags<P: Into<Path>, F: Into<PFlags>>(
        &self,
        filename: P,
        pflags: F,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + '_ {
        self.open_with_flags_attrs(filename, pflags, Attrs::default())
    }

    pub fn open_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        filename: P,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + '_ {
        self.open_with_flags_attrs(filename, PFlags::default(), attrs)
    }

    pub fn open<P: Into<Path>>(
        &self,
        filename: P,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + '_ {
        self.open_with_flags_attrs(filename, PFlags::default(), Attrs::default())
    }

    pub fn read<H: Into<Handle>>(
        &self,
        handle: H,
        offset: u64,
        length: u32,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
        let request = self.send(Read {
            handle: handle.into(),
            offset,
            length,
        });

        async move { Ok(request.await?.0) }
    }

    pub fn readdir_handle<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.send(OpenDir { path: path.into() })
    }

    pub fn readlink<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.send(ReadLink { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    pub fn realpath<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.send(RealPath { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    pub fn remove<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(Remove { path: path.into() })
    }

    pub fn rename<O: Into<Path>, N: Into<Path>>(
        &self,
        old_path: O,
        new_path: N,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(Rename {
            old_path: old_path.into(),
            new_path: new_path.into(),
        })
    }

    pub fn rmdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(RmDir { path: path.into() })
    }

    pub fn setstat<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(SetStat {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    pub fn stat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.send(Stat { path: path.into() })
    }

    pub fn symlink<L: Into<Path>, T: Into<Path>>(
        &self,
        link_path: L,
        target_path: T,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(Symlink {
            link_path: link_path.into(),
            target_path: target_path.into(),
        })
    }

    pub fn write<H: Into<Handle>, D: Into<Bytes>>(
        &self,
        handle: H,
        offset: u64,
        data: D,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.send(Write {
            handle: handle.into(),
            offset,
            data: Data(data.into()),
        })
    }
}

impl Drop for SftpClient {
    fn drop(&mut self) {
        futures::executor::block_on(self.stop());
    }
}

impl std::fmt::Debug for SftpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SftpClient")
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
send_impl!(Extended -> ExtendedReply);
