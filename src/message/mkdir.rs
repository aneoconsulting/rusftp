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

use super::{Attrs, Path};

/// Request to create a new directory.
///
/// An error will be returned if a file or directory with the specified path already exists.
///
/// It is answered with [`Status`](struct@crate::Status).
///
/// internal: `SSH_FXP_MKDIR`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MkDir {
    /// Path where the new directory will be located
    pub path: Path,
    /// Default attributes to apply to the newly created directory
    pub attrs: Attrs,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Attrs, Path, WireFormatError,
    };

    use super::MkDir;
    use bytes::Bytes;

    const MKDIR_VALID: &[u8] = b"\0\0\0\x04path\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            MkDir {
                path: Path(Bytes::from_static(b"path")),
                attrs: Attrs {
                    size: Some(0xa7735),
                    ..Default::default()
                },
            },
            MKDIR_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..MKDIR_VALID.len() {
            assert_eq!(
                fail_decode::<MkDir>(&MKDIR_VALID[..i]),
                WireFormatError::NotEnoughData
            );
        }
    }
}
