use super::{
    MAX_ENTRIES, MAX_ENTRY_BYTES, insert_alias, insert_entry, markup, replace_source, source_key,
};
use crate::types::{Attribution, Language, LexicalEntry, Sense};
use anyhow::{Context, Result, bail, ensure};
use flate2::read::MultiGzDecoder;
use rusqlite::Connection;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_INDEX: usize = 128 * 1024 * 1024;
const MAX_DATA: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum EntryKind {
    Definition,
    Translation,
}

pub(super) fn import(
    connection: &mut Connection,
    ifo: &Path,
    language: Language,
    source: Attribution,
    kind: EntryKind,
) -> Result<usize> {
    let header = String::from_utf8(read_file(ifo, 128 * 1024, false)?)
        .context("StarDict header is not UTF-8")?;
    let mut lines = header.lines();
    ensure!(
        lines.next().map(str::trim) == Some("StarDict's dict ifo file"),
        "Invalid StarDict header"
    );
    let options: HashMap<_, _> = lines.filter_map(|line| line.split_once('=')).collect();
    ensure!(
        matches!(options.get("version"), Some(&"2.4.2") | Some(&"3.0.0")),
        "Unsupported StarDict version"
    );
    let expected = number(&options, "wordcount")?;
    ensure!(
        expected > 0 && expected <= MAX_ENTRIES,
        "Invalid StarDict word count"
    );
    let expected_index = number(&options, "idxfilesize")?;
    ensure!(expected_index <= MAX_INDEX, "StarDict index exceeds limit");
    let offset_bytes = match options.get("idxoffsetbits").copied().unwrap_or("32") {
        "32" => 4,
        "64" => 8,
        _ => bail!("Invalid StarDict offset width"),
    };
    let sequence = options
        .get("sametypesequence")
        .copied()
        .unwrap_or("")
        .as_bytes();
    ensure!(
        sequence.len() <= 64 && sequence.iter().all(u8::is_ascii_alphabetic),
        "Invalid StarDict type sequence"
    );
    let root = ifo.parent().unwrap_or_else(|| Path::new("."));
    let base = ifo.with_extension("");
    let (idx_path, idx_gzip) = choose(&base, "idx", "idx.gz")?;
    let index = read_file(&idx_path, MAX_INDEX, idx_gzip)?;
    ensure!(
        index.len() == expected_index,
        "StarDict index size does not match header"
    );
    let (data_path, data_gzip) = choose(&base, "dict", "dict.dz")?;
    let data = read_file(&data_path, MAX_DATA, data_gzip)?;
    let transaction = connection.transaction()?;
    let key = source_key(&source, language);
    replace_source(&transaction, &key)?;
    let mut position = 0;
    let mut ids = Vec::with_capacity(expected);
    while position < index.len() {
        ensure!(ids.len() < expected, "StarDict index has excess entries");
        let word = read_string(&index, &mut position)?;
        let offset = if offset_bytes == 8 {
            read_u64(&index, &mut position)?
        } else {
            read_u32(&index, &mut position)? as u64
        };
        let size = read_u32(&index, &mut position)? as usize;
        ensure!(size <= MAX_ENTRY_BYTES, "StarDict entry exceeds limit");
        let start = usize::try_from(offset).context("StarDict offset overflows platform")?;
        let end = start
            .checked_add(size)
            .context("StarDict entry offset overflows")?;
        let bytes = data
            .get(start..end)
            .context("StarDict entry extends past dictionary data")?;
        let mut entry = parse_entry(&word, bytes, sequence, root, language, source.clone())?;
        if matches!(kind, EntryKind::Translation) {
            for sense in &mut entry.senses {
                sense.translations.append(&mut sense.definitions);
            }
            entry.definition_markup = None;
        }
        ids.push(insert_entry(&transaction, &key, &entry, &[])?);
    }
    ensure!(
        ids.len() == expected,
        "StarDict word count does not match index"
    );
    let expected_syn = options
        .get("synwordcount")
        .map(|s| s.parse::<usize>())
        .transpose()
        .context("Invalid synonym count")?
        .unwrap_or(0);
    ensure!(
        expected_syn <= MAX_ENTRIES,
        "StarDict synonym count exceeds limit"
    );
    let syn_path = appended(&base, "syn");
    let mut synonyms = 0;
    if syn_path.exists() {
        let bytes = read_file(&syn_path, MAX_INDEX, false)?;
        let mut pos = 0;
        while pos < bytes.len() {
            ensure!(
                synonyms < MAX_ENTRIES,
                "StarDict synonym count exceeds limit"
            );
            let alias = read_string(&bytes, &mut pos)?;
            let target = read_u32(&bytes, &mut pos)? as usize;
            let id = ids
                .get(target)
                .context("StarDict synonym references an unknown entry")?;
            insert_alias(&transaction, &alias, *id)?;
            synonyms += 1;
        }
    }
    ensure!(
        synonyms == expected_syn,
        "StarDict synonym count does not match header"
    );
    transaction.commit()?;
    Ok(ids.len())
}

