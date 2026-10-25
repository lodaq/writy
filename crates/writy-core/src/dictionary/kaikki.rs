use super::{MAX_ENTRIES, MAX_ENTRY_BYTES, insert_entry, markup, replace_source, source_key};
use crate::types::{Attribution, Language, LexicalEntry, Sense};
use anyhow::{Context, Result, ensure};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

pub(super) fn import(
    connection: &mut Connection,
    path: &Path,
    language: Language,
    source: Attribution,
) -> Result<usize> {
    let file = File::open(path).context("Opening Kaikki JSONL input")?;
    let mut reader = BufReader::new(file);
    let transaction = connection.transaction()?;
    let key = source_key(&source, language);
    replace_source(&transaction, &key)?;
    let mut count = 0;
    let mut line = Vec::new();
    let mut line_number = 0;
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_ENTRY_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        line_number += 1;
        ensure!(
            line.len() <= MAX_ENTRY_BYTES,
            "Kaikki line {line_number} exceeds entry size limit"
        );
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value: Value = serde_json::from_slice(&line)
            .with_context(|| format!("Invalid Kaikki JSON at line {line_number}"))?;
        ensure!(
            value.is_object(),
            "Kaikki line {line_number} must be an object"
        );
        let code = string(&value, "lang_code");
        if !code.is_empty() && code != language.code() {
            continue;
        }
        let word = string(&value, "word");
        ensure!(
            !word.is_empty(),
            "Kaikki entry at line {line_number} has no word"
        );
        if code.is_empty() && Language::detect(&word) != language {
            continue;
        }
        ensure!(count < MAX_ENTRIES, "Kaikki import exceeds entry limit");
        let aliases = words(&value, "forms", "form");
        let entry = parse(
            &value,
            word,
            language,
            source.clone(),
            path.parent().unwrap_or_else(|| Path::new(".")),
        );
        insert_entry(&transaction, &key, &entry, &aliases)?;
        count += 1;
    }
    ensure!(
        count > 0,
        "Kaikki input contains no entries for the requested language"
    );
    transaction.commit()?;
    Ok(count)
}

fn parse(
    value: &Value,
    word: String,
    language: Language,
    source: Attribution,
    root: &Path,
) -> LexicalEntry {
    let mut entry = LexicalEntry {
        headword: word,
        language,
        source,
        ..Default::default()
    };
    for sound in array(value, "sounds") {
        let ipa = string(sound, "ipa");
        if entry.pronunciation.is_empty() && !ipa.is_empty() {
            entry.pronunciation = ipa;
        }
        if let Some(path) = markup::resource(root, &string(sound, "audio"), true) {
            entry.audio.push(path);
        }
    }
    let pos = string(value, "pos");
    for original in array(value, "senses") {
        let mut sense = Sense {
            part_of_speech: pos.clone(),
            definitions: strings(original, "glosses"),
            translations: translations(original, language.opposite()),
            examples: words(original, "examples", "text"),
            collocations: words(original, "collocations", "word"),
            synonyms: words(original, "synonyms", "word"),
            antonyms: words(original, "antonyms", "word"),
            word_family: words(original, "derived", "word"),
        };
        if sense.definitions.is_empty() {
            sense.definitions = strings(original, "raw_glosses");
        }
        // Entry-level links are not reassigned to every distinct sense.
        if entry.senses.is_empty() {
            append_unique(
                &mut sense.translations,
                translations(value, language.opposite()),
            );
            append_unique(
                &mut sense.collocations,
                words(value, "collocations", "word"),
            );
            append_unique(&mut sense.synonyms, words(value, "synonyms", "word"));
            append_unique(&mut sense.antonyms, words(value, "antonyms", "word"));
            append_unique(&mut sense.word_family, words(value, "derived", "word"));
            append_unique(&mut sense.word_family, words(value, "related", "word"));
        }
        entry.senses.push(sense);
    }
    if entry.senses.is_empty() {
        entry.senses.push(Sense {
            part_of_speech: pos,
            translations: translations(value, language.opposite()),
            ..Default::default()
        });
    }
    entry
}
fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn string(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").into()
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    array(value, key)
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .collect()
}
fn words(value: &Value, key: &str, field: &str) -> Vec<String> {
    array(value, key)
        .iter()
        .filter_map(|v| v.as_str().or_else(|| v.get(field).and_then(Value::as_str)))
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .collect()
}
fn translations(value: &Value, target: Language) -> Vec<String> {
    array(value, "translations")
        .iter()
        .filter(|t| string(t, "lang_code") == target.code() || string(t, "code") == target.code())
        .map(|t| string(t, "word"))
        .filter(|s| !s.is_empty())
        .collect()
}
fn append_unique(target: &mut Vec<String>, extra: Vec<String>) {
    for value in extra {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}
