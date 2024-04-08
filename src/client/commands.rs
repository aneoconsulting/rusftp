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

use bytes::Bytes;
use futures::Future;

use crate::client::{Dir, Error, File, SftpClient, SftpFuture, SftpReply, SftpRequest, StatusCode};
use crate::message::{
    Attrs, Close, Data, Extended, ExtendedReply, FSetStat, FStat, Handle, LStat, Message, MkDir,
    Name, Open, OpenDir, PFlags, Path, Read, ReadDir, ReadLink, RealPath, Remove, Rename, RmDir,
    SetStat, Stat, Status, Symlink, Write,
};
use crate::utils::IntoBytes;

impl SftpClient {
    /// Close an opened file or directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn close(&self, handle: Handle) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or the directory
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn close(&self, handle: Handle) -> SftpFuture {
        self.request(Close { handle })
    }

    /// Send an extended request.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn extended(&self, request: impl Into<Bytes>, data: impl Into<Bytes>) -> Result<Bytes, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `request` - Extended-request name (format: `name@domain`)
    /// * `data` - Specific data needed by the extension to intrepret the request
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn extended(&self, request: impl IntoBytes, data: impl IntoBytes) -> SftpFuture<Bytes> {
        self.request_with(
            Extended {
                request: request.into_bytes(),
                data: data.into_bytes(),
            }
            .to_request_message(),
            (),
            |_, msg| Ok(ExtendedReply::from_reply_message(msg)?.data),
        )
    }

    /// Change the attributes (metadata) of an open file or directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn fsetstat(&self, handle: Handle, attrs: Attrs) -> Result<(), Error>;
    /// ```
    ///
    /// This operation is used for operations such as changing the ownership,
    /// permissions or access times, as well as for truncating a file.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or directory to change the attributes
    /// * `attrs` - New attributes to apply
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn fsetstat(&self, handle: Handle, attrs: Attrs) -> SftpFuture {
        self.request(FSetStat { handle, attrs })
    }

