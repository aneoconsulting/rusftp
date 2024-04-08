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

//! Utilities module

use bytes::{Bytes, BytesMut};

pub trait IntoBytes {
    fn into_bytes(self) -> Bytes;
}

impl IntoBytes for Bytes {
    fn into_bytes(self) -> Bytes {
        self
    }
}

impl IntoBytes for BytesMut {
    fn into_bytes(self) -> Bytes {
        Bytes::from(self)
    }
}

impl IntoBytes for String {
    fn into_bytes(self) -> Bytes {
        Bytes::from(self)
    }
}

impl IntoBytes for Box<str> {
    fn into_bytes(self) -> Bytes {
        Bytes::from(Box::<[u8]>::from(self))
    }
}

impl IntoBytes for &str {
    fn into_bytes(self) -> Bytes {
        Bytes::copy_from_slice(self.as_bytes())
    }
}

impl IntoBytes for Vec<u8> {
    fn into_bytes(self) -> Bytes {
        Bytes::from(self)
    }
}

impl IntoBytes for Box<[u8]> {
    fn into_bytes(self) -> Bytes {
        Bytes::from(self)
    }
}

impl IntoBytes for &[u8] {
    fn into_bytes(self) -> Bytes {
        Bytes::copy_from_slice(self)
    }
}
