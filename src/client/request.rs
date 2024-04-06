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

use crate::{message, Message, SftpClient, StatusCode};

impl SftpClient {
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
