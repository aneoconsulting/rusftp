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

/// Request to read a directory listing.
///
/// Each [`ReadDir`] request returns one or more file names with full file attributes for each file.
/// The client should call [`ReadDir`] repeatedly until it has found the file it is looking for
/// or until the server responds with a [`Status`](crate::message::Status) message indicating an error
/// (normally `EOF` if there are no more files in the directory).
/// The client should then close the handle using the [`Close`](crate::message::Close) request.
///
/// It is answered with [`Name`](crate::message::Name) in case of success
/// and [`Status`](crate::message::Status) in case of failure.
///
/// internal: `SSH_FXP_OPENDIR`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadDir {
    /// Handle of the open directory
    pub handle: Handle,
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID},
        Handle,
    };

    use super::ReadDir;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(
                ReadDir {
                    handle: Handle(Bytes::from_static(bytes)),
                },
                encoded,
            );
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<ReadDir>(bytes), expected);
        }
    }
}
