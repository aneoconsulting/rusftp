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
use std::task::{ready, Poll};

use tokio::sync::oneshot;

use crate::client::{Error, SftpClient};
use crate::message::{self, Message, Status, StatusCode};

impl SftpClient {
    /// Send a SFTP request, and return its reply.
    ///
    /// In case a reply is the status `OK`, the empty tuple is returned instead: `()`.
    ///
    /// You can implement your own extension requests by implementing [`SftpRequest`] and  [`SftpReply`].
    ///
    /// # Arguments
    ///
    /// * `request` - SFTP Request to be sent
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    ///
    /// # Implementation equivalent examples
    ///
    /// ```ignore
    /// async fn request(&self, request: Message) -> Result<Message, Error>;
    /// async fn request(&self, request: Open) -> Result<Handle, Error>;
    /// async fn request(&self, request: Close) -> Result<(), Error>;
    /// async fn request(&self, request: Read) -> Result<Data, Error>;
    /// async fn request(&self, request: Write) -> Result<(), Error>;
    /// async fn request(&self, request: LStat) -> Result<Attrs, Error>;
    /// async fn request(&self, request: FStat) -> Result<Attrs, Error>;
    /// async fn request(&self, request: SetStat) -> Result<(), Error>;
    /// async fn request(&self, request: FSetStat) -> Result<(), Error>;
    /// async fn request(&self, request: OpenDir) -> Result<Handle, Error>;
    /// async fn request(&self, request: ReadDir) -> Result<Name, Error>;
    /// async fn request(&self, request: Remove) -> Result<(), Error>;
    /// async fn request(&self, request: MkDir) -> Result<(), Error>;
    /// async fn request(&self, request: RmDir) -> Result<(), Error>;
    /// async fn request(&self, request: RealPath) -> Result<Name, Error>;
    /// async fn request(&self, request: Stat) -> Result<Attrs, Error>;
    /// async fn request(&self, request: Rename) -> Result<(), Error>;
    /// async fn request(&self, request: ReadLink) -> Result<Name, Error>;
    /// async fn request(&self, request: Symlink) -> Result<(), Error>;
    /// async fn request(&self, request: Extended) -> Result<ExtendedReply, Error>;
    /// ```
    pub fn request<R: SftpRequest>(&self, request: R) -> SftpFuture<R::Reply> {
        self.request_with(
            request.to_request_message(),
            (),
            stateless_from_reply_message::<R::Reply>,
        )
    }

    /// Send a raw SFTP request, and return its reply.
    ///
    /// # Arguments
    ///
    /// * `request` - SFTP Request to be sent
    /// * `state` - State used by the callback
    /// * `f` - callback used to transform the reply into a specific type
    ///
    /// # Return
    ///
    /// If the input request was an error, the future will return the error when polled.
    /// If an error occured at the sftp or ssh layer, the future will return the error when polled.
    /// If no error has occured, the future will return the output of `f`.
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use bytes::Bytes;
    /// # use rusftp::{client::{SftpClient, SftpFuture, SftpReply, SftpRequest}, message::{Data, Handle, Read}};
    /// pub fn read(sftp: &SftpClient, handle: Handle, offset: u64, length: u32) -> SftpFuture<Bytes> {
    ///     sftp.request_with(
    ///         Read {
    ///             handle,
    ///             offset,
    ///             length,
    ///         }
    ///         .to_request_message(),
    ///         (),
    ///         |_, msg| Ok(Data::from_reply_message(msg)?.0),
    ///     )
    /// }
    /// ```
    pub fn request_with<S, T>(
        &self,
        request: Result<Message, Error>,
        state: S,
        f: fn(S, Message) -> Result<T, Error>,
    ) -> SftpFuture<T, S> {
        if let Some(commands) = &self.commands {
            match request {
                Ok(Message::Status(Status {
                    code: StatusCode::Ok,
                    ..
                })) => SftpFuture::Error(
                    StatusCode::BadMessage
                        .to_status("Tried to send an OK status message to the server".into())
                        .into(),
                ),
                Ok(Message::Status(status)) => SftpFuture::Error(status.into()),
                Ok(msg) => {
                    let (tx, rx) = oneshot::channel();
                    match commands.send(super::receiver::Request(msg, tx)) {
                        Ok(()) => SftpFuture::Pending {
                            future: rx,
                            state,
                            f,
                        },
                        Err(err) => SftpFuture::Error(
                            StatusCode::Failure.to_status(err.to_string().into()).into(),
                        ),
                    }
                }
                Err(err) => SftpFuture::Error(err),
            }
        } else {
            SftpFuture::Error(
                std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "SFTP client has been stopped",
                )
                .into(),
            )
        }
    }
}

