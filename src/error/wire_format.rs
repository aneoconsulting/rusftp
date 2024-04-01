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

/// Error while encoding or decoding a message
#[derive(Debug, PartialEq, Eq)]
pub enum WireFormatError {
    /// The message was too small for the data it appears to be
    NotEnoughData,
    /// Unsupported character set
    Unsupported,
    /// Invalid character found
    InvalidChar,
    /// Custom error
    Custom(String),
}

impl std::fmt::Display for WireFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireFormatError::NotEnoughData => f.write_str("Decode Error: Not enough data"),
            WireFormatError::Unsupported => f.write_str("Decode Error: Unsupported"),
            WireFormatError::InvalidChar => f.write_str("Decode Error: Invalid character"),
            WireFormatError::Custom(msg) => write!(f, "Decode Error: {msg}"),
        }
    }
}

impl std::error::Error for WireFormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl serde::de::Error for WireFormatError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

impl serde::ser::Error for WireFormatError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}
