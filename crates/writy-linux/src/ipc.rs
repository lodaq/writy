use crate::protocol::Command;
use anyhow::{Context, Result, ensure};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

pub fn runtime_dir() -> Result<PathBuf> {
    let base =
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is required")?);
    let meta = std::fs::symlink_metadata(&base)?;
    ensure!(
        meta.is_dir() && meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o077 == 0,
        "runtime directory must be private and owned by this user"
    );
    let dir = base.join("writy");
    match std::fs::create_dir(&dir) {
        Ok(()) => std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let meta = std::fs::symlink_metadata(&dir)?;
    ensure!(
        meta.is_dir() && meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o077 == 0,
        "Writy runtime directory must be private and not a symlink"
    );
    Ok(dir)
}
pub async fn bind(name: &str) -> Result<UnixListener> {
    let path = runtime_dir()?.join(name);
    if path.exists() {
        ensure!(
            UnixStream::connect(&path).await.is_err(),
            "Writy is already running"
        );
        let meta = std::fs::symlink_metadata(&path)?;
        ensure!(
            meta.uid() == unsafe { libc::geteuid() }
                && std::os::unix::fs::FileTypeExt::is_socket(&meta.file_type()),
            "refusing to remove non-socket path"
        );
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}
pub fn same_user(stream: &UnixStream) -> bool {
    stream
        .peer_cred()
        .is_ok_and(|c| c.uid() == unsafe { libc::geteuid() })
}
pub async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    let n = reader
        .take(limit as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    if n == 0 {
        return Ok(None);
    }
    ensure!(
        n <= limit && bytes.last() == Some(&b'\n'),
        "invalid or oversized IPC frame"
    );
    Ok(Some(String::from_utf8(bytes)?))
}
pub async fn send(command: &Command) -> Result<String> {
    let path = runtime_dir()?.join("control.sock");
    let mut stream = UnixStream::connect(path)
        .await
        .context("Writy is not running; start `writy daemon`")?;
    let mut bytes = serde_json::to_vec(command)?;
    bytes.push(b'\n');
    stream.write_all(&bytes).await?;
    let mut reader = BufReader::new(stream);
    Ok(tokio::time::timeout(
        std::time::Duration::from_secs(3),
        read_line(&mut reader, 16384),
    )
    .await??
    .unwrap_or_default())
}
pub struct SocketCleanup(pub PathBuf);
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn database_path() -> PathBuf {
    writy_core::config::data_dir().join("dictionary.sqlite3")
}
pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn frames_are_bounded_even_without_newline() {
        let mut input = BufReader::new(&b"0123456789"[..]);
        assert!(read_line(&mut input, 4).await.is_err());
        let mut input = BufReader::new(&b"{}\n"[..]);
        assert_eq!(
            read_line(&mut input, 4).await.unwrap().as_deref(),
            Some("{}\n")
        );
    }
}
