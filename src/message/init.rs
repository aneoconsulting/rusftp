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

use std::collections::BTreeMap;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Initialization packet.
///
/// It is answered with [`Version`](crate::message::Version).
///
/// internal: `SSH_FXP_INIT`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Init {
    /// Maximal version of the protocol supported by the client
    pub version: u32,

    /// List of extensions supported by the client
    ///
    /// Implementations MUST silently ignore any extensions whose name they do not recognize.
    #[serde(rename = "extensions_implicit_length")]
    pub extensions: BTreeMap<Bytes, Bytes>,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        wire::Error,
    };

    use super::Init;
    use bytes::Bytes;

    const INIT_VALID: &[u8] = b"\xfe\xdc\xba\x98\0\0\0\x03key\0\0\0\x05value";

    #[test]
    fn encode_success() {
        for (version, map, encoded) in [
            (0u32, &[] as &[(&[u8], &[u8])], b"\0\0\0\0" as &[u8]),
            (0xfedcba98, &[], b"\xfe\xdc\xba\x98"),
            (0xfedcba98, &[(b"key", b"value")], INIT_VALID),
            (
                0xfedcba98,
                &[(b"key0", b"value0"), (b"key1", b"value1")],
                b"\xfe\xdc\xba\x98\0\0\0\x04key0\0\0\0\x06value0\0\0\0\x04key1\0\0\0\x06value1",
            ),
        ] {
            encode_decode(
                Init {
                    version,
                    extensions: map
                        .iter()
                        .map(|(k, v)| (Bytes::from_static(k), Bytes::from_static(v)))
                        .collect(),
                },
                encoded,
            );
        }
    }

    #[test]
    fn decode_failure() {
        for i in 5..INIT_VALID.len() {
            assert_eq!(fail_decode::<Init>(&INIT_VALID[..i]), Error::NotEnoughData);
        }
    }
}
