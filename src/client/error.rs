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

use crate::StatusCode;

/// SFTP client error
#[derive(Debug, Error)]
pub enum ClientError {
    /// Error sent from SFTP server
    #[error(transparent)]
    Sftp(#[from] crate::Status),

    /// Encoding or Decoding error
    #[error(transparent)]
    WireFormat(#[from] crate::wire::WireFormatError),

    /// SSH error
    #[error(transparent)]
    Ssh(russh::Error),

    /// IO error
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
impl From<russh::Error> for ClientError {
    fn from(value: russh::Error) -> Self {
        match value {
            russh::Error::IO(io) => ClientError::Io(io),
            other => ClientError::Ssh(other),
        }
    }
}

impl From<ClientError> for std::io::Error {
    fn from(value: ClientError) -> Self {
        match value {
            ClientError::Sftp(sftp) => {
                let kind = match sftp.code {
                    StatusCode::Ok => std::io::ErrorKind::Other,
                    StatusCode::Eof => std::io::ErrorKind::UnexpectedEof,
                    StatusCode::NoSuchFile => std::io::ErrorKind::NotFound,
                    StatusCode::PermissionDenied => std::io::ErrorKind::PermissionDenied,
                    StatusCode::Failure => std::io::ErrorKind::Other,
                    StatusCode::BadMessage => std::io::ErrorKind::InvalidData,
                    StatusCode::NoConnection => std::io::ErrorKind::Other,
                    StatusCode::ConnectionLost => std::io::ErrorKind::Other,
                    StatusCode::OpUnsupported => std::io::ErrorKind::Unsupported,
                };

                Self::new(kind, sftp)
            }
            ClientError::WireFormat(wire) => std::io::Error::new(std::io::ErrorKind::Other, wire),
            ClientError::Ssh(russh::Error::IO(io)) => io,
            ClientError::Ssh(ssh) => std::io::Error::new(std::io::ErrorKind::Other, ssh),
            ClientError::Io(io) => io,
        }
    }
}
