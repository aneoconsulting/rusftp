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

use std::{
    borrow::{Borrow, BorrowMut},
    ops::{Deref, DerefMut, Index, IndexMut},
    slice::SliceIndex,
};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::message::{Attrs, Path};

/// Arbitrary byte string containing the requested data.
///
/// The data string may be at most the number of bytes requested in a [`Read`](crate::message::Read) request,
/// but may also be shorter if end of file is reached or if the read is from something other than a regular file.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NameEntry {
    /// Path of the file or directory designated by this entry
    ///
    /// for [`ReadDir`](crate::message::ReadDir), it will be a relative name within the directory, without any path components.
    ///
    /// for [`RealPath`](crate::message::RealPath) it will be an absolute path name.
    pub filename: Path,

    /// Expanded format of the filename with permissions and owner, à-la `ls -l`.
    ///
    /// Its format is unspecified by this protocol.
    /// It MUST be suitable for use in the output of a directory listing command
    /// (in fact, the recommended operation for a directory listing command is to simply display this data).
    /// However, clients SHOULD NOT attempt to parse the longname field for file attributes;
    /// they SHOULD use the attrs field instead.
    pub long_name: Bytes,

    /// Attributes of the file or directory designated by this entry
    pub attrs: Attrs,
}

/// Arbitrary byte string containing the requested data.
///
/// The data string may be at most the number of bytes requested in a [`Read`](crate::message::Read) request,
/// but may also be shorter if end of file is reached or if the read is from something other than a regular file.
///
/// internal: `SSH_FXP_DATA`
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Name(pub Vec<NameEntry>);

impl IntoIterator for Name {
    type Item = NameEntry;

    type IntoIter = <Vec<NameEntry> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Name {
    type Item = &'a NameEntry;

    type IntoIter = <&'a Vec<NameEntry> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Name {
    type Item = &'a mut NameEntry;

    type IntoIter = <&'a mut Vec<NameEntry> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl FromIterator<NameEntry> for Name {
    fn from_iter<T: IntoIterator<Item = NameEntry>>(iter: T) -> Self {
        Self(Vec::from_iter(iter))
    }
}

impl<I: SliceIndex<[NameEntry]>> Index<I> for Name {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.0[index]
    }
}

impl<I: SliceIndex<[NameEntry]>> IndexMut<I> for Name {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl AsRef<[NameEntry]> for Name {
    fn as_ref(&self) -> &[NameEntry] {
        &self.0
    }
}
impl AsMut<[NameEntry]> for Name {
    fn as_mut(&mut self) -> &mut [NameEntry] {
        &mut self.0
    }
}

impl Deref for Name {
    type Target = [NameEntry];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Name {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<[NameEntry]> for Name {
    fn borrow(&self) -> &[NameEntry] {
        &self.0
    }
}
impl BorrowMut<[NameEntry]> for Name {
    fn borrow_mut(&mut self) -> &mut [NameEntry] {
        &mut self.0
    }
}

#[cfg(test)]
mod test {
    use crate::message::{
        test_utils::{encode_decode, fail_decode},
        Attrs, Path,
    };
    use crate::wire::Error;

    use super::NameEntry;
    use bytes::Bytes;

    const NAME_VALID: &[u8] =
        b"\0\0\0\x08filename\0\0\0\x09long name\0\0\0\x01\0\0\0\0\0\x0a\x77\x35";

    #[test]
    fn encode_success() {
        encode_decode(
            NameEntry {
                filename: Path("filename".to_owned()),
                long_name: Bytes::from_static(b"long name"),
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
            assert_eq!(
                fail_decode::<NameEntry>(&NAME_VALID[..i]),
                Error::NotEnoughData
            );
        }
    }
}
