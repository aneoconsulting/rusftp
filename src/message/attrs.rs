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

use serde::{ser::SerializeTuple, Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct Attrs {
    pub size: Option<u64>,
    pub owner: Option<Owner>,
    pub perms: Option<u32>,
    pub time: Option<Time>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
#[non_exhaustive]
pub enum Permisions {
    // Permissions for others
    OX = 0x0001,
    OW = 0x0002,
    OR = 0x0004,
    // Permissions for group
    GX = 0x0008,
    GW = 0x0010,
    GR = 0x0020,
    // Permissions for user
    UX = 0x0040,
    UW = 0x0080,
    UR = 0x0100,
    // Special permissions
    SX = 0x0200,
    SW = 0x0400,
    SR = 0x0800,
    // File type
    FIFO = 0x1000,
    CHR = 0x2000,
    DIR = 0x4000,
    BLK = 0x6000,
    REG = 0x8000,
    LNK = 0xA000,
    NAM = 0x5000,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub struct Time {
    pub atime: u32,
    pub mtime: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
#[non_exhaustive]
enum AttrFlags {
    Size = 0x00000001,
    Owner = 0x00000002,
    Perms = 0x00000004,
    Time = 0x00000008,
}

impl Serialize for Attrs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut attr_flags = 0u32;

        if self.size.is_some() {
            attr_flags |= AttrFlags::Size as u32;
        }
        if self.owner.is_some() {
            attr_flags |= AttrFlags::Owner as u32;
        }
        if self.perms.is_some() {
            attr_flags |= AttrFlags::Perms as u32;
        }
        if self.time.is_some() {
            attr_flags |= AttrFlags::Time as u32;
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

                if (attr_flags & AttrFlags::Size as u32) != 0 {
                    attrs.size = Some(next!(seq, "attr_size"));
                } else {
                    next!(seq, "attr_size");
                }
                if (attr_flags & AttrFlags::Owner as u32) != 0 {
                    attrs.owner = Some(next!(seq, "attr_owner"));
                } else {
                    next!(seq, "attr_owner");
                }
                if (attr_flags & AttrFlags::Perms as u32) != 0 {
                    attrs.perms = Some(next!(seq, "attr_perms"));
                } else {
                    next!(seq, "attr_perms");
                }
                if (attr_flags & AttrFlags::Time as u32) != 0 {
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
