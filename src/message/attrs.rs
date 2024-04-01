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

use bitflags::bitflags;
use serde::{ser::SerializeTuple, Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attrs {
    pub size: Option<u64>,
    pub owner: Option<Owner>,
    pub perms: Option<Permisions>,
    pub time: Option<Time>,
}

impl Attrs {
    pub const fn new() -> Self {
        Self {
            size: None,
            owner: None,
            perms: None,
            time: None,
        }
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Permisions: u32 {
        // Permissions for others
        const OX = 0x0001;
        const OW = 0x0002;
        const OR = 0x0004;
        // Permissions for group
        const GX = 0x0008;
        const GW = 0x0010;
        const GR = 0x0020;
        // Permissions for user
        const UX = 0x0040;
        const UW = 0x0080;
        const UR = 0x0100;
        // Special permissions
        const SX = 0x0200;
        const SW = 0x0400;
        const SR = 0x0800;
        // File type
        const FIFO = 0x1000;
        const CHR = 0x2000;
        const DIR = 0x4000;
        const BLK = 0x6000;
        const REG = 0x8000;
        const LNK = 0xA000;
        const NAM = 0x5000;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Time {
    pub atime: u32,
    pub mtime: u32,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[repr(transparent)]
    struct AttrFlags: u32 {
        const Size = 0x00000001;
        const Owner = 0x00000002;
        const Perms = 0x00000004;
        const Time = 0x00000008;
    }
}

impl Serialize for Attrs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut attr_flags = AttrFlags::empty();

        if self.size.is_some() {
            attr_flags |= AttrFlags::Size;
        }
        if self.owner.is_some() {
            attr_flags |= AttrFlags::Owner;
        }
        if self.perms.is_some() {
            attr_flags |= AttrFlags::Perms;
        }
        if self.time.is_some() {
            attr_flags |= AttrFlags::Time;
        }

        let mut state = serializer.serialize_tuple(5)?;

        state.serialize_element(&attr_flags)?;
        state.serialize_element(&self.size)?;
        state.serialize_element(&self.owner)?;
        state.serialize_element(&self.perms)?;
        state.serialize_element(&self.time)?;

        state.end()
    }
}

macro_rules! next {
    ($seq:expr, $field:expr) => {
        $seq.next_element()?
            .ok_or(serde::de::Error::missing_field($field))?
    };
}

impl<'de> Deserialize<'de> for Attrs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Attrs;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a flag, a size, an owner pair, a perm flag, and a time pair"
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut attrs = Attrs::default();
                let attr_flags: u32 = next!(seq, "attr_flags");

                let Some(attr_flags) = AttrFlags::from_bits(attr_flags) else {
                    return Err(A::Error::custom("invalid attr"));
                };

                if !(attr_flags & AttrFlags::Size).is_empty() {
                    attrs.size = Some(next!(seq, "attr_size"));
                } else {
                    next!(seq, "attr_size");
                }
                if !(attr_flags & AttrFlags::Owner).is_empty() {
                    attrs.owner = Some(next!(seq, "attr_owner"));
                } else {
                    next!(seq, "attr_owner");
                }
                if !(attr_flags & AttrFlags::Perms).is_empty() {
                    attrs.perms = Some(next!(seq, "attr_perms"));
                } else {
                    next!(seq, "attr_perms");
                }
                if !(attr_flags & AttrFlags::Time).is_empty() {
                    attrs.time = Some(next!(seq, "attr_time"));
                } else {
                    next!(seq, "attr_time");
                }

                Ok(attrs)
            }
        }

        deserializer.deserialize_tuple(5, Visitor)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        message::test_utils::{encode_decode, fail_decode, ATTRS_VALID},
        WireFormatError,
    };

    use super::Attrs;

    #[test]
    fn encode_success() {
        for (attrs, encoded) in ATTRS_VALID {
            encode_decode(attrs, encoded);
        }
    }

    #[test]
    fn decode_failure() {
        for (_, encoded) in ATTRS_VALID {
            for i in 0..encoded.len() - 1 {
                assert_eq!(
                    fail_decode::<Attrs>(&encoded[..i]),
                    WireFormatError::NotEnoughData
                );
            }
        }

        assert_eq!(
            fail_decode::<Attrs>(b"\0\0\x01\0"),
            WireFormatError::Custom("invalid attr".to_string())
        );
    }
}
