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


/// Generic reply for an extension.
/// 
/// It can be used to carry arbitrary extension-specific data from the server to the client.
/// 
/// internal: `SSH_FXP_EXTENDED`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtendedReply {
    /// Specific data needed by the extension to intrepret the reply
    #[serde(rename = "data_implicit_length")]
    pub data: Bytes,
}

#[cfg(test)]
mod test {
    use crate::message::test_utils::{encode_decode, BYTES_VALID};

    use super::ExtendedReply;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (bytes, _) in BYTES_VALID {
            encode_decode(
                ExtendedReply {
                    data: Bytes::from_static(bytes),
                },
                bytes,
            );
        }
    }
}
