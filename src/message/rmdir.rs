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

use super::Path;

/// Request to remove an existing directory.
///
/// An error will be returned if no directory with the specified path exists,
/// or if the specified directory is not empty, or if the path specified
/// a file system object other than a directory.
///
/// It is answered with [`Status`](struct@crate::Status).
///
/// internal: `SSH_FXP_RMDIR`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RmDir {
    /// Path of the directory to remove
    pub path: Path,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID},
        Path,
    };

    use super::RmDir;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(
                RmDir {
                    path: Path(Bytes::from_static(bytes)),
                },
                encoded,
            );
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<RmDir>(bytes), expected);
        }
    }
}
