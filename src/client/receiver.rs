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

use bytes::Buf;
use russh::{client::Msg, Channel, ChannelMsg};
use tokio::sync::{mpsc, oneshot};

use crate::client::Error;
use crate::message::Message;

pub(super) type Response = Result<Message, Error>;
pub(super) struct Request(pub(super) Message, pub(super) oneshot::Sender<Response>);

pub(super) struct Receiver {
    onflight: HashMap<u32, oneshot::Sender<Response>>,
    next_id: u32,
    commands: mpsc::UnboundedReceiver<Request>,
    channel: Channel<Msg>,
}

impl Receiver {
    /// Create a new receiver
    pub(super) fn new(channel: Channel<Msg>) -> (Self, mpsc::UnboundedSender<Request>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                onflight: HashMap::new(),
                next_id: 0,
                commands: rx,
                channel,
            },
            tx,
        )
    }

    /// Run a receiver until the ssh channel is closed or no more commands can be sent
    pub(super) async fn run(mut self) {
        log::debug!("Start SFTP client");
        loop {
            tokio::select! {
                // New request to send
                request = self.commands.recv() => {
                    // If received null, the commands channel has been closed
                    let Some(Request(message, tx)) = request else {
                        log::debug!("Command channel closed");
                        break;
                    };

                    self.process_command(message, tx).await;
                }

                // New response received
                response = self.channel.wait() => {
                    // If received null, the SSH channel has been closed
                    let Some(ChannelMsg::Data { data }) = response else {
                        log::debug!("SFTP channel closed");
                        break;
                    };

                    self.process_response(&data).await;
                }
            }
        }

        while !self.onflight.is_empty() {
            // If received null, the SSH channel has been closed
            let Some(ChannelMsg::Data { data }) = self.channel.wait().await else {
                break;
            };

            self.process_response(&data).await;
        }

        self.commands.close();
        if let Err(err) = self.channel.close().await {
            log::warn!("Error while closing SSH channel: {err:?}");
        }

        log::debug!("SFTP client stopped");
    }

    /// Process a command request
    async fn process_command(&mut self, message: Message, tx: oneshot::Sender<Response>) {
        self.next_id += 1;
        let id = self.next_id;

        log::trace!("Request #{id}: {message:?}");

        match message.encode(id) {
            Ok(frame) => match self.channel.data(frame.as_ref()).await {
                Ok(()) => {
                    self.onflight.insert(id, tx);
                }
                Err(err) => {
                    log::debug!("Could not send request #{id}: {err:?}");
                    send_message(tx, Err(err.into()));
                }
            },
            Err(err) => {
                log::debug!("Could not encode request #{id}: {err:?}");
                send_message(tx, Err(err.into()));
            }
        }
    }

    /// Process a SSH response
    async fn process_response(&mut self, data: &[u8]) {
        match Message::decode(data) {
            Ok((id, message)) => {
                log::trace!("Response #{id}: {message:?}");
                if let Some(tx) = self.onflight.remove(&id) {
                    send_message(tx, Ok(message));
                } else {
                    log::error!("SFTP Error: Received a reply with an invalid id");
                }
            }
            Err(err) => {
                log::trace!("Failed to parse message: {data:?}");
                if let Some(mut buf) = data.get(5..9) {
                    let id = buf.get_u32();
                    if let Some(tx) = self.onflight.remove(&id) {
                        send_message(tx, Err(err.into()));
                    } else {
                        log::error!("SFTP Error: Received a reply with an invalid id");
                    }
                } else {
                    log::error!("SFTP Error: Received a bad reply");
                }
            }
        }
    }
}

fn send_message(tx: oneshot::Sender<Response>, msg: Response) {
    match tx.send(msg) {
        Ok(()) => (),
        Err(err) => {
            log::error!("Could not send back message to client: {err:?}");
        }
    }
}
