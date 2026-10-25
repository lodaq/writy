use crate::types::{Attribution, Language};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha512};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Versioned, open bilingual packs; data keeps its own license independent of the app.
#[derive(Clone, Copy, Debug)]
pub enum OpenPack {
    EnglishArabic,
    ArabicEnglish,
}
impl OpenPack {
    pub fn id(self) -> &'static str {
        match self {
            Self::EnglishArabic => "eng-ara",
            Self::ArabicEnglish => "ara-eng",
        }
    }
    pub fn language(self) -> Language {
        match self {
            Self::EnglishArabic => Language::English,
            Self::ArabicEnglish => Language::Arabic,
        }
    }
    pub fn source(self) -> Attribution {
        Attribution {
            name: format!("FreeDict {} 0.6.3", self.id()),
            url: "https://freedict.org/downloads/".into(),
            license: "GPL-2.0-or-later".into(),
        }
    }
    pub fn url(self) -> String {
        format!(
            "https://download.freedict.org/dictionaries/{0}/0.6.3/freedict-{0}-0.6.3.stardict.tar.xz",
            self.id()
        )
    }
    fn checksum(self) -> &'static str {
        match self {
            Self::EnglishArabic => {
                "846e634c13e522a4273ddc7a595467d968274eb667394f1a19fa134155f2f05f98f512ada17a82c36109ca5b84565e9917af49d8eac877b62c84feb9c852f7cc"
            }
            Self::ArabicEnglish => {
                "8566344067a25ad7c679edf7c858f86063ba780a81daf0dab2f2cddbbebf89d3154d4eed8be4ff0908f6059ce8776bebd411de6d7c24b31669ef082cdb36e9f4"
            }
        }
    }
}

/// Streams and checks an official FreeDict release, then installs its files atomically.
/// No text query is sent; the return value is the installed `.ifo` path for import.
pub async fn download_open_pack(pack: OpenPack, cache_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(cache_dir).context("Creating dictionary pack cache")?;
    let destination = cache_dir.join(format!("freedict-{}-0.6.3", pack.id()));
    let installed = destination
        .join(pack.id())
        .join(format!("{}.ifo", pack.id()));
    if installed.is_file() {
        return Ok(installed);
    }
    let unique = unique_id();
    let download_path = cache_dir.join(format!(".writy-download-{unique}"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&download_path)?;
    let download_guard = Cleanup(download_path.clone());
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut response = client.get(pack.url()).send().await?.error_for_status()?;
    const MAX_DOWNLOAD: u64 = 8 * 1024 * 1024;
    ensure!(
        response.content_length().unwrap_or(0) <= MAX_DOWNLOAD,
        "Dictionary pack download exceeds size limit"
    );
    let mut digest = Sha512::new();
    let mut total = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        total = total
            .checked_add(chunk.len() as u64)
            .context("Dictionary download size overflow")?;
        ensure!(
            total <= MAX_DOWNLOAD,
            "Dictionary pack download exceeds size limit"
        );
        digest.update(&chunk);
        file.write_all(&chunk)?;
    }
    ensure!(
        format!("{:x}", digest.finalize()) == pack.checksum(),
        "Dictionary pack SHA512 checksum mismatch"
    );
    file.sync_all()?;
    drop(file);
    let stage = cache_dir.join(format!(".writy-pack-{unique}"));
    let extraction_path = download_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        extract_pack(&extraction_path, &stage, &destination, pack)
    })
    .await
    .context("Dictionary extraction worker failed")??;
    drop(download_guard);
    Ok(result)
}

fn extract_pack(
    archive: &Path,
    stage: &Path,
    destination: &Path,
    pack: OpenPack,
) -> Result<PathBuf> {
    fs::create_dir(stage)?;
    let guard = Cleanup(stage.to_owned());
    let decoder = liblzma::read::XzDecoder::new(File::open(archive)?);
    let mut archive = tar::Archive::new(decoder.take(80 * 1024 * 1024));
    let mut total = 0_u64;
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < 1024, "Dictionary archive has too many members");
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        ensure!(
            safe_archive_path(&path),
            "Dictionary archive contains an unsafe path"
        );
        let output = stage.join(path);
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        ensure!(
            kind.is_file(),
            "Dictionary archive contains links or unsupported entries"
        );
        let size = entry.size();
        total = total
            .checked_add(size)
            .context("Dictionary archive size overflow")?;
        ensure!(
            size <= 32 * 1024 * 1024 && total <= 64 * 1024 * 1024,
            "Dictionary archive exceeds extraction size limit"
        );
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut file)?;
        ensure!(copied == size, "Truncated dictionary archive member");
    }
    let relative = PathBuf::from(pack.id()).join(format!("{}.ifo", pack.id()));
    ensure!(
        stage.join(&relative).is_file(),
        "Dictionary archive is missing its expected header"
    );
    // Never overwrite an existing directory: another process may have completed the same pack.
    if destination.exists() {
        ensure!(
            destination.join(&relative).is_file(),
            "Dictionary pack destination exists but is incomplete"
        );
    } else {
        fs::rename(stage, destination).context("Installing dictionary pack")?;
    }
    drop(guard);
    Ok(destination.join(relative))
}

fn safe_archive_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.as_os_str().len() <= 1024
        && path.components().count() <= 16
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}
fn unique_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.0.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive_file(dir: &Path, linked: bool) -> PathBuf {
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o600);
        if linked {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_link_name("../../outside").unwrap();
            header.set_cksum();
            tar.append_data(&mut header, "eng-ara/link", std::io::empty())
                .unwrap();
        } else {
            header.set_size(7);
            header.set_cksum();
            tar.append_data(&mut header, "eng-ara/eng-ara.ifo", &b"fixture"[..])
                .unwrap();
        }
        let bytes = tar.into_inner().unwrap();
        let mut xz = liblzma::write::XzEncoder::new(Vec::new(), 1);
        xz.write_all(&bytes).unwrap();
        let path = dir.join("archive.tar.xz");
        fs::write(&path, xz.finish().unwrap()).unwrap();
        path
    }
    #[test]
    fn archive_extraction_is_atomic_and_keeps_member_contents() {
        let dir = tempfile::TempDir::new().unwrap();
        let archive = archive_file(dir.path(), false);
        let stage = dir.path().join("stage");
        let dest = dir.path().join("installed");
        let ifo = extract_pack(&archive, &stage, &dest, OpenPack::EnglishArabic).unwrap();
        assert_eq!(fs::read(ifo).unwrap(), b"fixture");
        assert!(!stage.exists());
    }
    #[test]
    fn archive_links_are_rejected_and_staging_is_cleaned() {
        let dir = tempfile::TempDir::new().unwrap();
        let archive = archive_file(dir.path(), true);
        let stage = dir.path().join("stage");
        let dest = dir.path().join("installed");
        assert!(extract_pack(&archive, &stage, &dest, OpenPack::EnglishArabic).is_err());
        assert!(!stage.exists());
        assert!(!dest.exists());
    }
    #[test]
    fn archive_paths_reject_escape_and_absolute_paths() {
        assert!(!safe_archive_path(Path::new("../outside")));
        assert!(!safe_archive_path(Path::new("/etc/passwd")));
        assert!(safe_archive_path(Path::new("eng-ara/eng-ara.ifo")));
    }
}
