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
use std::pin::Pin;
use std::task::Poll;

use bytes::{Buf, Bytes, BytesMut};
use futures::{Stream, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::client::Error;
use crate::message::{Message, StatusCode};

pub(super) type Response = Result<Message, Error>;
pub struct Request(pub(super) Message, pub(super) oneshot::Sender<Response>);

pub(super) struct Receiver<S> {
    onflight: HashMap<u32, oneshot::Sender<Response>>,
    next_id: u32,
    commands: mpsc::UnboundedReceiver<Request>,
    stream: S,
    response_size: Option<u32>,
    response_buffer: BytesMut,
}

impl<S> Receiver<S> {
    /// Create a new receiver
    pub(super) fn new(stream: S) -> (Self, mpsc::UnboundedSender<Request>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                onflight: HashMap::new(),
                next_id: 0,
                commands: rx,
                stream,
                response_size: None,
                response_buffer: Default::default(),
            },
            tx,
        )
    }
}

pub enum StreamItem {
    Request(Request),
    Response(Bytes),
    Error(std::io::Error),
}

impl<S: AsyncRead + AsyncWrite + Unpin> Stream for Receiver<S> {
    type Item = StreamItem;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // Check if new commands have been sent
        match self.commands.poll_recv(cx) {
            Poll::Ready(Some(request)) => {
                return Poll::Ready(Some(StreamItem::Request(request)));
            }
            Poll::Ready(None) => {
                // If commands are closed and no request is on-flight,
                // No more messages could be received
                if self.onflight.is_empty() {
                    return Poll::Ready(None);
                }
            }
            Poll::Pending => (),
        };

        // No command was available, trying to read responses from the stream
        loop {
            let new_len;
            match self.response_size {
                // A size has already been read from the stream
                Some(response_size) => {
                    if self.response_buffer.len() >= response_size as usize {
                        self.response_size = None;
                        let response = self.response_buffer.split_to(response_size as usize);
                        return Poll::Ready(Some(StreamItem::Response(response.freeze())));
                    }
                    new_len = response_size as usize;
                }
                // Must read the size of the frame from the stream
                None => {
                    if self.response_buffer.len() >= std::mem::size_of::<u32>() {
                        let len = self.response_buffer.get_u32();
                        self.response_size = Some(len);
                        continue;
                    }
                    new_len = std::mem::size_of::<u32>();
                }
            }

            let old_len = self.response_buffer.len();

            // taking is required to avoid borrowing multiple times `self`
            let mut buffer = std::mem::take(&mut self.response_buffer);

            // tries to read the whole frame, or at least the next kilobyte
            buffer.resize(new_len.max(1024), 0);
            let mut read_buf = tokio::io::ReadBuf::new(&mut buffer[old_len..]);
            let read = Pin::new(&mut self.stream).poll_read(cx, &mut read_buf);

            // Adjust buffer size according to what was read
            let len = read_buf.filled().len();
            buffer.resize(old_len + len, 0);
            self.response_buffer = buffer;

            // Check status of reading
            match read {
                Poll::Ready(Ok(())) => (),
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Some(StreamItem::Error(err)));
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }

            // EoF
            if len == old_len {
                return Poll::Ready(None);
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Receiver<S> {
    /// Run a receiver until the ssh channel is closed or no more commands can be sent
    pub(super) async fn run(mut self) {
        log::debug!("Start SFTP client");

        // Read all the events
        while let Some(event) = self.next().await {
            match event {
                // New request was received
                StreamItem::Request(Request(message, tx)) => {
                    self.next_id += 1;
                    let id = self.next_id;

                    log::trace!("Request #{id}: {message:?}");

                    match write_msg(&mut self.stream, message, id).await {
                        Ok(()) => {
                            self.onflight.insert(id, tx);
                        }
                        Err(err) => {
                            log::debug!("Could not send request #{id}: {err:?}");
                            send_response(tx, Err(err));
                        }
                    }
                }

                // New response was received
                StreamItem::Response(response) => match Message::decode_raw(response.as_ref()) {
                    Ok((id, message)) => {
                        log::trace!("Response #{id}: {message:?}");
                        if let Some(tx) = self.onflight.remove(&id) {
                            send_response(tx, Ok(message));
                        } else {
                            log::error!("SFTP Error: Received a reply with an invalid id");
                        }
                    }
                    Err(err) => {
                        log::trace!("Failed to parse message: {response:?}: {err:?}");
                        if let Some(id) = err.id {
                            if let Some(tx) = self.onflight.remove(&id) {
                                send_response(tx, Err(err.into()));
                            } else {
                                log::error!("SFTP Error: Received a reply with an invalid id");
                            }
                        } else {
                            log::error!("SFTP Error: Received a bad reply");
                        }
                    }
                },

                // Error while receiving
                StreamItem::Error(err) => {
                    log::error!("Error while waiting for SFTP response: {err:?}");
                    match err.kind() {
                        std::io::ErrorKind::WouldBlock => (),
                        std::io::ErrorKind::TimedOut => (),
                        std::io::ErrorKind::WriteZero => (),
                        std::io::ErrorKind::Interrupted => (),
                        std::io::ErrorKind::OutOfMemory => (),
                        _ => break,
                    }
                }
            }
        }

        for (_, tx) in self.onflight {
            send_response(
                tx,
                Err(Error::Sftp(StatusCode::ConnectionLost.to_status(
                    "Could not receive response: SFTP stream stopped",
                ))),
            );
        }

        self.commands.close();
        if let Err(err) = self.stream.shutdown().await {
            log::warn!("Error while closing SSH channel: {err:?}");
        }

        log::debug!("SFTP client stopped");
    }
}

fn send_response(tx: oneshot::Sender<Response>, msg: Response) {
    match tx.send(msg) {
        Ok(()) => (),
        Err(err) => {
            log::error!("Could not send back message to client: {err:?}");
        }
    }
}

pub(super) async fn write_msg(
    stream: &mut (impl AsyncWrite + Unpin),
    msg: Message,
    id: u32,
) -> Result<(), Error> {
    let frame = msg.encode(id)?;
    Ok(stream.write_all(frame.as_ref()).await?)
}

pub(super) async fn read_msg(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<(u32, Message), Error> {
    let length = stream.read_u32().await?;

    let mut bytes = vec![0u8; length as usize];
    stream.read_exact(bytes.as_mut_slice()).await?;

    Ok(Message::decode_raw(bytes.as_slice())?)
}
