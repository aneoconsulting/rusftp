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

use tokio::sync::oneshot;

use crate::{message, ClientError, Message, SftpClient, Status, StatusCode};

impl SftpClient {
    /// Send a SFTP request, and return its reply.
    ///
    /// In case a reply is the status `OK`, the empty tuple is returned instead: `()`.
    ///
    /// You can implement your own extension requests by implementing [`SftpRequest`].
    pub fn request<R: SftpRequest>(
        &self,
        request: R,
    ) -> impl Future<Output = Result<R::Reply, ClientError>> + Send + Sync + 'static {
        let sent = if let Some(commands) = &self.commands {
            match request.to_request_message() {
                Ok(Message::Status(Status {
                    code: StatusCode::Ok,
                    ..
                })) => Err(StatusCode::BadMessage
                    .to_status("Tried to send an OK status message to the server".into())
                    .into()),
                Ok(Message::Status(status)) => Err(status.into()),
                Ok(msg) => {
                    let (tx, rx) = oneshot::channel();
                    match commands.send(super::receiver::Request(msg, tx)) {
                        Ok(()) => Ok(rx),
                        Err(err) => {
                            Err(StatusCode::Failure.to_status(err.to_string().into()).into())
                        }
                    }
                }
                Err(err) => Err(err),
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "SFTP client has been stopped",
            )
            .into())
        };

        async move {
            match sent?.await {
                Ok(msg) => R::from_reply_message(msg?),
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "Could not get reply from SFTP client",
                )
                .into()),
            }
            // let msg = match sent {
            //     Ok(rx) => rx.await.unwrap_or(
            //         StatusCode::Failure.to_message("Could not get reply from SFTP client".into()),
            //     ),
            //     Err(err) => err,
            // };
            // R::from_reply_message(msg)
        }
    }
}

/// Defines how a request is performed and how the reply is interpreted.
pub trait SftpRequest {
    /// Decoded type of the reply
    type Reply;

    /// Convert the request type into an actual SFTP message
    fn to_request_message(self) -> Result<Message, ClientError>;

    /// Convert the reply message into the decoded Reply type
    ///
    /// The message can contain an Error status.
    /// If so, it is recommended to return the error as-is.
    fn from_reply_message(msg: Message) -> Result<Self::Reply, ClientError>;
}

impl SftpRequest for Message {
    type Reply = Message;

    fn to_request_message(self) -> Result<Message, ClientError> {
        Ok(self)
    }

    fn from_reply_message(msg: Message) -> Result<Self::Reply, ClientError> {
        Ok(msg)
    }
}

macro_rules! send_impl {
    ($input:ident) => {
        impl SftpRequest for message::$input {
            type Reply = ();

            fn to_request_message(self) -> Result<Message, ClientError> {
                Ok(self.into())
            }

            fn from_reply_message(msg: Message) -> Result<Self::Reply, ClientError> {
                match msg {
                    Message::Status(status) => status.to_result(()),
                    _ => Err(message::StatusCode::BadMessage
                        .to_status("Expected a status".into())),
                }.map_err(Into::into)
            }
        }
    };
    ($input:ident -> $output:ident) => {
        impl SftpRequest for message::$input {
            type Reply = message::$output;

            fn to_request_message(self) -> Result<Message, ClientError> {
                Ok(self.into())
            }

            fn from_reply_message(msg: Message) -> Result<Self::Reply, ClientError> {
                match msg {
                    Message::$output(response) => Ok(response),
                    Message::Status(status) => Err(status),
                    _ => Err(message::StatusCode::BadMessage
                        .to_status(std::stringify!(Expected a $output or a status).into())),
                }.map_err(Into::into)
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
