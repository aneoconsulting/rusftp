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

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{wire::WireFormatError, Message};

/// Status code of an operation.
///
/// `OK` indicates that the operations has been successful.
/// All other values indicate errors.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[repr(u32)]
#[non_exhaustive]
pub enum StatusCode {
    /// Indicates successful completion of the operation.
    ///
    /// internal: `SSH_FX_OK`
    #[error("Ok")]
    #[default]
    Ok = 0,

    /// Indicates end-of-file condition.
    ///
    /// For [`Read`](crate::Read) it means that no more data is available in the file,
    /// and for [`ReadDir`](crate::ReadDir) it indicates that no more files are contained in the directory.
    ///
    /// internal: `SSH_FX_EOF`
    #[error("Eof")]
    Eof = 1,

    /// Returned when a reference is made to a file which should exist but doesn't.
    ///
    /// internal: `SSH_FX_NO_SUCH_FILE`
    #[error("NoSuchFile")]
    NoSuchFile = 2,

    /// Returned when the authenticated user does not have sufficient permissions to perform the operation.
    ///
    /// internal: `SSH_FX_PERMISSION_DENIED`
    #[error("PermissionDenied")]
    PermissionDenied = 3,

    /// A generic catch-all error message
    ///
    /// It should be returned if an error occurs for which there is no more specific error code defined.
    ///
    /// internal: `SSH_FX_FAILURE`
    #[error("Failure")]
    Failure = 4,

    /// May be returned if a badly formatted packet or protocol incompatibility is detected.
    ///
    /// internal: `SSH_FX_BAD_MESSAGE`
    #[error("BadMessage")]
    BadMessage = 5,

    /// A pseudo-error which indicates that the client has no connection to the server.
    ///
    /// It can only be generated locally by the client, and MUST NOT be returned by servers.
    ///
    /// internal: `SSH_FX_NO_CONNECTION`
    #[error("NoConnection")]
    NoConnection = 6,

    /// A pseudo-error which indicates that the connection to the server has been lost.
    ///
    /// It can only be generated locally by the client, and MUST NOT be returned by servers.
    ///
    /// internal: `SSH_FX_CONNECTION_LOST`
    #[error("ConnectionLost")]
    ConnectionLost = 7,

    /// Indicates that an attempt was made to perform an operation which is not supported for the server
    ///
    /// It may be generated locally by the client if e.g.  the version number exchange indicates
    /// that a required feature is not supported by the server,
    /// or it may be returned by the server if the server does not implement an operation.
    ///
    /// internal: `SSH_FX_OP_UNSUPPORTED`
    #[error("OpUnsupported")]
    OpUnsupported = 8,
}

/// Status of an operation.
///
/// A status code `OK` indicates that the operations has been successful.
/// All other status codes indicate errors.
///
/// internal: `SSH_FXP_STATUS`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Status {
    /// Code of the status, see [`StatusCode`]
    pub code: StatusCode,
    /// Message of the error
    pub error: Bytes,
    /// Language tag for the error message
    pub language: Bytes,
}

impl Status {
    pub fn is_ok(&self) -> bool {
        self.code == StatusCode::Ok
    }
    pub fn is_err(&self) -> bool {
        self.code != StatusCode::Ok
    }

    pub fn to_result<T>(self, value: T) -> Result<T, Self> {
        if self.is_ok() {
            Ok(value)
        } else {
            Err(self)
        }
    }
}

impl StatusCode {
    pub fn to_status(self, msg: Bytes) -> Status {
        let msg = if msg.is_empty() {
            self.to_string().into()
        } else {
            msg
        };

        Status {
            code: self,
            error: msg,
            language: "en".into(),
        }
    }

    pub fn to_message(self, msg: Bytes) -> Message {
        Message::Status(self.to_status(msg))
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.error.is_empty() {
            write!(f, "{}", self.code)
        } else {
            write!(
                f,
                "{}: {}",
                self.code,
                String::from_utf8_lossy(self.error.as_ref())
            )
        }
    }
}

