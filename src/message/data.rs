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

use std::{borrow::Borrow, ops::Deref};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Arbitrary byte string containing the requested data.
///
/// The data string may be at most the number of bytes requested in a [`Read`](crate::message::Read) request,
/// but may also be shorter if end of file is reached or if the read is from something other than a regular file.
///
/// internal: `SSH_FXP_DATA`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data(pub Bytes);

impl<T: crate::utils::IntoBytes> From<T> for Data {
    fn from(value: T) -> Self {
        Data(value.into_bytes())
    }
}

impl Deref for Data {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Borrow<[u8]> for Data {
    fn borrow(&self) -> &[u8] {
        self.0.borrow()
    }
}

#[cfg(test)]
mod test {
    use crate::message::test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID};

    use super::Data;
    use bytes::Bytes;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(Data(Bytes::from(bytes)), encoded);
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<Data>(bytes), expected);
        }
    }
}
