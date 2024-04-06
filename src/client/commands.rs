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

use crate::{
    Attrs, ClientError, Close, Data, Dir, Extended, FSetStat, FStat, File, Handle, LStat, MkDir,
    Name, Open, OpenDir, PFlags, Path, Read, ReadDir, ReadLink, RealPath, Remove, Rename, RmDir,
    SetStat, SftpClient, Stat, Status, StatusCode, Symlink, Write,
};

impl SftpClient {
    /// Close an opened file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or the directory
    pub fn close(
        &self,
        handle: Handle,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(Close { handle })
    }

    /// Send an extended request.
    ///
    /// # Arguments
    ///
    /// * `request` - Extended-request name (format: `name@domain`)
    /// * `data` - Specific data needed by the extension to intrepret the request
    pub fn extended(
        &self,
        request: impl Into<Bytes>,
        data: impl Into<Bytes>,
    ) -> impl Future<Output = Result<Bytes, ClientError>> + Send + Sync + 'static {
        let request = self.request(Extended {
            request: request.into(),
            data: data.into(),
        });
        async move { Ok(request.await?.data) }
    }

    /// Change the attributes (metadata) of an open file or directory.
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
    pub fn fsetstat(
        &self,
        handle: Handle,
        attrs: Attrs,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(FSetStat { handle, attrs })
    }

    /// Read the attributes (metadata) of an open file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open file or directory
    pub fn fstat(
        &self,
        handle: Handle,
    ) -> impl Future<Output = Result<Attrs, ClientError>> + Send + Sync + 'static {
        self.request(FStat { handle })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links are followed.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file, directory, or symbolic link
    pub fn lstat(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Attrs, ClientError>> + Send + Sync + 'static {
        self.request(LStat { path: path.into() })
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located
    pub fn mkdir(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.mkdir_with_attrs(path, Attrs::default())
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located
    /// * `attrs` - Default attributes to apply to the newly created directory
    pub fn mkdir_with_attrs(
        &self,
        path: impl Into<Path>,
        attrs: Attrs,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(MkDir {
            path: path.into(),
            attrs,
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns an [`Handle`](struct@crate::Handle) for the file specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    /// * `attrs` - Default file attributes to use upon file creation
    pub fn open_handle(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
        attrs: Attrs,
    ) -> impl Future<Output = Result<Handle, ClientError>> + Send + Sync + 'static {
        self.request(Open {
            filename: filename.into(),
            pflags,
            attrs,
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    /// * `attrs` - Default file attributes to use upon file creation
    pub fn open_with_flags_attrs(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
        attrs: Attrs,
    ) -> impl Future<Output = Result<File, ClientError>> + Send + Sync + 'static {
        let request = self.open_handle(filename, pflags, attrs);
        let client = self.clone();

        async move { Ok(File::new(client, request.await?)) }
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `pflags` - Flags for the file opening
    pub fn open_with_flags(
        &self,
        filename: impl Into<Path>,
        pflags: PFlags,
    ) -> impl Future<Output = Result<File, ClientError>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, pflags, Attrs::default())
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    /// * `attrs` - Default file attributes to use upon file creation
    pub fn open_with_attrs(
        &self,
        filename: impl Into<Path>,
        attrs: Attrs,
    ) -> impl Future<Output = Result<File, ClientError>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, PFlags::default(), attrs)
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open
    pub fn open(
        &self,
        filename: impl Into<Path>,
    ) -> impl Future<Output = Result<File, ClientError>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, PFlags::default(), Attrs::default())
    }

    /// Open a directory for listing.
    ///
    /// Once the directory has been successfully opened, files (and directories)
    /// contained in it can be listed using `readdir_handle`.
    ///
    /// Returns an [`Handle`] for the directory specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open
    pub fn opendir_handle(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Handle, ClientError>> + Send + Sync + 'static {
        self.request(OpenDir { path: path.into() })
    }

    /// Open a directory for listing.
    ///
    /// Returns a [`Dir`] for the directory specified.
    /// It implements [`Stream<Item = Result<NameEntry, ...>>`](futures::stream::Stream).
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open
    pub fn opendir(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Dir, ClientError>> + Send + Sync + 'static {
        let request = self.request(OpenDir { path: path.into() });
        let client = self.clone();

        async move { Ok(Dir::new(client, request.await?)) }
    }

    /// Read a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to read from
    /// * `offset`: Byte offset where the read should start
    /// * `length`: Number of bytes to read
    pub fn read(
        &self,
        handle: Handle,
        offset: u64,
        length: u32,
    ) -> impl Future<Output = Result<Bytes, ClientError>> + Send + Sync + 'static {
        let request = self.request(Read {
            handle,
            offset,
            length,
        });

        async move { Ok(request.await?.0) }
    }

    /// Read a directory listing.
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
    pub fn readdir_handle(
        &self,
        handle: Handle,
    ) -> impl Future<Output = Result<Name, ClientError>> + Send + Sync + 'static {
        self.request(ReadDir { handle })
    }

    /// Read a directory listing.
    ///
    /// If you need an asynchronous [`Stream`](futures::stream::Stream), you can use `opendir()` instead
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to list
    pub fn readdir(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Name, ClientError>> + Send + Sync + 'static {
        let dir = self.request(OpenDir { path: path.into() });
        let client = self.clone();
        let mut entries = Name::default();

        async move {
            let handle = dir.await?;

            loop {
                match client.readdir_handle(handle.clone()).await {
                    Ok(mut chunk) => entries.0.append(&mut chunk.0),
                    Err(ClientError::Sftp(Status {
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
    /// # Arguments
    ///
    /// * `path`: Path of the symbolic link to read
    pub fn readlink(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Path, ClientError>> + Send + Sync + 'static {
        let request = self.request(ReadLink { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(ClientError::Sftp(
                    StatusCode::BadMessage.to_status("No entry".into()),
                )),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(ClientError::Sftp(
                    StatusCode::BadMessage.to_status("Multiple entries".into()),
                )),
            }
        }
    }

    /// Canonicalize a path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to canonicalize
    pub fn realpath(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Path, ClientError>> + Send + Sync + 'static {
        let request = self.request(RealPath { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(ClientError::Sftp(
                    StatusCode::BadMessage.to_status("No entry".into()),
                )),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(ClientError::Sftp(
                    StatusCode::BadMessage.to_status("Multiple entries".into()),
                )),
            }
        }
    }

    /// Remove a file.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file to remove
    pub fn remove(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(Remove { path: path.into() })
    }

    /// Rename/move a file or a directory.
    ///
    /// # Arguments
    ///
    /// * `old_path`: Current path of the file or directory to rename/move
    /// * `new_path`: New path where the file or directory will be moved to
    pub fn rename(
        &self,
        old_path: impl Into<Path>,
        new_path: impl Into<Path>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(Rename {
            old_path: old_path.into(),
            new_path: new_path.into(),
        })
    }

    /// Remove an existing directory.
    ///
    /// An error will be returned if no directory with the specified path exists,
    /// or if the specified directory is not empty, or if the path specified
    /// a file system object other than a directory.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to remove
    pub fn rmdir(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(RmDir { path: path.into() })
    }

    /// Change the attributes (metadata) of a file or directory.
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
    pub fn setstat(
        &self,
        path: impl Into<Path>,
        attrs: Attrs,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(SetStat {
            path: path.into(),
            attrs,
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links *are not* followed.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory
    pub fn stat(
        &self,
        path: impl Into<Path>,
    ) -> impl Future<Output = Result<Attrs, ClientError>> + Send + Sync + 'static {
        self.request(Stat { path: path.into() })
    }

    /// Create a symbolic link.
    ///
    /// # Arguments
    ///
    /// * `link_path`: Path name of the symbolic link to be created
    /// * `target_path`: Target of the symbolic link
    pub fn symlink(
        &self,
        link_path: impl Into<Path>,
        target_path: impl Into<Path>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(Symlink {
            link_path: link_path.into(),
            target_path: target_path.into(),
        })
    }

    /// Write to a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to write to
    /// * `offset`: Byte offset where the write should start
    /// * `data`: Bytes to be written to the file
    pub fn write(
        &self,
        handle: Handle,
        offset: u64,
        data: impl Into<Data>,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + Sync + 'static {
        self.request(Write {
            handle,
            offset,
            data: data.into(),
        })
    }
}
