//! Offline lexical storage. Dictionary prose is stored as data, never executable markup.
use crate::types::{Attribution, Language, LexicalEntry};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, Transaction, params};
use std::{fs, path::Path, time::Duration};
use unicode_normalization::UnicodeNormalization;

mod kaikki;
mod markup;
mod packs;
mod stardict;
pub use packs::{OpenPack, download_open_pack};

pub(crate) const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_ENTRIES: usize = 2_000_000;

pub struct Dictionary {
    connection: Connection,
}

impl Dictionary {
    pub fn sources(&self) -> Result<Vec<(Attribution, usize)>> {
        let mut statement = self.connection.prepare(
            "SELECT payload, COUNT(*) FROM lexical_entries GROUP BY source_key ORDER BY source_key",
        )?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?)))?;
        rows.map(|r| {
            let (payload, count) = r?;
            let entry: LexicalEntry = serde_json::from_str(&payload)?;
            Ok((entry.source, count))
        })
        .collect()
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path).context("Opening lexical database")?;
        connection.busy_timeout(Duration::from_millis(250))?;
        connection.execute_batch("PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS lexical_entries (
                id INTEGER PRIMARY KEY, source_key TEXT NOT NULL, headword TEXT NOT NULL, payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS lexical_sources ON lexical_entries(source_key);
            CREATE TABLE IF NOT EXISTS lexical_aliases (
                alias TEXT NOT NULL, entry_id INTEGER NOT NULL REFERENCES lexical_entries(id) ON DELETE CASCADE,
                PRIMARY KEY(alias,entry_id)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS lexical_alias_entries ON lexical_aliases(entry_id);")?;
        Ok(Self { connection })
    }

    pub fn lookup(&self, query: &str) -> Result<Vec<LexicalEntry>> {
        let key = normalize(query);
        if key.is_empty() || key.len() > 4096 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare("SELECT e.payload FROM lexical_aliases a JOIN lexical_entries e ON e.id=a.entry_id WHERE a.alias=?1 ORDER BY ROW_NUMBER() OVER (PARTITION BY e.source_key ORDER BY e.id), e.id LIMIT 50")?;
        let rows = statement.query_map([key], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    pub fn complete(&self, prefix: &str, limit: usize) -> Result<Vec<String>> {
        let prefix = normalize(prefix);
        if prefix.is_empty() || prefix.len() > 4096 || limit == 0 {
            return Ok(Vec::new());
        }
        let end = format!("{prefix}\u{10ffff}");
        let mut statement = self.connection.prepare("SELECT DISTINCT e.headword FROM lexical_aliases a JOIN lexical_entries e ON e.id=a.entry_id WHERE a.alias>=?1 AND a.alias<?2 ORDER BY e.headword LIMIT ?3")?;
        let rows = statement.query_map(params![prefix, end, limit.min(100)], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Replaces this source atomically; a malformed input leaves the previous import intact.
    pub fn import_stardict(
        &mut self,
        ifo: &Path,
        language: Language,
        source: Attribution,
    ) -> Result<usize> {
        stardict::import(
            &mut self.connection,
            ifo,
            language,
            source,
            stardict::EntryKind::Definition,
        )
    }

    /// Explicit bilingual imports expose payloads as translations, without guessing from script.
    pub fn import_bilingual_stardict(
        &mut self,
        ifo: &Path,
        language: Language,
        source: Attribution,
    ) -> Result<usize> {
        stardict::import(
            &mut self.connection,
            ifo,
            language,
            source,
            stardict::EntryKind::Translation,
        )
    }

    /// Streams one JSON object per line, retaining individual senses and source provenance.
    pub fn import_kaikki(
        &mut self,
        path: &Path,
        language: Language,
        source: Attribution,
    ) -> Result<usize> {
        kaikki::import(&mut self.connection, path, language, source)
    }
}

pub(crate) fn normalize(text: &str) -> String {
    text.trim()
        .nfkc()
        .flat_map(char::to_lowercase)
        .filter_map(|c| match c {
            '\u{0640}'
            | '\u{0610}'..='\u{061a}'
            | '\u{064b}'..='\u{065f}'
            | '\u{0670}'
            | '\u{06d6}'..='\u{06ed}' => None,
            'أ' | 'إ' | 'آ' | 'ٱ' => Some('ا'),
            _ => Some(c),
        })
        .collect()
}

pub(crate) fn source_key(source: &Attribution, language: Language) -> String {
    format!("{}\n{}\n{}", source.name, source.url, language.code())
}

pub(crate) fn replace_source(transaction: &Transaction<'_>, key: &str) -> Result<()> {
    transaction.execute("DELETE FROM lexical_entries WHERE source_key=?1", [key])?;
    Ok(())
}

pub(crate) fn insert_entry(
    transaction: &Transaction<'_>,
    key: &str,
    entry: &LexicalEntry,
    aliases: &[String],
) -> Result<i64> {
    ensure!(
        !entry.headword.trim().is_empty() && entry.headword.len() <= 4096,
        "Invalid dictionary headword"
    );
    let payload = serde_json::to_string(entry)?;
    ensure!(
        payload.len() <= MAX_ENTRY_BYTES * 4,
        "Dictionary entry exceeds storage limit"
    );
    transaction.execute(
        "INSERT INTO lexical_entries(source_key,headword,payload) VALUES (?1,?2,?3)",
        params![key, entry.headword, payload],
    )?;
    let id = transaction.last_insert_rowid();
    insert_alias(transaction, &entry.headword, id)?;
    for alias in aliases {
        insert_alias(transaction, alias, id)?;
    }
    Ok(id)
}

pub(crate) fn insert_alias(transaction: &Transaction<'_>, alias: &str, id: i64) -> Result<()> {
    ensure!(alias.len() <= 4096, "Dictionary alias exceeds limit");
    let alias = normalize(alias);
    if !alias.is_empty() {
        transaction.execute(
            "INSERT OR IGNORE INTO lexical_aliases(alias,entry_id) VALUES (?1,?2)",
            params![alias, id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
