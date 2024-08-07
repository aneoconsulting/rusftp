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

/// Request to rename/move a file or a directory.
///
/// It is answered with [`Status`](crate::message::Status).
///
/// internal: `SSH_FXP_RENAME`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rename {
    /// Current path of the file or directory to rename/move
    pub old_path: Path,
    /// New path where the file or directory will be moved to
    pub new_path: Path,
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode},
        Path,
    };
    use crate::wire::Error;

    use super::Rename;

    const RENAME_VALID: &[u8] = b"\0\0\0\x03old\0\0\0\x03new";

    #[test]
    fn encode_success() {
        encode_decode(
            Rename {
                old_path: Path("old".to_owned()),
                new_path: Path("new".to_owned()),
            },
            RENAME_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..RENAME_VALID.len() {
            assert_eq!(
                fail_decode::<Rename>(&RENAME_VALID[..i]),
                Error::NotEnoughData
            );
        }
    }
}
