use super::{Dictionary, DictionarySource};
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
    pub fn cache_path(self, cache: &Path) -> PathBuf {
        cache.join(format!("freedict-{}-0.6.3", self.id()))
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
    let destination = pack.cache_path(cache_dir);
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

pub struct CatalogueEntry {
    pub name: &'static str,
    pub url: &'static str,
    pub detail: &'static str,
    pub pack: &'static str,
    language: Language,
    license: &'static str,
}
impl CatalogueEntry {
    pub fn matches(&self, source: &DictionarySource) -> bool {
        source.attribution.url == self.url
            && match self.open_pack() {
                Some(pack) => source
                    .attribution
                    .name
                    .starts_with(&format!("FreeDict {} ", pack.id())),
                None => source.attribution.name == self.name,
            }
    }
    pub fn open_pack(&self) -> Option<OpenPack> {
        match self.pack {
            "en-ar" => Some(OpenPack::EnglishArabic),
            "ar-en" => Some(OpenPack::ArabicEnglish),
            _ => None,
        }
    }
    fn cache_path(&self, cache: &std::path::Path) -> std::path::PathBuf {
        match self.open_pack() {
            Some(pack) => pack.cache_path(cache),
            None => cache.join(self.pack),
        }
    }
}

pub const CATALOGUE: &[CatalogueEntry] = &[
    CatalogueEntry {
        name: "FreeDict English → Arabic",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: "en-ar",
        language: Language::English,
        license: "GPL-2.0-or-later",
    },
    CatalogueEntry {
        name: "FreeDict Arabic → English",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: "ar-en",
        language: Language::Arabic,
        license: "GPL-2.0-or-later",
    },
    CatalogueEntry {
        name: "Wiktionary English (Kaikki)",
        url: "https://kaikki.org/dictionary/English/",
        detail: "Definitions, examples and word forms · Large download (~510 MB) · CC-BY-SA-4.0 / GFDL",
        pack: "wiktionary-en",
        language: Language::English,
        license: "CC-BY-SA-4.0 / GFDL",
    },
    CatalogueEntry {
        name: "Wiktionary Arabic (Kaikki)",
        url: "https://kaikki.org/dictionary/Arabic/",
        detail: "Arabic headwords with English glosses · ~69 MB download · CC-BY-SA-4.0 / GFDL",
        pack: "wiktionary-ar",
        language: Language::Arabic,
        license: "CC-BY-SA-4.0 / GFDL",
    },
    CatalogueEntry {
        name: "Open English WordNet",
        url: "https://github.com/globalwordnet/english-wordnet",
        detail: "English senses and synonyms · Automatic conversion · CC-BY-4.0",
        pack: "wordnet",
        language: Language::English,
        license: "CC-BY-4.0",
    },
    CatalogueEntry {
        name: "Arramooz Alwaseet",
        url: "https://sourceforge.net/projects/arramooz/files/",
        detail: "Arabic roots and morphology · ~2.2 MB download · GPL-2.0",
        pack: "arramooz",
        language: Language::Arabic,
        license: "GPL-2.0",
    },
    CatalogueEntry {
        name: "GNU GCIDE",
        url: "https://gcide.gnu.org.ua/download",
        detail: "English definitions · Includes historical usage · GPL-3.0-or-later",
        pack: "gcide",
        language: Language::English,
        license: "GPL-3.0-or-later",
    },
    CatalogueEntry {
        name: "Princeton WordNet",
        url: "https://wordnet.princeton.edu/",
        detail: "English senses, synonyms and examples · Version 3.0 · Overlaps Open English WordNet · WordNet licence",
        pack: "princeton",
        language: Language::English,
        license: "WordNet-3.0",
    },
];

// Downloads can overlap; serialize imports and removals within this process.
static DICTIONARY_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub struct DictionaryLibrary {
    database: PathBuf,
    cache: PathBuf,
}
impl DictionaryLibrary {
    pub fn new(database: impl Into<PathBuf>) -> Self {
        let database = database.into();
        let cache = database
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("packs");
        Self { database, cache }
    }
    pub fn open(&self) -> Result<Dictionary> {
        let mut dictionary = Dictionary::open(&self.database)?;
        dictionary.reset_library_once(&self.cache)?;
        Ok(dictionary)
    }
    pub async fn install(&self, entry: &CatalogueEntry, importing: impl FnOnce()) -> Result<usize> {
        self.open()?;
        let path = match entry.open_pack() {
            Some(pack) => download_open_pack(pack, &self.cache).await?,
            None => prepare_download(entry.pack, &self.cache)?,
        };
        importing();
        self.import_path(&path, entry)
    }
    pub fn remove(&self, entry: &CatalogueEntry) -> Result<()> {
        let _write = DICTIONARY_WRITE
            .lock()
            .map_err(|_| anyhow::anyhow!("Dictionary worker stopped"))?;
        let mut dictionary = self.open()?;
        let ids = dictionary
            .source_states()?
            .into_iter()
            .filter(|source| entry.matches(source))
            .map(|source| source.id)
            .collect::<Vec<_>>();
        let cache = entry.cache_path(&self.cache);
        match std::fs::remove_dir_all(&cache) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        dictionary.remove_sources(&ids)
    }
    fn import_path(&self, path: &Path, entry: &CatalogueEntry) -> anyhow::Result<usize> {
        let _import = DICTIONARY_WRITE
            .lock()
            .map_err(|_| anyhow::anyhow!("Dictionary import worker stopped"))?;
        let mut dictionary = self.open()?;
        let source = Attribution {
            name: entry.name.into(),
            license: entry.license.into(),
            url: entry.url.into(),
        };
        match entry.open_pack() {
            Some(pack) => dictionary.import_bilingual_stardict(path, entry.language, pack.source()),
            None => match path.extension().and_then(|extension| extension.to_str()) {
                Some("ifo") => dictionary.import_stardict(path, entry.language, source),
                Some("jsonl") => dictionary.import_kaikki(path, entry.language, source),
                _ => anyhow::bail!(
                    "Choose a StarDict .ifo or Kaikki .jsonl file after extracting the archive."
                ),
            },
        }
    }
}

fn prepare_download(pack: &str, cache: &Path) -> Result<PathBuf> {
    fs::create_dir_all(cache)?;
    let scripts = cache.join(format!(".installer-{}", unique_id()));
    fs::create_dir(&scripts)?;
    let _cleanup = Cleanup(scripts.clone());
    for (name, content) in [
        (
            "download_dictionary.py",
            include_str!("../../../../tools/download_dictionary.py"),
        ),
        (
            "clean_dictionary_jsonl.py",
            include_str!("../../../../tools/clean_dictionary_jsonl.py"),
        ),
        (
            "wordnet_to_jsonl.py",
            include_str!("../../../../tools/wordnet_to_jsonl.py"),
        ),
    ] {
        fs::write(scripts.join(name), content)?;
    }
    let output = std::process::Command::new("python3")
        .arg("-B")
        .arg(scripts.join("download_dictionary.py"))
        .arg(pack)
        .arg(cache)
        .output()
        .context("Cannot start dictionary downloader; Python 3 is required")?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let path = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    anyhow::ensure!(
        path.starts_with(cache) && path.is_file(),
        "Downloader returned an invalid dictionary path"
    );
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

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
    fn cached_library() -> (
        tempfile::TempDir,
        DictionaryLibrary,
        &'static CatalogueEntry,
        PathBuf,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let library = DictionaryLibrary::new(directory.path().join("dictionary.sqlite3"));
        library.open().unwrap();
        let entry = CATALOGUE
            .iter()
            .find(|entry| entry.pack == "wordnet")
            .unwrap();
        let cache = entry.cache_path(&library.cache);
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("ready.json"), "{}").unwrap();
        fs::write(
            cache.join("dictionary.jsonl"),
            r#"{"word":"book","lang_code":"en","senses":[{"glosses":["a volume"]}]}"#,
        )
        .unwrap();
        (directory, library, entry, cache)
    }

    #[tokio::test]
    async fn cached_install_reimport_and_removal_preserve_library_rules() {
        let (_directory, library, entry, cache) = cached_library();
        let importing = Cell::new(false);
        assert_eq!(
            library
                .install(entry, || importing.set(true))
                .await
                .unwrap(),
            1
        );
        assert!(importing.get());
        let dictionary = library.open().unwrap();
        let source = dictionary.source_states().unwrap().remove(0);
        assert!(entry.matches(&source));
        dictionary.set_source_enabled(&source.id, false).unwrap();
        library.install(entry, || {}).await.unwrap();
        assert!(!library.open().unwrap().source_states().unwrap()[0].enabled);

        // A cache cleanup error must leave lexical entries and preferences intact.
        fs::remove_dir_all(&cache).unwrap();
        fs::write(&cache, "not a directory").unwrap();
        assert!(library.remove(entry).is_err());
        assert!(!library.open().unwrap().source_states().unwrap()[0].enabled);
        fs::remove_file(&cache).unwrap();
        library.remove(entry).unwrap();
        assert!(library.open().unwrap().source_states().unwrap().is_empty());
        assert!(!cache.exists());
        library.remove(entry).unwrap();
    }

    #[tokio::test]
    async fn failed_cached_import_retains_entries_and_can_retry() {
        let (_directory, library, entry, cache) = cached_library();
        library.install(entry, || {}).await.unwrap();
        let path = cache.join("dictionary.jsonl");
        let original = fs::read(&path).unwrap();
        fs::write(&path, "invalid json").unwrap();
        assert!(library.install(entry, || {}).await.is_err());
        assert_eq!(library.open().unwrap().lookup("book").unwrap().len(), 1);
        fs::write(&path, original).unwrap();
        assert_eq!(library.install(entry, || {}).await.unwrap(), 1);
        assert!(fs::read_dir(&library.cache).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".installer-")
        }));
    }

    #[tokio::test]
    async fn cached_freedict_install_uses_bilingual_attribution_in_both_directions() {
        let directory = tempfile::tempdir().unwrap();
        let library = DictionaryLibrary::new(directory.path().join("dictionary.sqlite3"));
        library.open().unwrap();
        for entry in CATALOGUE.iter().filter(|entry| entry.open_pack().is_some()) {
            let pack = entry.open_pack().unwrap();
            let cache = entry.cache_path(&library.cache).join(pack.id());
            fs::create_dir_all(&cache).unwrap();
            let mut index = b"book\0".to_vec();
            index.extend_from_slice(&0_u32.to_be_bytes());
            index.extend_from_slice(&4_u32.to_be_bytes());
            fs::write(cache.join(format!("{}.idx", pack.id())), &index).unwrap();
            fs::write(cache.join(format!("{}.dict", pack.id())), "text").unwrap();
            fs::write(cache.join(format!("{}.ifo", pack.id())), format!("StarDict's dict ifo file\nversion=2.4.2\nbookname=Fixture\nwordcount=1\nidxfilesize={}\nsametypesequence=m\n", index.len())).unwrap();
            assert_eq!(library.install(entry, || {}).await.unwrap(), 1);
            let dictionary = library.open().unwrap();
            assert!(
                dictionary
                    .source_states()
                    .unwrap()
                    .iter()
                    .any(|source| entry.matches(source))
            );
            let found = dictionary.lookup("book").unwrap();
            let found = found
                .iter()
                .find(|found| found.source.name == pack.source().name)
                .unwrap();
            assert_eq!(found.language, pack.language());
            assert_eq!(found.senses[0].translations, ["text"]);
            assert!(found.senses[0].definitions.is_empty());
        }
    }
}
