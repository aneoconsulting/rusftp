use std::sync::Arc;

use async_trait::async_trait;

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
    let config = Arc::new(russh::client::Config::default());
    let mut ssh = russh::client::connect(config, ("localhost", 2222), Handler)
        .await
        .unwrap();

    ssh.authenticate_password("user", "pass").await.unwrap();
    let sftp = rusftp::SftpClient::new(ssh).await.unwrap();

    let cwd = sftp.stat(rusftp::Stat { path: ".".into() }).await.unwrap();
    println!("CWD: {:?}", cwd);
}