    /// Read the attributes (metadata) of an open file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn fstat(&self, handle: Handle) -> Result<Attrs, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open file
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn fstat(&self, handle: Handle) -> SftpFuture<Attrs> {
        self.request(FStat { handle })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn lstat(&self, path: impl Into<Path>) -> Result<Attrs, Error>;
    /// ```
    ///
    /// Symbolic links are followed.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file, directory, or symbolic link
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn lstat(&self, path: impl Into<Path>) -> SftpFuture<Attrs> {
        self.request(LStat { path: path.into() })
    }

    /// Create a new directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn mkdir(&self, path: impl Into<Path>) -> Result<(), Error>;
    /// ```
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn mkdir(&self, path: impl Into<Path>) -> SftpFuture {
        self.mkdir_with_attrs(path, Attrs::default())
    }

    /// Create a new directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn mkdir_with_attrs(&self, path: impl Into<Path>, attrs: Attrs) -> Result<(), Error>;
    /// ```
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located
    /// * `attrs` - Default attributes to apply to the newly created directory
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn mkdir_with_attrs(&self, path: impl Into<Path>, attrs: Attrs) -> SftpFuture {
        self.request(MkDir {
            path: path.into(),
            attrs,
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn open_handle(&self, filename: impl Into<Path>, pflags: PFlags, attrs: Attrs) -> Result<Handle, Error>;
    /// ```
    ///
    /// Returns an [`Handle`] for the file specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    /// * `attrs` - Default file attributes to use upon file creation
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn open_handle(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
        attrs: Attrs,
    ) -> SftpFuture<Handle> {
        self.request(Open {
            filename: filename.into(),
            pflags,
            attrs,
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn open_with_flags_attrs(&self, filename: impl Into<Path>, pflags: PFlags, attrs: Attrs) -> Result<File, Error>;
    /// ```
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    /// * `attrs` - Default file attributes to use upon file creation
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn open_with_flags_attrs(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
        attrs: Attrs,
    ) -> SftpFuture<File, SftpClient> {
        self.request_with(
            Open {
                filename: filename.into(),
                pflags,
                attrs,
            }
            .to_request_message(),
            self.clone(),
            |client, msg| Ok(File::new(client, Handle::from_reply_message(msg)?)),
        )
    }

    /// Open a file for reading or writing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn open_with_flags(&self, filename: impl Into<Path>, pflags: PFlags) -> Result<File, Error>;
    /// ```
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn open_with_flags(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
    ) -> SftpFuture<File, SftpClient> {
        self.open_with_flags_attrs(filename, pflags, Attrs::default())
    }

    /// Open a file for reading or writing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn open_with_attrs(&self, filename: impl Into<Path>, attrs: Attrs) -> Result<File, Error>;
    /// ```
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `attrs` - Default file attributes to use upon file creation
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn open_with_attrs(
        &self,
        filename: impl Into<Path>,
        attrs: Attrs,
    ) -> SftpFuture<File, SftpClient> {
        self.open_with_flags_attrs(filename, PFlags::default(), attrs)
    }

    /// Open a file for reading or writing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn open(&self, filename: impl Into<Path>) -> Result<File, Error>;
    /// ```
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn open(&self, filename: impl Into<Path>) -> SftpFuture<File, SftpClient> {
        self.open_with_flags_attrs(filename, PFlags::default(), Attrs::default())
    }

    /// Open a directory for listing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn opendir_handle(&self, path: impl Into<Path>) -> Result<Handle, Error>;
    /// ```
    ///
    /// Once the directory has been successfully opened, files (and directories)
    /// contained in it can be listed using `readdir_handle`.
    ///
    /// Returns an [`Handle`] for the directory specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn opendir_handle(&self, path: impl Into<Path>) -> SftpFuture<Handle> {
        self.request(OpenDir { path: path.into() })
    }

    /// Open a directory for listing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn opendir(&self, path: impl Into<Path>) -> Result<Dir, Error>;
    /// ```
    ///
    /// Returns a [`Dir`] for the directory specified.
    /// It implements [`Stream<Item = Result<NameEntry, ...>>`](futures::stream::Stream).
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn opendir(&self, path: impl Into<Path>) -> SftpFuture<Dir, SftpClient> {
        self.request_with(
            OpenDir { path: path.into() }.to_request_message(),
            self.clone(),
            |client, msg| Ok(Dir::new(client, Handle::from_reply_message(msg)?)),
        )
    }

    /// Read a portion of an opened file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn read(&self, handle: Handle, offset: u64, length: u32) -> Result<Bytes, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to read from
    /// * `offset`: Byte offset where the read should start
    /// * `length`: Number of bytes to read
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn read(&self, handle: Handle, offset: u64, length: u32) -> SftpFuture<Bytes> {
        self.request_with(
            Read {
                handle,
                offset,
                length,
            }
            .to_request_message(),
            (),
            |_, msg| Ok(Handle::from_reply_message(msg)?.0),
        )
    }

    /// Read a directory listing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn readdir_handle(&self, handle: Handle) -> Result<Name, Error>;
    /// ```
    ///
    /// Each `readdir_handle` returns one or more file names with full file attributes for each file.
    /// The client should call `readdir_handle` repeatedly until it has found the file it is looking for
    /// or until the server responds with a [`Status`] message indicating an error
    /// (normally `EOF` if there are no more files in the directory).
    /// The client should then close the handle using `close`.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the open directory
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn readdir_handle(&self, handle: Handle) -> SftpFuture<Name> {
        self.request(ReadDir { handle })
    }

    /// Read a directory listing.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn readdir(&self, path: impl Into<Path>) -> Result<Name, Error>;
    /// ```
    ///
    /// If you need an asynchronous [`Stream`](futures::stream::Stream), you can use `opendir()` instead
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to list
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn readdir(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Name, Error>> + Send + Sync + 'static {
        let dir = self.request(OpenDir { path: path.into() });
        let client = self.clone();
        let mut entries = Name::default();

        async move {
            let handle = dir.await?;

            loop {
                match client.readdir_handle(handle.clone()).await {
                    Ok(mut chunk) => entries.0.append(&mut chunk.0),
                    Err(Error::Sftp(Status {
                        code: StatusCode::Eof,
                        ..
                    })) => break,
                    Err(err) => {
                        _ = client.close(handle).await;
                        return Err(err);
                    }
                }
            }

            client.close(handle).await?;
            Ok(entries)
        }
    }

    /// Read the target of a symbolic link.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn readlink(&self, path: impl Into<Path>) -> Result<Path, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the symbolic link to read
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn readlink(&self, path: impl Into<Path>) -> SftpFuture<Path> {
        self.request_with(
            ReadLink { path: path.into() }.to_request_message(),
            (),
            extract_path_from_name_message,
        )
    }

    /// Canonicalize a path.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn realpath(&self, path: impl Into<Path>) -> Result<Path, Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path`: Path to canonicalize
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn realpath(&self, path: impl Into<Path>) -> SftpFuture<Path> {
        self.request_with(
            RealPath { path: path.into() }.to_request_message(),
            (),
            extract_path_from_name_message,
        )
    }

    /// Remove a file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn remove(&self, path: impl Into<Path>) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file to remove
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn remove(&self, path: impl Into<Path>) -> SftpFuture {
        self.request(Remove { path: path.into() })
    }

    /// Rename/move a file or a directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn rename(&self, old_path: impl Into<Path>, new_path: impl Into<Path>) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `old_path`: Current path of the file or directory to rename/move
    /// * `new_path`: New path where the file or directory will be moved to
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn rename(&self, old_path: impl Into<Path>, new_path: impl Into<Path>) -> SftpFuture {
        self.request(Rename {
            old_path: old_path.into(),
            new_path: new_path.into(),
        })
    }

    /// Remove an existing directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn rmdir(&self, path: impl Into<Path>) -> Result<(), Error>;
    /// ```
    ///
    /// An error will be returned if no directory with the specified path exists,
    /// or if the specified directory is not empty, or if the path specified
    /// a file system object other than a directory.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to remove
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn rmdir(&self, path: impl Into<Path>) -> SftpFuture {
        self.request(RmDir { path: path.into() })
    }

    /// Change the attributes (metadata) of a file or directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn setstat(&self, path: impl Into<Path>, attrs: Attrs) -> Result<(), Error>;
    /// ```
    ///
    /// This request is used for operations such as changing the ownership,
    /// permissions or access times, as well as for truncating a file.
    ///
    /// An error will be returned if the specified file system object does not exist
    /// or the user does not have sufficient rights to modify the specified attributes.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory to change the attributes
    /// * `attrs`: New attributes to apply
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn setstat(&self, path: impl Into<Path>, attrs: Attrs) -> SftpFuture {
        self.request(SetStat {
            path: path.into(),
            attrs,
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn stat(&self, path: impl Into<Path>) -> Result<Attrs, Error>;
    /// ```
    ///
    /// Symbolic links *are not* followed.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn stat(&self, path: impl Into<Path>) -> SftpFuture<Attrs> {
        self.request(Stat { path: path.into() })
    }

    /// Create a symbolic link.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn symlink(&self, link_path: impl Into<Path>, target_path: impl Into<Path>) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `link_path`: Path name of the symbolic link to be created
    /// * `target_path`: Target of the symbolic link
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn symlink(&self, link_path: impl Into<Path>, target_path: impl Into<Path>) -> SftpFuture {
        self.request(Symlink {
            link_path: link_path.into(),
            target_path: target_path.into(),
        })
    }

    /// Write to a portion of an opened file.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn write(&self, handle: Handle, offset: u64, data: impl Into<Data>,) -> Result<(), Error>;
    /// ```
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to write to
    /// * `offset`: Byte offset where the write should start
    /// * `data`: Bytes to be written to the file
    ///
    /// # Cancel safety
    ///
    /// It is safe to cancel the future.
    /// However, the request is actually sent before the future is returned.
    pub fn write(&self, handle: Handle, offset: u64, data: impl Into<Data>) -> SftpFuture {
        self.request(Write {
            handle,
            offset,
            data: data.into(),
        })
    }
}

/// Convert a SFTP message into [`Name`], and extract its only entry.
/// It fails if the message is not a [`Name`], or if it has not exactly one entry.
fn extract_path_from_name_message(_: (), msg: Message) -> Result<Path, Error> {
    match Name::from_reply_message(msg)?.as_mut() {
        [] => Err(Error::Sftp(StatusCode::BadMessage.to_status("No entry"))),
        [entry] => Ok(std::mem::take(entry).filename),
        _ => Err(Error::Sftp(
            StatusCode::BadMessage.to_status("Multiple entries"),
        )),
    }
}
