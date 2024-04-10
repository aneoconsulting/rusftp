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

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use rusftp::{client::SftpClient, message::PFlags};

struct Handler;

#[async_trait]
impl russh::client::Handler for Handler {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    // You can start a sftp server configured for this client with the following command:
    //
    // docker run -v /tmp:/home/user/tmp -p 2222:22 --rm atmoz/sftp:alpine user:pass:1000

    println!("> Connect to the ssh server");
    let config = Arc::new(russh::client::Config::default());
    let mut ssh = russh::client::connect(config, ("127.0.0.1", 2222), Handler).await?;
    ssh.authenticate_password("user", "pass").await?;

    println!("> Start SFTP client");
    let mut sftp = SftpClient::new(ssh).await?;

    println!("> Create a directory");
    sftp.mkdir("/tmp/dir").await?;

    println!("> Create a symlink");
    sftp.symlink("/tmp/dummy.txt", "/tmp/dir/link").await?;

    println!("> Open a file for reading and writing");
    let mut file = sftp
        .open_with_flags(
            "tmp/dir/link",
            PFlags::CREATE | PFlags::READ | PFlags::WRITE,
        )
        .await?;

    println!("> Write content to the file");
    file.write_all(b"Hello world!").await?;

    println!("> Seek back to the start of the file");
    file.seek(std::io::SeekFrom::Start(0)).await?;

    println!("> Read the whole content of the file");
    let mut content = String::new();
    file.read_to_string(&mut content).await?;
    println!("File content: {content:?}");

    println!("> Close the file");
    // optional, file is closed is performed when dropped
    file.close().await?;

    println!("> Get informations");
    println!("stat: {:?}", sftp.stat("/tmp/dir/link").await?);
    println!("lstat: {:?}", sftp.lstat("/tmp/dir/link").await?);
    println!("readlink: {:?}", sftp.readlink("/tmp/dir/link").await?);
    println!("realpath: {:?}", sftp.realpath("/tmp/dir/link").await?);

    println!("> Read dir");
    let mut dir = sftp.opendir("/tmp/dir").await?;

    while let Some(entry) = dir.next().await {
        println!("{:?}", entry?);
    }

    dir.close().await?;

    println!("> Remove both file and link");
    let (a, b) = tokio::join!(sftp.remove("/tmp/dir/link"), sftp.remove("/tmp/dummy.txt"));
    a?;
    b?;

    println!("> Read dir");
    for entry in sftp.readdir("/tmp/dir").await? {
        println!("{:?}", entry);
    }

    println!("> Remove directory");
    sftp.rmdir("/tmp/dir").await?;

    println!("> Stat");
    let cwd = sftp.stat(".");

    println!("> Stop sftp client");
    // optional, sftp is stopped when dropped
    sftp.stop().await;

    println!("CWD: {:?}", cwd.await?);

    Ok(())
}
