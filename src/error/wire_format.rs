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

/// Error while encoding or decoding a message
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WireFormatError {
    /// The message was too small for the data it appears to be
    #[error("Not enough data")]
    NotEnoughData,

    /// Unsupported character set
    #[error("Unsupported operation")]
    Unsupported,
    
    /// Invalid character found
    #[error("Invalid character")]
    InvalidChar,

    /// Custom error
    #[error("{0}")]
    Custom(String),
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
