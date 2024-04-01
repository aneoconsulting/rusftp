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

use std::ops::Deref;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Arbitrary string that identifies an open file or directory on the server.
/// 
/// The handle is opaque to the client;
/// the client MUST NOT attempt to interpret or modify it in any way.
/// The length of the handle string MUST NOT exceed 256 data bytes.
/// 
/// internal: `SSH_FXP_HANDLE`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Handle(pub Bytes);

impl Deref for Handle {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Handle {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(test)]
mod test {
    use crate::message::test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID};

    use super::Handle;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(Handle(Bytes::from_static(bytes)), encoded);
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<Handle>(bytes), expected);
        }
    }
}
