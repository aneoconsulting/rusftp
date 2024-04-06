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

/// Generic request for an extension.
///
/// It may be answered with [`ExtendedReply`](crate::message::ExtendedReply) in case of success
/// and [`Status`](crate::message::Status) in case of failure.
///
/// If the server does not recognize the request,
/// then the server MUST respond with [`Status`](crate::message::Status) with status set to `OpUnsupported`.
///
/// internal: `SSH_FXP_EXTENDED`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Extended {
    /// Extended-request name
    ///
    /// It is a string of the format `name@domain`,
    /// where domain is an internet domain name of the vendor defining the request.
    /// The rest of the request is completely vendor-specific,
    /// and servers should only attempt to interpret it if they recognize the `extended-request' name.
    pub request: Bytes,

    /// Specific data needed by the extension to intrepret the request
    #[serde(rename = "data_implicit_length")]
    pub data: Bytes,
}

#[cfg(test)]
mod test {
    use crate::message::test_utils::{encode_decode, fail_decode, BYTES_INVALID};

    use super::Extended;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (request, data, encoded) in [
            (b"" as &[u8], b"" as &[u8], b"\0\0\0\0" as &[u8]),
            (b"", b"data", b"\0\0\0\0data"),
            (b"request", b"", b"\0\0\0\x07request"),
            (b"request", b"data", b"\0\0\0\x07requestdata"),
        ] {
            encode_decode(
                Extended {
                    request: Bytes::from_static(request),
                    data: Bytes::from_static(data),
                },
                encoded,
            );
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<Extended>(bytes), expected);
        }
    }
}
