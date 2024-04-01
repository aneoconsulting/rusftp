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

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symlink {
    pub link_path: Path,
    pub target_path: Path,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Path, WireFormatError,
    };

    use super::Symlink;
    use bytes::Bytes;

    const SYMLINK_VALID: &[u8] = b"\0\0\0\x04link\0\0\0\x06target";

    #[test]
    fn encode_success() {
        encode_decode(
            Symlink {
                link_path: Path(Bytes::from_static(b"link")),
                target_path: Path(Bytes::from_static(b"target")),
            },
            SYMLINK_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..SYMLINK_VALID.len() {
            assert_eq!(
                fail_decode::<Symlink>(&SYMLINK_VALID[..i]),
                WireFormatError::NotEnoughData
            );
        }
    }
}
