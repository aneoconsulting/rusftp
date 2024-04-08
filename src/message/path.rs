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

use serde::{Deserialize, Serialize};

/// Path component on the remote server.
///
/// It can be a path relative to the current work directory on the remote server,
/// or it can be an absolute path.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Path(pub String);

/*
 * Conversion
 */
impl<T: Into<String>> From<T> for Path {
    fn from(value: T) -> Self {
        Path(value.into())
    }
}

impl Deref for Path {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl AsRef<[u8]> for Path {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsRef<str> for Path {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Borrow<str> for Path {
    fn borrow(&self) -> &str {
        self.0.as_ref()
    }
}

/*
 * Append
 */
impl std::ops::DivAssign<Path> for Path {
    fn div_assign(&mut self, rhs: Path) {
        *self /= rhs.0;
    }
}

impl std::ops::DivAssign<String> for Path {
    fn div_assign(&mut self, rhs: String) {
        if self.0.is_empty() {
            self.0 = rhs;
        } else {
            match rhs.chars().next() {
                None => (),
                Some('/') => {
                    self.0 = rhs;
                }
                _ => {
                    match self.0.chars().last() {
                        None => unreachable!(),
                        Some('/') => (),
                        _ => self.0.push('/'),
                    }
                    self.0.push_str(rhs.as_ref());
                }
            }
        }
    }
}

impl<T: AsRef<str>> std::ops::DivAssign<&T> for Path {
    fn div_assign(&mut self, rhs: &T) {
        let rhs = rhs.as_ref();

        if self.0.is_empty() {
            self.0 = String::from(rhs);
        } else {
            match rhs.chars().next() {
                None => (),
                Some('/') => {
                    self.0 = String::from(rhs);
                }
                _ => {
                    match self.0.chars().last() {
                        None => unreachable!(),
                        Some('/') => (),
                        _ => self.0.push('/'),
                    }
                    self.0.push_str(rhs);
                }
            }
        }
    }
}

impl<T> std::ops::Div<T> for Path
where
    Path: std::ops::DivAssign<T>,
{
    type Output = Path;

    fn div(mut self, rhs: T) -> Self::Output {
        self /= rhs;
        self
    }
}

#[cfg(test)]
mod test {
    use crate::message::test_utils::{encode_decode, fail_decode, BYTES_INVALID, BYTES_VALID};

    use super::Path;

    #[test]
    fn encode_success() {
        for (bytes, encoded) in BYTES_VALID {
            encode_decode(Path(bytes.to_owned()), encoded);
        }
    }

    #[test]
    fn decode_failure() {
        for (bytes, expected) in BYTES_INVALID {
            assert_eq!(fail_decode::<Path>(bytes), expected);
        }
    }

    #[test]
    fn path_concatenation() {
        assert_eq!(Path::from("abc"), Path::from("abc") / &"");
        assert_eq!(Path::from("abc/"), Path::from("abc/") / &"");
        assert_eq!(Path::from("def"), Path::from("") / &"def");
        assert_eq!(Path::from("/def"), Path::from("/") / &"def");
        assert_eq!(Path::from("abc/def"), Path::from("abc") / &"def");
        assert_eq!(Path::from("abc/def"), Path::from("abc/") / &"def");
        assert_eq!(Path::from("/def"), Path::from("abc") / &"/def");
        assert_eq!(Path::from("/def"), Path::from("abc/") / &"/def");

        assert_eq!(Path::from("abc"), Path::from("abc") / String::from(""));
        assert_eq!(Path::from("abc/"), Path::from("abc/") / String::from(""));
        assert_eq!(Path::from("def"), Path::from("") / String::from("def"));
        assert_eq!(Path::from("/def"), Path::from("/") / String::from("def"));
        assert_eq!(
            Path::from("abc/def"),
            Path::from("abc") / String::from("def")
        );
        assert_eq!(
            Path::from("abc/def"),
            Path::from("abc/") / String::from("def")
        );
        assert_eq!(Path::from("/def"), Path::from("abc") / String::from("/def"));
        assert_eq!(
            Path::from("/def"),
            Path::from("abc/") / String::from("/def")
        );

        assert_eq!(Path::from("abc"), Path::from("abc") / Path::from(""));
        assert_eq!(Path::from("abc/"), Path::from("abc/") / Path::from(""));
        assert_eq!(Path::from("def"), Path::from("") / Path::from("def"));
        assert_eq!(Path::from("/def"), Path::from("/") / Path::from("def"));
        assert_eq!(Path::from("abc/def"), Path::from("abc") / Path::from("def"));
        assert_eq!(
            Path::from("abc/def"),
            Path::from("abc/") / Path::from("def")
        );
        assert_eq!(Path::from("/def"), Path::from("abc") / Path::from("/def"));
        assert_eq!(Path::from("/def"), Path::from("abc/") / Path::from("/def"));
    }
}
