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

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct Open {
    pub filename: Path,
    pub pflags: u32,
    pub attrs: Attrs,
}

pub mod pflags {
    pub const READ: u32 = 0x00000001;
    pub const WRITE: u32 = 0x00000002;
    pub const APPEND: u32 = 0x00000004;
    pub const CREATE: u32 = 0x00000008;
    pub const TRUNCATE: u32 = 0x00000010;
    pub const EXCLUDE: u32 = 0x00000020;
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Attrs, Error, Path,
    };

    use super::Open;
    use bytes::Bytes;

    const OPEN_VALID: &[u8] = b"\0\0\0\x08filename\x56\xfe\x78\x21\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            Open {
                filename: Path(Bytes::from_static(b"filename")),
                pflags: 0x56fe7821,
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
            assert_eq!(fail_decode::<Open>(&OPEN_VALID[..i]), Error::NotEnoughData);
        }
    }
}
