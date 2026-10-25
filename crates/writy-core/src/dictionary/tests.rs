use super::*;
use std::{fs, io::Write};
use tempfile::TempDir;

fn source() -> Attribution {
    Attribution {
        name: "Fixture".into(),
        url: "https://example.test/dictionary".into(),
        license: "CC0".into(),
    }
}
fn stardict(
    dir: &Path,
    words: &[(&str, &str)],
    aliases: &[(&str, u32)],
    types: &str,
) -> std::path::PathBuf {
    let mut idx = Vec::new();
    let mut dict = Vec::new();
    for (word, body) in words {
        idx.extend_from_slice(word.as_bytes());
        idx.push(0);
        idx.extend_from_slice(&(dict.len() as u32).to_be_bytes());
        idx.extend_from_slice(&(body.len() as u32).to_be_bytes());
        dict.extend_from_slice(body.as_bytes());
    }
    fs::write(dir.join("fixture.idx"), &idx).unwrap();
    fs::write(dir.join("fixture.dict"), dict).unwrap();
    let mut syn = Vec::new();
    for (alias, target) in aliases {
        syn.extend_from_slice(alias.as_bytes());
        syn.push(0);
        syn.extend_from_slice(&target.to_be_bytes());
    }
    if !syn.is_empty() {
        fs::write(dir.join("fixture.syn"), syn).unwrap();
    }
    let ifo = dir.join("fixture.ifo");
    fs::write(&ifo, format!("StarDict's dict ifo file\nversion=2.4.2\nbookname=Fixture\nwordcount={}\nidxfilesize={}\nsynwordcount={}\nsametypesequence={}\n", words.len(), idx.len(), aliases.len(), types)).unwrap();
    ifo
}

#[test]
fn stardict_roundtrip_keeps_senses_and_synonym_aliases() {
    let dir = TempDir::new().unwrap();
    let ifo = stardict(
        dir.path(),
        &[("Decision", "a choice\nan outcome"), ("كِتَاب", "book")],
        &[("decisions", 0)],
        "m",
    );
    let path = dir.path().join("lookup.sqlite");
    let mut db = Dictionary::open(&path).unwrap();
    assert_eq!(
        db.import_stardict(&ifo, Language::English, source())
            .unwrap(),
        2
    );
    drop(db);
    let db = Dictionary::open(path).unwrap();
    let found = db.lookup("DECISIONS").unwrap();
    assert_eq!(found[0].headword, "Decision");
    assert!(found[0].senses[0].definitions[0].contains("a choice"));
    assert_eq!(db.lookup("كتاب").unwrap()[0].headword, "كِتَاب");
    assert_eq!(db.complete("dec", 3).unwrap(), vec!["Decision"]);
}

#[test]
fn corrupt_stardict_import_does_not_replace_existing_source() {
    let dir = TempDir::new().unwrap();
    let ifo = stardict(dir.path(), &[("word", "original")], &[], "m");
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_stardict(&ifo, Language::English, source())
        .unwrap();
    fs::write(dir.path().join("fixture.idx"), b"broken").unwrap();
    assert!(
        db.import_stardict(&ifo, Language::English, source())
            .is_err()
    );
    assert_eq!(
        db.lookup("word").unwrap()[0].senses[0].definitions[0],
        "original"
    );
}

#[test]
fn gzip_dictionary_and_markup_are_imported_without_unsafe_resources() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("res")).unwrap();
    fs::write(dir.path().join("res/safe.ogg"), b"fixture").unwrap();
    let html = "<b>Meaning</b><script>secret</script><img src=\"https://evil.test/x\"/><img src=\"../../outside.png\"/><a href=\"sound://safe.ogg\">sound</a>";
    let ifo = stardict(dir.path(), &[("word", html)], &[], "h");
    let data = fs::read(dir.path().join("fixture.dict")).unwrap();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&data).unwrap();
    fs::write(dir.path().join("fixture.dict.dz"), enc.finish().unwrap()).unwrap();
    fs::remove_file(dir.path().join("fixture.dict")).unwrap();
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_stardict(&ifo, Language::English, source())
        .unwrap();
    let found = db.lookup("word").unwrap();
    let e = &found[0];
    assert!(e.senses[0].definitions[0].contains("Meaning"));
    assert!(!e.senses[0].definitions[0].contains("secret"));
    assert!(!e.senses[0].definitions[0].contains("<b>"));
    assert!(e.images.is_empty());
    assert_eq!(e.audio.len(), 1);
}

