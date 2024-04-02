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
use std::sync::Arc;
use std::task::ready;

use async_trait::async_trait;
use bytes::{Buf, Bytes};
use russh::{client::Msg, Channel, ChannelMsg};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::{
    message, Attrs, Close, Data, Extended, FSetStat, FStat, Handle, LStat, Message, MkDir, Name,
    Open, OpenDir, PFlags, Path, Read, ReadDir, ReadLink, RealPath, Remove, Rename, RmDir, SetStat,
    Stat, Status, StatusCode, Symlink, Write,
};

mod dir;
mod file;

pub use dir::{Dir, DIR_CLOSED};
pub use file::{File, FILE_CLOSED};

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
    pub async fn new<T: ToSftpChannel>(ssh: T) -> Result<Self, std::io::Error> {
        Self::with_channel(ssh.to_sftp_channel().await?).await
    }

    /// Creates a new client from a [`russh::Channel<Msg>`].
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
                        // If received null, the commands channel has been closed
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
                        // If received null, the SSH channel has been closed
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
                                        code: StatusCode::BadMessage,
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
            commands: Some(tx),
            request_processor: Some(Arc::new(request_processor)),
        })
    }

    /// Send a SFTP request, and return its reply.
    ///
    /// In case a reply is the status `OK`, the empty tuple is returned instead: `()`.
    ///
    /// You can implement your own extension requests by implementing [`SftpRequest`].
    pub fn request<R: SftpRequest>(
        &self,
        request: R,
    ) -> impl Future<Output = R::Reply> + Send + Sync + 'static {
        let sent = if let Some(commands) = &self.commands {
            let msg = request.to_requets_message();

            if let Message::Status(status) = msg {
                if status.is_ok() {
                    Err(StatusCode::BadMessage
                        .to_message("Tried to send an OK status message to the server".into()))
                } else {
                    Err(Message::Status(status))
                }
            } else {
                let (tx, rx) = oneshot::channel();
                match commands.send((msg, tx)) {
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
            R::from_reply_message(msg)
        }
    }

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
        if let Some(a) = self.commands.take() {
            std::mem::drop(a)
        }

        // Try to unwrap the join handle into the future
        // This can happen only if the current client is the last client of the session
        if let Some(request_processor) = self.request_processor.take() {
            if let Ok(request_processor) = Arc::try_unwrap(request_processor) {
                return SftpClientStopping {
                    client: self,
                    request_processor: Some(request_processor),
                };
            }
        }

        // If the current client is not the last of the session, nothing to wait
        SftpClientStopping {
            client: self,
            request_processor: None,
        }
    }

    /// Check whether the client is stopped.
    pub fn is_stopped(&self) -> bool {
        self.commands.is_none()
    }

    /// Close an opened file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or the directory (convertible to [`Handle`])
    pub fn close<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Close {
            handle: handle.into(),
        })
    }

    /// Send an extended request.
    ///
    /// # Arguments
    ///
    /// * `request` - Extended-request name (format: `name@domain`, convertible to [`Bytes`])
    /// * `data` - Specific data needed by the extension to intrepret the request (convertible to [`Bytes`])
    pub fn extended<R: Into<Bytes>, D: Into<Bytes>>(
        &self,
        request: R,
        data: D,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
        let request = self.request(Extended {
            request: request.into(),
            data: data.into(),
        });
        async move { Ok(request.await?.data) }
    }

    /// Change the attributes (metadata) of an open file or directory.
    ///
    /// This operation is used for operations such as changing the ownership,
    /// permissions or access times, as well as for truncating a file.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or directory to change the attributes (convertible to [`Handle`])
    /// * `attrs` - New attributes to apply (convertible to [`Attrs`])
    pub fn fsetstat<H: Into<Handle>, A: Into<Attrs>>(
        &self,
        handle: H,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(FSetStat {
            handle: handle.into(),
            attrs: attrs.into(),
        })
    }

    /// Read the attributes (metadata) of an open file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open file or directory (convertible to [`Handle`])
    pub fn fstat<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(FStat {
            handle: handle.into(),
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links are followed.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file, directory, or symbolic link (convertible to [`Path`])
    pub fn lstat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(LStat { path: path.into() })
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located (convertible to [`Path`])
    pub fn mkdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.mkdir_with_attrs(path, Attrs::default())
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located (convertible to [`Path`])
    /// * `attrs` - Default attributes to apply to the newly created directory (convertible to [`Attrs`])
    pub fn mkdir_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(MkDir {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns an [`Handle`](struct@crate::Handle) for the file specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_handle<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.request(Open {
            filename: filename.into(),
            pflags: pflags.into(),
            attrs: attrs.into(),
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_with_flags_attrs<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        let request = self.open_handle(filename, pflags, attrs);
        let client = self.clone();

        async move { Ok(File::new(client, request.await?)) }
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    pub fn open_with_flags<P: Into<Path>, F: Into<PFlags>>(
        &self,
        filename: P,
        pflags: F,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, pflags, Attrs::default())
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        filename: P,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, PFlags::default(), attrs)
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    pub fn open<P: Into<Path>>(
        &self,
        filename: P,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, PFlags::default(), Attrs::default())
    }

    /// Open a directory for listing.
    ///
    /// Once the directory has been successfully opened, files (and directories)
    /// contained in it can be listed using `readdir_handle`.
    ///
    /// Returns an [`Handle`] for the directory specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open (convertible to [`Path`])
    pub fn opendir_handle<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.request(OpenDir { path: path.into() })
    }

    /// Open a directory for listing.
    ///
    /// Returns a [`Dir`] for the directory specified.
    /// It implements [`Stream<Item = Result<NameEntry, ...>>`](futures::stream::Stream).
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open (convertible to [`Path`])
    pub fn opendir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Dir, Status>> + Send + Sync + 'static {
        let request = self.request(OpenDir { path: path.into() });
        let client = self.clone();

        async move { Ok(Dir::new(client, request.await?)) }
    }

    /// Read a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to read from (convertible to [`Handle`])
    /// * `offset`: Byte offset where the read should start
    /// * `length`: Number of bytes to read
    pub fn read<H: Into<Handle>>(
        &self,
        handle: H,
        offset: u64,
        length: u32,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
        let request = self.request(Read {
            handle: handle.into(),
            offset,
            length,
        });

        async move { Ok(request.await?.0) }
    }

    /// Read a directory listing.
    ///
    /// Each `readdir_handle` returns one or more file names with full file attributes for each file.
    /// The client should call `readdir_handle` repeatedly until it has found the file it is looking for
    /// or until the server responds with a [`Status`] message indicating an error
    /// (normally `EOF` if there are no more files in the directory).
    /// The client should then close the handle using `close`.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the open directory (convertible to [`Handle`])
    pub fn readdir_handle<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<Name, Status>> + Send + Sync + 'static {
        self.request(ReadDir {
            handle: handle.into(),
        })
    }

    /// Read a directory listing.
    ///
    /// If you need an asynchronous [`Stream`](futures::stream::Stream), you can use `opendir()` instead
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to list (convertible to [`Path`])
    pub fn readdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Name, Status>> + Send + Sync + 'static {
        let dir = self.request(OpenDir { path: path.into() });
        let client = self.clone();
        let mut entries = Name::default();

        async move {
            let handle = dir.await?;

            loop {
                match client.readdir_handle(handle.clone()).await {
                    Ok(mut chunk) => entries.0.append(&mut chunk.0),
                    Err(Status {
                        code: StatusCode::Eof,
                        ..
                    }) => break,
                    Err(err) => {
                        _ = client.close(handle).await;
                        return Err(err);
                    }
                }
            }

            client.close(handle).await?;
            Ok(entries)
        }
    }

    /// Read the target of a symbolic link.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the symbolic link to read (convertible to [`Path`])
    pub fn readlink<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.request(ReadLink { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    /// Canonicalize a path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to canonicalize (convertible to [`Path`])
    pub fn realpath<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.request(RealPath { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    /// Remove a file.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file to remove (convertible to [`Path`])
    pub fn remove<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Remove { path: path.into() })
    }

    /// Rename/move a file or a directory.
    ///
    /// # Arguments
    ///
    /// * `old_path`: Current path of the file or directory to rename/move (convertible to [`Path`])
    /// * `new_path`: New path where the file or directory will be moved to (convertible to [`Path`])
    pub fn rename<O: Into<Path>, N: Into<Path>>(
        &self,
        old_path: O,
        new_path: N,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Rename {
            old_path: old_path.into(),
            new_path: new_path.into(),
        })
    }

    /// Remove an existing directory.
    ///
    /// An error will be returned if no directory with the specified path exists,
    /// or if the specified directory is not empty, or if the path specified
    /// a file system object other than a directory.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to remove (convertible to [`Path`])
    pub fn rmdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(RmDir { path: path.into() })
    }

    /// Change the attributes (metadata) of a file or directory.
    ///
    /// This request is used for operations such as changing the ownership,
    /// permissions or access times, as well as for truncating a file.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory to change the attributes (convertible to [`Path`])
    /// * `attrs`: New attributes to apply (convertible to [`Attrs`])
    pub fn setstat<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(SetStat {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links *are not* followed.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory (convertible to [`Path`])
    pub fn stat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(Stat { path: path.into() })
    }

    /// Create a symbolic link.
    ///
    /// # Arguments
    ///
    /// * `link_path`: Path name of the symbolic link to be created (convertible to [`Path`])
    /// * `target_path`: Target of the symbolic link (convertible to [`Path`])
    pub fn symlink<L: Into<Path>, T: Into<Path>>(
        &self,
        link_path: L,
        target_path: T,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Symlink {
            link_path: link_path.into(),
            target_path: target_path.into(),
        })
    }

    /// Write to a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to write to (convertible to [`Handle`])
    /// * `offset`: Byte offset where the write should start
    /// * `data`: Bytes to be written to the file
    pub fn write<H: Into<Handle>, D: Into<Bytes>>(
        &self,
        handle: H,
        offset: u64,
        data: D,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Write {
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

/// Future for stopping a SftpClient
struct SftpClientStopping<'a> {
    client: &'a mut SftpClient,
    request_processor: Option<JoinHandle<()>>,
}

impl Future for SftpClientStopping<'_> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if let Some(request_processor) = &mut self.request_processor {
            tokio::pin!(request_processor);
            _ = ready!(request_processor.poll(cx));
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

/// Convert the object to a SSH channel
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

/// Defines how a request is performed and how the reply is interpreted.
pub trait SftpRequest {
    /// Decoded type of the reply
    ///
    /// It is recommended to return a [`Result<T, Status>`]
    type Reply;

    /// Convert the request type into an actual SFTP message
    fn to_requets_message(self) -> Message;

    /// Convert the reply message into the decoded Reply type
    ///
    /// The message can contain an Error status.
    /// If so, it is recommended to return the error as-is.
    fn from_reply_message(msg: Message) -> Self::Reply;
}

impl SftpRequest for Message {
    type Reply = Message;

    fn to_requets_message(self) -> Message {
        self
    }

    fn from_reply_message(msg: Message) -> Self::Reply {
        msg
    }
}

macro_rules! send_impl {
    ($input:ident) => {
        impl SftpRequest for message::$input {
            type Reply = Result<(), message::Status>;

            fn to_requets_message(self) -> Message {
                self.into()
            }

            fn from_reply_message(msg: Message) -> Self::Reply {
                match msg {
                    Message::Status(status) => status.to_result(()),
                    _ => Err(message::StatusCode::BadMessage
                        .to_status("Expected a status".into())),
                }
            }
        }
    };
    ($input:ident -> $output:ident) => {
        impl SftpRequest for message::$input {
            type Reply = Result<message::$output, message::Status>;

            fn to_requets_message(self) -> Message {
                self.into()
            }

            fn from_reply_message(msg: Message) -> Self::Reply {
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