/// SFTP future.
///
/// Represents a future of a SFTP request that returns a `Result<Output, Error>` when polled.
///
/// If the request had an error even before being sent, the error will be returned when polled.
///
/// If `State` is `'static`, then the future itself is also `'static`.
///
/// # Cancel safety
///
/// It is safe to cancel the future.
/// However, the request is actually sent before the future is created.
pub enum SftpFuture<Output = (), State = ()> {
    /// An error occured before sending the request to the SFTP server.
    Error(Error),

    /// Waiting the result from the SFTP server.
    Pending {
        future: tokio::sync::oneshot::Receiver<Result<Message, Error>>,
        state: State,
        f: fn(State, Message) -> Result<Output, Error>,
    },

    /// The future has already been polled.
    Polled,
}

impl<Output, State> Future for SftpFuture<Output, State>
where
    State: Unpin,
{
    type Output = Result<Output, Error>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match &mut *self {
            SftpFuture::Error(_) => {
                let SftpFuture::Error(err) = std::mem::replace(&mut *self, SftpFuture::Polled)
                else {
                    unreachable!()
                };
                Poll::Ready(Err(err))
            }
            SftpFuture::Pending { future, .. } => {
                let result = match ready!(Pin::new(future).poll(cx)) {
                    Ok(Ok(msg)) => {
                        let SftpFuture::Pending { state, f, .. } =
                            std::mem::replace(&mut *self, SftpFuture::Polled)
                        else {
                            unreachable!()
                        };
                        f(state, msg)
                    }
                    Ok(Err(err)) => Err(err),
                    Err(_) => Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "Could not get reply from SFTP client",
                    ))),
                };

                *self = SftpFuture::Polled;
                Poll::Ready(result)
            }
            SftpFuture::Polled => panic!("Duplicated poll"),
        }
    }
}

/// Defines how a request is performed.
pub trait SftpRequest {
    /// Decoded type of the reply.
    ///
    /// The reply must implement [`SftpReply`].
    type Reply: SftpReply;

    /// Convert the request type into an actual SFTP message.
    fn to_request_message(self) -> Result<Message, Error>;
}

/// Defines how the reply is interpreted.
pub trait SftpReply: Sized {
    /// Convert the reply message into the decoded Reply type.
    ///
    /// The message can contain an Error status.
    /// If so, it is recommended to return the error as-is.
    fn from_reply_message(msg: Message) -> Result<Self, Error>;
}

impl SftpRequest for Message {
    type Reply = Message;

    fn to_request_message(self) -> Result<Message, Error> {
        Ok(self)
    }
}

impl SftpReply for Message {
    fn from_reply_message(msg: Message) -> Result<Self, Error> {
        Ok(msg)
    }
}

impl SftpReply for () {
    fn from_reply_message(msg: Message) -> Result<Self, Error> {
        match msg {
            Message::Status(status) => status.to_result(()),
            _ => Err(StatusCode::BadMessage.to_status("Expected a status".into())),
        }
        .map_err(Into::into)
    }
}

macro_rules! request_impl {
    ($input:ident) => {
        impl SftpRequest for message::$input {
            type Reply = ();

            fn to_request_message(self) -> Result<Message, Error> {
                Ok(self.into())
            }
        }
    };
    ($input:ident -> $output:ident) => {
        impl SftpRequest for message::$input {
            type Reply = message::$output;

            fn to_request_message(self) -> Result<Message, Error> {
                Ok(self.into())
            }
        }
    };
}

macro_rules! reply_impl {
    ($output:ident) => {
        impl SftpReply for message::$output {
            fn from_reply_message(msg: Message) -> Result<Self, Error> {
                match msg {
                    Message::$output(response) => Ok(response),
                    Message::Status(status) => Err(status),
                    _ => Err(StatusCode::BadMessage
                        .to_status(std::stringify!(Expected a $output or a Status).into())),
                }.map_err(Into::into)
            }
        }
    };
}

request_impl!(Open -> Handle);
request_impl!(Close);
request_impl!(Read -> Data);
request_impl!(Write);
request_impl!(LStat -> Attrs);
request_impl!(FStat -> Attrs);
request_impl!(SetStat);
request_impl!(FSetStat);
request_impl!(OpenDir -> Handle);
request_impl!(ReadDir -> Name);
request_impl!(Remove);
request_impl!(MkDir);
request_impl!(RmDir);
request_impl!(RealPath -> Name);
request_impl!(Stat -> Attrs);
request_impl!(Rename);
request_impl!(ReadLink -> Name);
request_impl!(Symlink);
request_impl!(Extended -> ExtendedReply);

reply_impl!(Attrs);
reply_impl!(Data);
reply_impl!(Handle);
reply_impl!(Name);
reply_impl!(ExtendedReply);

/// Wrapper for [`SftpReply::from_reply_message`] that takes an empty state.
///
/// Useful for `SftpClient::request_with`
fn stateless_from_reply_message<R: SftpReply>(_: (), msg: Message) -> Result<R, Error> {
    R::from_reply_message(msg)
}
