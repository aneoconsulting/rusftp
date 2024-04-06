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

use thiserror::Error;

/// SFTP client error
#[derive(Debug, Error)]
pub enum ClientError {
    /// Error sent from SFTP server
    #[error(transparent)]
    Sftp(crate::Status),

    /// Encoding or Decoding error
    #[error(transparent)]
    WireFormat(crate::wire::WireFormatError),

    /// SSH error
    #[error(transparent)]
    Ssh(russh::Error),

    /// IO error
    #[error(transparent)]
    Io(std::io::Error),
}

impl From<crate::Status> for ClientError {
    fn from(value: crate::Status) -> Self {
        ClientError::Sftp(value)
    }
}

impl From<crate::wire::WireFormatError> for ClientError {
    fn from(value: crate::wire::WireFormatError) -> Self {
        ClientError::WireFormat(value)
    }
}

impl From<russh::Error> for ClientError {
    fn from(value: russh::Error) -> Self {
        match value {
            russh::Error::IO(io) => ClientError::Io(io),
            other => ClientError::Ssh(other),
        }
    }
}

impl From<std::io::Error> for ClientError {
    fn from(value: std::io::Error) -> Self {
        ClientError::Io(value)
    }
}

impl From<ClientError> for std::io::Error {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Sftp(sftp) => sftp.into(),
            ClientError::WireFormat(wire) => std::io::Error::new(std::io::ErrorKind::Other, wire),
            ClientError::Ssh(russh::Error::IO(io)) => io,
            ClientError::Ssh(ssh) => std::io::Error::new(std::io::ErrorKind::Other, ssh),
            ClientError::Io(io) => io,
        }
    }
}
