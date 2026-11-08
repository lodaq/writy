//! Dictionary installation and removal shared by desktop and command-line callers.
use super::{Dictionary, DictionarySource, OpenPack, download_open_pack};
use crate::types::{Attribution, Language};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct CatalogueEntry {
    pub name: &'static str,
    prefix: &'static str,
    pub url: &'static str,
    pub detail: &'static str,
    pub pack: &'static str,
    language: Language,
    license: &'static str,
}
impl CatalogueEntry {
    pub fn matches(&self, source: &DictionarySource) -> bool {
        source.attribution.url == self.url
            && match self.pack {
                "en-ar" | "ar-en" => source
                    .attribution
                    .name
                    .starts_with(&format!("{} ", self.prefix)),
                _ => source.attribution.name == self.name,
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
        prefix: "FreeDict eng-ara",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: "en-ar",
        language: Language::English,
        license: "GPL-2.0-or-later",
    },
    CatalogueEntry {
        name: "FreeDict Arabic → English",
        prefix: "FreeDict ara-eng",
        url: "https://freedict.org/downloads/",
        detail: "Bilingual translations · GPL-2.0-or-later",
        pack: "ar-en",
        language: Language::Arabic,
        license: "GPL-2.0-or-later",
    },
    CatalogueEntry {
        name: "Wiktionary English (Kaikki)",
        prefix: "Wiktionary English",
        url: "https://kaikki.org/dictionary/English/",
        detail: "Definitions, examples and word forms · Large download (~510 MB) · CC-BY-SA-4.0 / GFDL",
        pack: "wiktionary-en",
        language: Language::English,
        license: "CC-BY-SA-4.0 / GFDL",
    },
    CatalogueEntry {
        name: "Wiktionary Arabic (Kaikki)",
        prefix: "Wiktionary Arabic",
        url: "https://kaikki.org/dictionary/Arabic/",
        detail: "Arabic headwords with English glosses · ~69 MB download · CC-BY-SA-4.0 / GFDL",
        pack: "wiktionary-ar",
        language: Language::Arabic,
        license: "CC-BY-SA-4.0 / GFDL",
    },
    CatalogueEntry {
        name: "Open English WordNet",
        prefix: "Open English WordNet",
        url: "https://github.com/globalwordnet/english-wordnet",
        detail: "English senses and synonyms · Automatic conversion · CC-BY-4.0",
        pack: "wordnet",
        language: Language::English,
        license: "CC-BY-4.0",
    },
    CatalogueEntry {
        name: "Arramooz Alwaseet",
        prefix: "Arramooz",
        url: "https://sourceforge.net/projects/arramooz/files/",
        detail: "Arabic roots and morphology · ~2.2 MB download · GPL-2.0",
        pack: "arramooz",
        language: Language::Arabic,
        license: "GPL-2.0",
    },
    CatalogueEntry {
        name: "GNU GCIDE",
        prefix: "GNU GCIDE",
        url: "https://gcide.gnu.org.ua/download",
        detail: "English definitions · Includes historical usage · GPL-3.0-or-later",
        pack: "gcide",
        language: Language::English,
        license: "GPL-3.0-or-later",
    },
    CatalogueEntry {
        name: "Princeton WordNet",
        prefix: "Princeton WordNet",
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

fn prepare_download(pack: &str, cache: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    use anyhow::Context;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    fs::create_dir_all(cache)?;
    let scripts = cache.join(format!(
        ".installer-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&scripts)?;
    let result = (|| {
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
        let path = std::path::PathBuf::from(String::from_utf8(output.stdout)?.trim());
        anyhow::ensure!(
            path.starts_with(cache) && path.is_file(),
            "Downloader returned an invalid dictionary path"
        );
        Ok(path)
    })();
    let _ = fs::remove_dir_all(scripts);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, fs};

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