#[test]
fn kaikki_preserves_distinct_senses_and_original_arabic_spelling() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("words.jsonl");
    fs::write(&input,r#"{"word":"كِتَاب","lang_code":"ar","pos":"noun","sounds":[{"ipa":"/kitaːb/","audio":"remote.ogg"}],"senses":[{"glosses":["book"],"examples":[{"text":"هذا كتاب"}],"synonyms":[{"word":"مؤلف"}]},{"glosses":["document"]}],"forms":[{"form":"كتب"}],"translations":[{"lang_code":"en","word":"book"}]}
{"word":"cat","lang_code":"en","senses":[{"glosses":["animal"]}]}
"#).unwrap();
    let mut db = Dictionary::open(":memory:").unwrap();
    assert_eq!(
        db.import_kaikki(&input, Language::Arabic, source())
            .unwrap(),
        1
    );
    let e = &db.lookup("كتاب").unwrap()[0];
    assert_eq!(e.headword, "كِتَاب");
    assert_eq!(e.senses.len(), 2);
    assert_eq!(e.senses[0].examples, vec!["هذا كتاب"]);
    assert_eq!(e.senses[1].definitions, vec!["document"]);
    assert_eq!(e.senses[0].translations, vec!["book"]);
    assert!(e.audio.is_empty());
    assert_eq!(db.lookup("كتب").unwrap()[0].headword, "كِتَاب");
}

#[test]
fn oversized_line_and_invalid_alias_target_are_rejected() {
    let dir = TempDir::new().unwrap();
    let ifo = stardict(dir.path(), &[("word", "text")], &[("alias", 7)], "m");
    let mut db = Dictionary::open(":memory:").unwrap();
    assert!(
        db.import_stardict(&ifo, Language::English, source())
            .is_err()
    );
    let input = dir.path().join("large.jsonl");
    fs::write(&input, vec![b'x'; 2 * 1024 * 1024 + 1]).unwrap();
    assert!(
        db.import_kaikki(&input, Language::English, source())
            .is_err()
    );
}

#[test]
fn failed_kaikki_import_rolls_back_previous_source_and_entries() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("words.jsonl");
    fs::write(
        &input,
        "{\"word\":\"word\",\"lang_code\":\"en\",\"senses\":[{\"glosses\":[\"original\"]}]}\n",
    )
    .unwrap();
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_kaikki(&input, Language::English, source())
        .unwrap();
    fs::write(
        &input,
        "{\"word\":\"replacement\",\"lang_code\":\"en\"}\ninvalid json\n",
    )
    .unwrap();
    assert!(
        db.import_kaikki(&input, Language::English, source())
            .is_err()
    );
    assert_eq!(db.lookup("word").unwrap().len(), 1);
    assert!(db.lookup("replacement").unwrap().is_empty());
}

#[test]
fn compressed_index_and_dotted_dictionary_name_work() {
    let dir = TempDir::new().unwrap();
    stardict(dir.path(), &[("word", "a &amp; b")], &[], "h");
    for ext in ["ifo", "dict"] {
        fs::rename(
            dir.path().join(format!("fixture.{ext}")),
            dir.path().join(format!("my.dictionary.{ext}")),
        )
        .unwrap();
    }
    let idx = fs::read(dir.path().join("fixture.idx")).unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&idx).unwrap();
    fs::write(
        dir.path().join("my.dictionary.idx.gz"),
        gzip.finish().unwrap(),
    )
    .unwrap();
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_stardict(
        &dir.path().join("my.dictionary.ifo"),
        Language::English,
        source(),
    )
    .unwrap();
    assert_eq!(
        db.lookup("word").unwrap()[0].senses[0].definitions[0],
        "a & b"
    );
}

