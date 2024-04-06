use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

use crate::{
    wire::{SftpDecoder, SftpEncoder, WireFormatError},
    Attrs, Owner, Permisions, Time,
};

pub(crate) fn encode_decode<T>(value: T, expected: &[u8])
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let mut serializer = SftpEncoder::with_vec(Vec::new());
    let input = value;
    input
        .serialize(&mut serializer)
        .expect("Serialization should succeed");
    let encoded = serializer.buf.as_slice();
    assert_eq!(encoded, expected);
    let mut deserializer = SftpDecoder::new(encoded);
    let output = T::deserialize(&mut deserializer)
        .unwrap_or_else(|err| panic!("Deserialization of {:?} should succeed: {:?}", encoded, err));

    assert_eq!(input, output);
}

pub(crate) fn fail_decode<T>(encoded: &[u8]) -> WireFormatError
where
    T: DeserializeOwned + Debug,
{
    let mut deserializer = SftpDecoder::new(encoded);
    match T::deserialize(&mut deserializer) {
        Ok(val) => panic!("Deserialization of {:?} should fail: {:?}", encoded, val),
        Err(err) => err,
    }
}

pub(crate) const BYTES_VALID: [(&[u8], &[u8]); 8] = [
    (b"" as &[u8], b"\0\0\0\0" as &[u8]),
    (b"\0", b"\0\0\0\x01\0"),
    (b"\0\0", b"\0\0\0\x02\0\0"),
    (b"byte string", b"\0\0\0\x0bbyte string"),
    (
        b"byte string with\nline returns and spaces",
        b"\0\0\0\x28byte string with\nline returns and spaces",
    ),
    (b"null\0bytes", b"\0\0\0\x0anull\0bytes"),
    (b"null\0bytes\0", b"\0\0\0\x0bnull\0bytes\0"),
    (
        b"this is a very long byte string that should be larger than 256 bytes and would therefore need a size encoded into two bytes. This serve as a good example of very long messages to ensure the logic is well defined for large inputs like this one. And I ran out of description for this very long byte string so here it is: Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
        b"\0\0\x01\xbathis is a very long byte string that should be larger than 256 bytes and would therefore need a size encoded into two bytes. This serve as a good example of very long messages to ensure the logic is well defined for large inputs like this one. And I ran out of description for this very long byte string so here it is: Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
    ),
];

pub(crate) const BYTES_INVALID: [(&[u8], WireFormatError); 2] = [
    (b"" as &[u8], WireFormatError::NotEnoughData),
    (b"\0\0\0\x01", WireFormatError::NotEnoughData),
];

//   Permisions::OW
// | Permisions::GW
// | Permisions::GR
// | Permisions::SX
// | Permisions::SW
// | Permisions::FIFO
// | Permisions::CHR
// | Permisions::DIR
pub(crate) const PERMISSIONS_EXAMPLE: Permisions = Permisions::from_bits_retain(0x00007632);

pub(crate) const ATTRS_VALID: [(Attrs, &[u8]); 20] = [
    // Default
    (Attrs::new(), b"\0\0\0\0" as &[u8]),
    // Size
    (
        Attrs {
            size: Some(0),
            ..Attrs::new()
        },
        b"\0\0\0\x01\0\0\0\0\0\0\0\0",
    ),
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            ..Attrs::new()
        },
        b"\0\0\0\x01\xfe\xdc\xba\x98\x76\x54\x32\x10",
    ),
    // owner
    (
        Attrs {
            owner: Some(Owner { uid: 0, gid: 0 }),
            ..Attrs::new()
        },
        b"\0\0\0\x02\0\0\0\0\0\0\0\0",
    ),
    (
        Attrs {
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x02\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80",
    ),
    // perms
    (
        Attrs {
            perms: Some(Permisions::empty()),
            ..Attrs::new()
        },
        b"\0\0\0\x04\0\0\0\0",
    ),
    (
        Attrs {
            perms: Some(PERMISSIONS_EXAMPLE),
            ..Attrs::new()
        },
        b"\0\0\0\x04\0\0\x76\x32",
    ),
    // time
    (
        Attrs {
            time: Some(Time { atime: 0, mtime: 0 }),
            ..Attrs::new()
        },
        b"\0\0\0\x08\0\0\0\0\0\0\0\0",
    ),
    (
        Attrs {
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x08\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // size + owner
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x03\xfe\xdc\xba\x98\x76\x54\x32\x10\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80",
    ),
    // size + perms
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            perms: Some(PERMISSIONS_EXAMPLE),
            ..Attrs::new()
        },
        b"\0\0\0\x05\xfe\xdc\xba\x98\x76\x54\x32\x10\0\0\x76\x32",
    ),
    // size + time
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x09\xfe\xdc\xba\x98\x76\x54\x32\x10\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // owner + perms
    (
        Attrs {
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            perms: Some(PERMISSIONS_EXAMPLE),
            ..Attrs::new()
        },
        b"\0\0\0\x06\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\0\0\x76\x32",
    ),
    // owner + time
    (
        Attrs {
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x0a\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // perms + time
    (
        Attrs {
            perms: Some(PERMISSIONS_EXAMPLE),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x0c\0\0\x76\x32\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // size + owner + perms
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            perms: Some(PERMISSIONS_EXAMPLE),
            ..Attrs::new()
        },
        b"\0\0\0\x07\xfe\xdc\xba\x98\x76\x54\x32\x10\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\0\0\x76\x32",
    ),
    // size + owner + time
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x0b\xfe\xdc\xba\x98\x76\x54\x32\x10\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // size + perms + time
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            perms: Some(PERMISSIONS_EXAMPLE),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x0d\xfe\xdc\xba\x98\x76\x54\x32\x10\0\0\x76\x32\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // owner + perms + time
    (
        Attrs {
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            perms: Some(PERMISSIONS_EXAMPLE),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
            ..Attrs::new()
        },
        b"\0\0\0\x0e\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\0\0\x76\x32\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
    // size + owner + perms + time
    (
        Attrs {
            size: Some(0xfedcba9876543210),
            owner: Some(Owner {
                uid: 0xf7e6d5c4,
                gid: 0xb3a29180,
            }),
            perms: Some(PERMISSIONS_EXAMPLE),
            time: Some(Time {
                atime: 0xfdb97531,
                mtime: 0xeca86420,
            }),
        },
        b"\0\0\0\x0f\xfe\xdc\xba\x98\x76\x54\x32\x10\xf7\xe6\xd5\xc4\xb3\xa2\x91\x80\0\0\x76\x32\xfd\xb9\x75\x31\xec\xa8\x64\x20",
    ),
];
