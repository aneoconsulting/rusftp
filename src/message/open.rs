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

/// Request to open a file for reading or writing.
///
/// It is answered with [`Handle`](struct@crate::Handle) in case of success
/// and [`Status`](struct@crate::Status) in case of failure.
///
/// internal: `SSH_FXP_OPEN`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Open {
    /// Path of the file to open
    pub filename: Path,
    /// Flags for the file opening
    pub pflags: PFlags,
    /// Default file attributes to use upon file creation
    pub attrs: Attrs,
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct PFlags: u32 {
        /// Open the file for reading.
        ///
        /// internal: `SSH_FXF_READ`
        const READ = 0x00000001;

        /// Open the file for writing.
        /// If both this and `READ` are specified, the file is opened for both reading and writing.
        ///
        /// internal: `SSH_FXF_WRITE`
        const WRITE = 0x00000002;

        /// Force all writes to append data at the end of the file.
        ///
        /// internal: `SSH_FXF_APPEND`
        const APPEND = 0x00000004;

        /// If this flag is specified, then a new file will be created if one does not already exist
        /// (if O_TRUNC is specified, the new file will be truncated to zero length if it previously exists).
        ///
        /// internal: `SSH_FXF_CREAT`
        const CREATE = 0x00000008;

        /// Forces an existing file with the same name to be truncated to zero length when creating a file by specifying `CREATE`.
        /// `CREATE` MUST also be specified if this flag is used.
        ///
        /// internal: `SSH_FXF_TRUNC`
        const TRUNCATE = 0x00000010;

        /// Causes the request to fail if the named file already exists.
        /// `CREATE` MUST also be specified if this flag is used.
        ///
        /// internal: `SSH_FXF_EXCL`
        const EXCLUDE = 0x00000020;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Attrs, Path, WireFormatError,
    };

    use super::{Open, PFlags};
    use bytes::Bytes;

    const OPEN_VALID: &[u8] = b"\0\0\0\x08filename\0\0\0\x09\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            Open {
                filename: Path(Bytes::from_static(b"filename")),
                pflags: PFlags::READ | PFlags::CREATE,
                attrs: Attrs {
                    size: Some(0xa7735),
                    ..Default::default()
                },
            },
            OPEN_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..OPEN_VALID.len() {
            assert_eq!(
                fail_decode::<Open>(&OPEN_VALID[..i]),
                WireFormatError::NotEnoughData
            );
        }
    }
}
