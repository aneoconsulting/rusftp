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
use rusftp::RealPath;

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
pub async fn main() {
    // You can start a sftp server configured for this client with the following command:
    //
    // docker run -p 2222:22 --rm atmoz/sftp:alpine user:pass:1000

    let config = Arc::new(russh::client::Config::default());
    let mut ssh = russh::client::connect(config, ("localhost", 2222), Handler)
        .await
        .unwrap();

    ssh.authenticate_password("user", "pass").await.unwrap();
    let sftp = rusftp::SftpClient::new(ssh).await.unwrap();

    let cwd = sftp.stat(rusftp::Stat { path: ".".into() }).await.unwrap();
    println!("CWD: {:?}", cwd);

    let realpath = sftp.realpath(RealPath { path: ".".into() }).await.unwrap();
    println!("RealPath: {:?}", realpath);
}