impl TryFrom<u32> for StatusCode {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == Self::Ok as u32 {
            Ok(Self::Ok)
        } else if value == Self::Eof as u32 {
            Ok(Self::Eof)
        } else if value == Self::NoSuchFile as u32 {
            Ok(Self::NoSuchFile)
        } else if value == Self::PermissionDenied as u32 {
            Ok(Self::PermissionDenied)
        } else if value == Self::Failure as u32 {
            Ok(Self::Failure)
        } else if value == Self::BadMessage as u32 {
            Ok(Self::BadMessage)
        } else if value == Self::NoConnection as u32 {
            Ok(Self::NoConnection)
        } else if value == Self::ConnectionLost as u32 {
            Ok(Self::ConnectionLost)
        } else if value == Self::OpUnsupported as u32 {
            Ok(Self::OpUnsupported)
        } else {
            Err(value)
        }
    }
}

impl From<std::io::ErrorKind> for StatusCode {
    fn from(value: std::io::ErrorKind) -> Self {
        match value {
            std::io::ErrorKind::NotFound => Self::NoSuchFile,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused => Self::NoConnection,
            std::io::ErrorKind::ConnectionReset => Self::ConnectionLost,
            std::io::ErrorKind::ConnectionAborted => Self::ConnectionLost,
            std::io::ErrorKind::NotConnected => Self::NoConnection,
            std::io::ErrorKind::InvalidInput => Self::BadMessage,
            std::io::ErrorKind::InvalidData => Self::BadMessage,
            std::io::ErrorKind::Unsupported => Self::OpUnsupported,
            std::io::ErrorKind::UnexpectedEof => Self::Eof,
            _ => Self::Failure,
        }
    }
}

impl From<std::io::Error> for Status {
    fn from(value: std::io::Error) -> Self {
        Self {
            code: StatusCode::from(value.kind()),
            error: value.to_string().into(),
            language: "en".into(),
        }
    }
}

impl From<russh::Error> for Status {
    fn from(value: russh::Error) -> Self {
        let status_code = match value {
            russh::Error::ChannelOpenFailure(_) => StatusCode::NoConnection,
            russh::Error::Disconnect => StatusCode::ConnectionLost,
            russh::Error::IO(io) => {
                return io.into();
            }
            _ => StatusCode::Failure,
        };
        Self {
            code: status_code,
            error: value.to_string().into(),
            language: "en".into(),
        }
    }
}

impl From<WireFormatError> for Status {
    fn from(error: WireFormatError) -> Self {
        Self {
            code: StatusCode::BadMessage,
            error: error.to_string().into(),
            language: "en".into(),
        }
    }
}

impl From<Status> for std::io::Error {
    fn from(value: Status) -> Self {
        let kind = match value.code {
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

        Self::new(kind, value)
    }
}

impl std::error::Error for Status {}

#[cfg(test)]
mod test {
    use bytes::Bytes;

    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        wire::WireFormatError,
    };

    use super::{Status, StatusCode};

    const STATUS_VALID: &[u8] = b"\0\0\0\x01\0\0\0\x03eof\0\0\0\x02en";

    #[test]
    fn encode_success() {
        for (code, encoded) in [
            (StatusCode::Ok, b"\0\0\0\x00"),
            (StatusCode::Eof, b"\0\0\0\x01"),
            (StatusCode::NoSuchFile, b"\0\0\0\x02"),
            (StatusCode::PermissionDenied, b"\0\0\0\x03"),
            (StatusCode::Failure, b"\0\0\0\x04"),
            (StatusCode::BadMessage, b"\0\0\0\x05"),
            (StatusCode::NoConnection, b"\0\0\0\x06"),
            (StatusCode::ConnectionLost, b"\0\0\0\x07"),
            (StatusCode::OpUnsupported, b"\0\0\0\x08"),
        ] {
            encode_decode(code, encoded);
        }

        encode_decode(
            Status {
                code: StatusCode::Eof,
                error: Bytes::from_static(b"eof"),
                language: Bytes::from_static(b"en"),
            },
            STATUS_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..STATUS_VALID.len() {
            assert_eq!(
                fail_decode::<Status>(&STATUS_VALID[..i]),
                WireFormatError::NotEnoughData
            );
        }
    }
}
