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

use crate::message::Path;

/// Request to canonicalize a path.
///
/// The response will contain a single entry whose attributes have dummy values.
///
/// It is answered with [`Name`](crate::message::Name) in case of success
/// and [`Status`](crate::message::Status) in case of failure.
///
/// internal: `SSH_FXP_REALPATH`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RealPath {
    /// Path to canonicalize
    pub path: Path,
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID},
        Path,
    };

    use super::RealPath;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(
                RealPath {
                    path: Path(bytes.to_owned()),
                },
                encoded,
            );
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<RealPath>(bytes), expected);
        }
    }
}