#[test]
fn normalization_preserves_original_headwords_and_ignores_query_sql() {
    let dir = TempDir::new().unwrap();
    let ifo = stardict(dir.path(), &[("إِيمَان", "faith")], &[], "m");
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_stardict(&ifo, Language::Arabic, source())
        .unwrap();
    assert_eq!(db.lookup("ايمان").unwrap()[0].headword, "إِيمَان");
    assert!(db.lookup("' OR 1=1 --").unwrap().is_empty());
    assert!(db.complete("%", 10).unwrap().is_empty());
    assert!(db.complete("", 10).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn resource_symlinks_cannot_escape_dictionary_directory() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("res")).unwrap();
    fs::write(outside.path().join("secret.ogg"), b"secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.ogg"),
        dir.path().join("res/secret.ogg"),
    )
    .unwrap();
    let ifo = stardict(
        dir.path(),
        &[("word", "<a href=\"sound://secret.ogg\">meaning</a>")],
        &[],
        "h",
    );
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_stardict(&ifo, Language::English, source())
        .unwrap();
    assert!(db.lookup("word").unwrap()[0].audio.is_empty());
}

#[test]
fn bilingual_import_populates_translation_instead_of_definition_huds() {
    let dir = TempDir::new().unwrap();
    let ifo = stardict(dir.path(), &[("decision", "<b>قرار</b>")], &[], "h");
    let mut db = Dictionary::open(":memory:").unwrap();
    db.import_bilingual_stardict(&ifo, Language::English, source())
        .unwrap();
    let entries = db.lookup("decision").unwrap();
    assert_eq!(entries[0].senses[0].translations, vec!["قرار"]);
    assert!(entries[0].senses[0].definitions.is_empty());
    assert!(entries[0].definition_markup.is_none());
}

#[test]
fn lookups_keep_committed_entries_available_during_a_reimport() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("concurrent.sqlite");
    let ifo = stardict(dir.path(), &[("decision", "choice")], &[], "m");
    let mut db = Dictionary::open(&path).unwrap();
    db.import_stardict(&ifo, Language::English, source())
        .unwrap();
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("BEGIN EXCLUSIVE; DELETE FROM lexical_entries;")
        .unwrap();
    assert_eq!(db.lookup("decision").unwrap().len(), 1);
    assert_eq!(db.complete("dec", 3).unwrap(), vec!["decision"]);
    writer.execute_batch("ROLLBACK").unwrap();
    assert_eq!(db.lookup("decision").unwrap().len(), 1);
}

#[test]
fn lookup_keeps_multiple_sources_when_one_has_many_senses() {
    let dir = TempDir::new().unwrap();
    let mut dictionary = Dictionary::open(dir.path().join("dictionary.db")).unwrap();
    let file = dir.path().join("entries.jsonl");
    fs::write(
        &file,
        "{\"word\":\"test\",\"lang_code\":\"en\",\"senses\":[{\"glosses\":[\"first\"]}]}\n"
            .repeat(55),
    )
    .unwrap();
    dictionary
        .import_kaikki(&file, Language::English, source())
        .unwrap();
    fs::write(
        &file,
        "{\"word\":\"test\",\"lang_code\":\"en\",\"senses\":[{\"glosses\":[\"second\"]}]}\n",
    )
    .unwrap();
    let mut second = source();
    second.name = "Second dictionary".into();
    dictionary
        .import_kaikki(&file, Language::English, second)
        .unwrap();
    let entries = dictionary.lookup("test").unwrap();
    assert_eq!(entries.len(), 50);
    assert!(
        entries
            .iter()
            .any(|entry| entry.source.name == "Second dictionary")
    );
}
