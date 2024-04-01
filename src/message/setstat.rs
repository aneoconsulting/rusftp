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

/// Request to change the attributes (metadata) of a file or directory.
///
/// This request is used for operations such as changing the ownership,
/// permissions or access times, as well as for truncating a file.
///
/// An error will be returned if the specified file system object does not exist
/// or the user does not have sufficient rights to modify the specified attributes.
///
/// It is answered with [`Status`](struct@crate::Status).
///
/// internal: `SSH_FXP_SETSTAT`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SetStat {
    /// Path of the file or directory to change the attributes
    pub path: Path,
    /// New attributes to apply
    pub attrs: Attrs,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Attrs, Path, WireFormatError,
    };

    use super::SetStat;
    use bytes::Bytes;

    const SETSTAT_VALID: &[u8] = b"\0\0\0\x04path\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            SetStat {
                path: Path(Bytes::from_static(b"path")),
                attrs: Attrs {
                    size: Some(0xa7735),
                    ..Default::default()
                },
            },
            SETSTAT_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..SETSTAT_VALID.len() {
            assert_eq!(
                fail_decode::<SetStat>(&SETSTAT_VALID[..i]),
                WireFormatError::NotEnoughData
            );
        }
    }
}
