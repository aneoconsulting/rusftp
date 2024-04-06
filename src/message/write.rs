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

use crate::message::{Data, Handle};

/// Request to write to a portion of an opened file.
///
/// It is answered with [`Status`](crate::Status).
///
/// internal: `SSH_FXP_WRITE`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Write {
    /// Handle of the file to write to
    pub handle: Handle,
    /// Byte offset where the write should start
    pub offset: u64,
    /// Bytes to be written to the file
    pub data: Data,
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode},
        Data, Handle,
    };
    use crate::wire::Error;

    use super::Write;
    use bytes::Bytes;

    const WRITE_VALID: &[u8] = b"\0\0\0\x06handle\xfe\xdc\xba\x98\x76\x54\x32\x10\0\0\0\x04data";

    #[test]
    fn encode_success() {
        encode_decode(
            Write {
                handle: Handle(Bytes::from_static(b"handle")),
                offset: 0xfedcba9876543210,
                data: Data(Bytes::from_static(b"data")),
            },
            WRITE_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..WRITE_VALID.len() {
            assert_eq!(
                fail_decode::<Write>(&WRITE_VALID[..i]),
                Error::NotEnoughData
            );
        }
    }
}
