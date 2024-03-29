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
pub struct Name {
    pub filename: Path,
    pub long_name: Path,
    pub attrs: Attrs,
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode},
        Attrs, Error, Path,
    };

    use super::Name;
    use bytes::Bytes;

    const NAME_VALID: &[u8] =
        b"\0\0\0\x08filename\0\0\0\x09long name\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            Name {
                filename: Path(Bytes::from_static(b"filename")),
                long_name: Path(Bytes::from_static(b"long name")),
                attrs: Attrs {
                    size: Some(0xa7735),
                    ..Default::default()
                },
            },
            NAME_VALID,
        );
    }

    #[test]
    fn decode_failure() {
        for i in 0..NAME_VALID.len() {
            assert_eq!(fail_decode::<Name>(&NAME_VALID[..i]), Error::NotEnoughData);
        }
    }
}
