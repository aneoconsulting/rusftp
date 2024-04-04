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
    Attrs, Close, Data, Dir, Extended, FSetStat, FStat, File, Handle, LStat, MkDir, Name, Open,
    OpenDir, PFlags, Path, Read, ReadDir, ReadLink, RealPath, Remove, Rename, RmDir, SetStat,
    SftpClient, Stat, Status, StatusCode, Symlink, Write,
};

impl SftpClient {
    /// Close an opened file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the file or the directory (convertible to [`Handle`])
    pub fn close<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Close {
            handle: handle.into(),
        })
    }

    /// Send an extended request.
    ///
    /// # Arguments
    ///
    /// * `request` - Extended-request name (format: `name@domain`, convertible to [`Bytes`])
    /// * `data` - Specific data needed by the extension to intrepret the request (convertible to [`Bytes`])
    pub fn extended<R: Into<Bytes>, D: Into<Bytes>>(
        &self,
        request: R,
        data: D,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
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
    /// * `handle` - Handle of the file or directory to change the attributes (convertible to [`Handle`])
    /// * `attrs` - New attributes to apply (convertible to [`Attrs`])
    pub fn fsetstat<H: Into<Handle>, A: Into<Attrs>>(
        &self,
        handle: H,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(FSetStat {
            handle: handle.into(),
            attrs: attrs.into(),
        })
    }

    /// Read the attributes (metadata) of an open file or directory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle of the open file or directory (convertible to [`Handle`])
    pub fn fstat<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(FStat {
            handle: handle.into(),
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links are followed.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file, directory, or symbolic link (convertible to [`Path`])
    pub fn lstat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(LStat { path: path.into() })
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located (convertible to [`Path`])
    pub fn mkdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.mkdir_with_attrs(path, Attrs::default())
    }

    /// Create a new directory.
    ///
    /// An error will be returned if a file or directory with the specified path already exists.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the new directory will be located (convertible to [`Path`])
    /// * `attrs` - Default attributes to apply to the newly created directory (convertible to [`Attrs`])
    pub fn mkdir_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(MkDir {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns an [`Handle`](struct@crate::Handle) for the file specified.
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_handle<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.request(Open {
            filename: filename.into(),
            pflags: pflags.into(),
            attrs: attrs.into(),
        })
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_with_flags_attrs<P: Into<Path>, F: Into<PFlags>, A: Into<Attrs>>(
        &self,
        filename: P,
        pflags: F,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
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
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `pflags` - Flags for the file opening (convertible to [`PFlags`])
    pub fn open_with_flags<P: Into<Path>, F: Into<PFlags>>(
        &self,
        filename: P,
        pflags: F,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, pflags, Attrs::default())
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    /// * `attrs` - Default file attributes to use upon file creation (convertible to [`Attrs`])
    pub fn open_with_attrs<P: Into<Path>, A: Into<Attrs>>(
        &self,
        filename: P,
        attrs: A,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
        self.open_with_flags_attrs(filename, PFlags::default(), attrs)
    }

    /// Open a file for reading or writing.
    ///
    /// Returns a [`File`] object that is compatible with [`tokio::io`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the file to open (convertible to [`Path`])
    pub fn open<P: Into<Path>>(
        &self,
        filename: P,
    ) -> impl Future<Output = Result<File, Status>> + Send + Sync + 'static {
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
    /// * `path` - Path of the directory to open (convertible to [`Path`])
    pub fn opendir_handle<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Handle, Status>> + Send + Sync + 'static {
        self.request(OpenDir { path: path.into() })
    }

    /// Open a directory for listing.
    ///
    /// Returns a [`Dir`] for the directory specified.
    /// It implements [`Stream<Item = Result<NameEntry, ...>>`](futures::stream::Stream).
    ///
    /// # Arguments
    ///
    /// * `path` - Path of the directory to open (convertible to [`Path`])
    pub fn opendir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Dir, Status>> + Send + Sync + 'static {
        let request = self.request(OpenDir { path: path.into() });
        let client = self.clone();

        async move { Ok(Dir::new(client, request.await?)) }
    }

    /// Read a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to read from (convertible to [`Handle`])
    /// * `offset`: Byte offset where the read should start
    /// * `length`: Number of bytes to read
    pub fn read<H: Into<Handle>>(
        &self,
        handle: H,
        offset: u64,
        length: u32,
    ) -> impl Future<Output = Result<Bytes, Status>> + Send + Sync + 'static {
        let request = self.request(Read {
            handle: handle.into(),
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
    /// * `handle`: Handle of the open directory (convertible to [`Handle`])
    pub fn readdir_handle<H: Into<Handle>>(
        &self,
        handle: H,
    ) -> impl Future<Output = Result<Name, Status>> + Send + Sync + 'static {
        self.request(ReadDir {
            handle: handle.into(),
        })
    }

    /// Read a directory listing.
    ///
    /// If you need an asynchronous [`Stream`](futures::stream::Stream), you can use `opendir()` instead
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the directory to list (convertible to [`Path`])
    pub fn readdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Name, Status>> + Send + Sync + 'static {
        let dir = self.request(OpenDir { path: path.into() });
        let client = self.clone();
        let mut entries = Name::default();

        async move {
            let handle = dir.await?;

            loop {
                match client.readdir_handle(handle.clone()).await {
                    Ok(mut chunk) => entries.0.append(&mut chunk.0),
                    Err(Status {
                        code: StatusCode::Eof,
                        ..
                    }) => break,
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
    /// * `path`: Path of the symbolic link to read (convertible to [`Path`])
    pub fn readlink<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.request(ReadLink { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    /// Canonicalize a path.
    ///
    /// # Arguments
    ///
    /// * `path`: Path to canonicalize (convertible to [`Path`])
    pub fn realpath<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Path, Status>> + Send + Sync + 'static {
        let request = self.request(RealPath { path: path.into() });

        async move {
            match request.await?.as_mut() {
                [] => Err(StatusCode::NoSuchFile.to_status("No entry".into())),
                [entry] => Ok(std::mem::take(entry).filename),
                _ => Err(StatusCode::BadMessage.to_status("Multiple entries".into())),
            }
        }
    }

    /// Remove a file.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file to remove (convertible to [`Path`])
    pub fn remove<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Remove { path: path.into() })
    }

    /// Rename/move a file or a directory.
    ///
    /// # Arguments
    ///
    /// * `old_path`: Current path of the file or directory to rename/move (convertible to [`Path`])
    /// * `new_path`: New path where the file or directory will be moved to (convertible to [`Path`])
    pub fn rename<O: Into<Path>, N: Into<Path>>(
        &self,
        old_path: O,
        new_path: N,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
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
    /// * `path`: Path of the directory to remove (convertible to [`Path`])
    pub fn rmdir<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
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
    /// * `path`: Path of the file or directory to change the attributes (convertible to [`Path`])
    /// * `attrs`: New attributes to apply (convertible to [`Attrs`])
    pub fn setstat<P: Into<Path>, A: Into<Attrs>>(
        &self,
        path: P,
        attrs: A,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(SetStat {
            path: path.into(),
            attrs: attrs.into(),
        })
    }

    /// Read the attributes (metadata) of a file or directory.
    ///
    /// Symbolic links *are not* followed.
    ///
    /// # Arguments
    ///
    /// * `path`: Path of the file or directory (convertible to [`Path`])
    pub fn stat<P: Into<Path>>(
        &self,
        path: P,
    ) -> impl Future<Output = Result<Attrs, Status>> + Send + Sync + 'static {
        self.request(Stat { path: path.into() })
    }

    /// Create a symbolic link.
    ///
    /// # Arguments
    ///
    /// * `link_path`: Path name of the symbolic link to be created (convertible to [`Path`])
    /// * `target_path`: Target of the symbolic link (convertible to [`Path`])
    pub fn symlink<L: Into<Path>, T: Into<Path>>(
        &self,
        link_path: L,
        target_path: T,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Symlink {
            link_path: link_path.into(),
            target_path: target_path.into(),
        })
    }

    /// Write to a portion of an opened file.
    ///
    /// # Arguments
    ///
    /// * `handle`: Handle of the file to write to (convertible to [`Handle`])
    /// * `offset`: Byte offset where the write should start
    /// * `data`: Bytes to be written to the file
    pub fn write<H: Into<Handle>, D: Into<Bytes>>(
        &self,
        handle: H,
        offset: u64,
        data: D,
    ) -> impl Future<Output = Result<(), Status>> + Send + Sync + 'static {
        self.request(Write {
            handle: handle.into(),
            offset,
            data: Data(data.into()),
        })
    }
}
