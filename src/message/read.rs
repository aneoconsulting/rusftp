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

use serde::{Deserialize, Serialize};

use crate::message::Handle;

/// Request to read a portion of an opened file.
///
/// It is answered with [`Data`](crate::message::Data) in case of success
/// and [`Status`](crate::message::Status) in case of failure.
///
/// internal: `SSH_FXP_READ`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Read {
    /// Handle of the file to read from
    pub handle: Handle,
    /// Byte offset where the read should start
    pub offset: u64,
    /// Number of bytes to read
    pub length: u32,
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode},
        Handle,
    };
    use crate::wire::Error;

    use super::Read;
    use bytes::Bytes;

    const FSETSTAT_VALID: &[u8] =
        b"\0\0\0\x06handle\xfe\xdc\xba\x98\x76\x54\x32\x10\xfd\xb9\x75\x31";

    #[test]
    fn encode_success() {
        encode_decode(
            Read {
                handle: Handle(Bytes::from_static(b"handle")),
                offset: 0xfedcba9876543210,
                length: 0xfdb97531,
            },
            FSETSTAT_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..FSETSTAT_VALID.len() {
            assert_eq!(
                fail_decode::<Read>(&FSETSTAT_VALID[..i]),
                Error::NotEnoughData
            );
        }
    }
}