fn parse_entry(
    word: &str,
    data: &[u8],
    sequence: &[u8],
    root: &Path,
    language: Language,
    source: Attribution,
) -> Result<LexicalEntry> {
    let mut entry = LexicalEntry {
        headword: word.into(),
        language,
        source,
        ..Default::default()
    };
    let mut position = 0;
    let mut seq_pos = 0;
    while position < data.len() {
        let (kind, last) = if sequence.is_empty() {
            let kind = data[position];
            position += 1;
            (kind, false)
        } else {
            let kind = *sequence
                .get(seq_pos)
                .context("StarDict has trailing entry data")?;
            seq_pos += 1;
            (kind, seq_pos == sequence.len())
        };
        ensure!(kind.is_ascii_alphabetic(), "Invalid StarDict field type");
        let field = if last {
            let field = &data[position..];
            position = data.len();
            field
        } else if kind.is_ascii_uppercase() {
            let length = read_u32(data, &mut position)? as usize;
            let end = position
                .checked_add(length)
                .context("StarDict field length overflow")?;
            let field = data
                .get(position..end)
                .context("Truncated StarDict binary field")?;
            position = end;
            field
        } else {
            let start = position;
            let length = data[start..]
                .iter()
                .position(|b| *b == 0)
                .context("Unterminated StarDict text field")?;
            position += length + 1;
            &data[start..start + length]
        };
        if kind.is_ascii_uppercase() {
            continue;
        } // Embedded binary types are not written to disk.
        let text = std::str::from_utf8(field).context("StarDict text field is not UTF-8")?;
        if kind == b't' || kind == b'y' {
            entry.pronunciation = text.into();
            continue;
        }
        if kind == b'r' {
            for line in text.lines() {
                if let Some((category, path)) = line.split_once('\t') {
                    let audio = matches!(category, "snd" | "sound");
                    if matches!(category, "snd" | "sound" | "img" | "image")
                        && let Some(path) = markup::resource(root, path, audio)
                    {
                        if audio {
                            entry.audio.push(path);
                        } else {
                            entry.images.push(path);
                        }
                    }
                }
            }
            continue;
        }
        let content = if matches!(kind, b'h' | b'g' | b'x' | b'k') {
            markup::extract(text, root)
        } else {
            markup::Content {
                text: text.into(),
                ..Default::default()
            }
        };
        entry.audio.extend(content.audio);
        entry.images.extend(content.images);
        if !content.text.trim().is_empty() {
            let formatted = content
                .markup
                .unwrap_or_else(|| quick_xml::escape::escape(&content.text).into_owned());
            let markup = entry.definition_markup.get_or_insert_with(String::new);
            if !markup.is_empty() {
                markup.push('\n');
            }
            markup.push_str(&formatted);
            entry.senses.push(Sense {
                definitions: vec![content.text],
                ..Default::default()
            });
        }
    }
    ensure!(
        sequence.is_empty() || seq_pos == sequence.len(),
        "Missing StarDict fields"
    );
    Ok(entry)
}

fn number(options: &HashMap<&str, &str>, key: &str) -> Result<usize> {
    options
        .get(key)
        .with_context(|| format!("Missing StarDict {key}"))?
        .parse()
        .with_context(|| format!("Invalid StarDict {key}"))
}
fn choose(base: &Path, plain: &str, compressed: &str) -> Result<(PathBuf, bool)> {
    let plain = appended(base, plain);
    if plain.is_file() {
        return Ok((plain, false));
    }
    let compressed = appended(base, compressed);
    ensure!(compressed.is_file(), "Missing StarDict data/index file");
    Ok((compressed, true))
}
fn appended(base: &Path, suffix: &str) -> PathBuf {
    let mut name = base.as_os_str().to_os_string();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}
fn read_file(path: &Path, limit: usize, gzip: bool) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("Reading {}", path.display()))?;
    let mut reader: Box<dyn Read> = if gzip {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "StarDict file exceeds decompressed size limit"
    );
    Ok(bytes)
}
fn read_string(bytes: &[u8], position: &mut usize) -> Result<String> {
    let start = *position;
    let tail = bytes.get(start..).context("Truncated StarDict string")?;
    let length = tail
        .iter()
        .take(4097)
        .position(|b| *b == 0)
        .context("Unterminated or oversized StarDict headword")?;
    ensure!(length > 0, "Empty StarDict headword");
    *position = start + length + 1;
    Ok(std::str::from_utf8(&tail[..length])
        .context("StarDict headword is not UTF-8")?
        .into())
}
fn read_u32(bytes: &[u8], position: &mut usize) -> Result<u32> {
    let end = position
        .checked_add(4)
        .context("StarDict offset overflow")?;
    let data = bytes
        .get(*position..end)
        .context("Truncated StarDict integer")?;
    *position = end;
    Ok(u32::from_be_bytes(data.try_into()?))
}
fn read_u64(bytes: &[u8], position: &mut usize) -> Result<u64> {
    let end = position
        .checked_add(8)
        .context("StarDict offset overflow")?;
    let data = bytes
        .get(*position..end)
        .context("Truncated StarDict integer")?;
    *position = end;
    Ok(u64::from_be_bytes(data.try_into()?))
}
