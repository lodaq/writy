# Arabic and English dictionary sources

Research checked 2026-10-25 against publisher/project sources. This is a practical catalogue, not a claim to enumerate every dictionary on the internet. A readable website, downloadable mobile application, or GitHub code licence does not automatically grant permission to redistribute its dictionary content. Counts and download sizes change.

## Recommended offline foundation

Use several complementary source families: FreeDict for Arabic–English translations, Open English WordNet for English senses and semantic relations, Arabic WordNet for Arabic synsets, and Wiktionary for additional definitions, forms, and examples. Preserve provenance for each sense. Two editions or mirrors of the same upstream dictionary are not independent evidence.

Dictionaries supply word/phrase evidence, not dependable translations of arbitrary paragraphs. Sentence translation still needs the translation model; retrieve relevant lexical senses into its single structured request and keep retrieved text separate from instructions. Do not label generated paraphrases or examples as dictionary quotations.

## Downloadable dictionaries and lexical databases

| Source | Coverage and useful fields | Download options | Licence and integration decision |
| --- | --- | --- | --- |
| **FreeDict English → Arabic** | 87,424 headwords in release 0.6.3; translations; derived from Arabeyes | [Official downloads](https://freedict.org/downloads/): TEI XML source, StarDict, DICT/dictd archives; [source TEI and licence header](https://github.com/freedict/fd-dictionaries/blob/master/eng-ara/eng-ara.tei) | GPL-2.0-or-later. Good compact bilingual pack; retain original notices and source. |
| **FreeDict Arabic → English** | 52,996 headwords in release 0.6.3; separately indexed reverse direction | [Official downloads](https://freedict.org/downloads/); [source TEI and licence header](https://github.com/freedict/fd-dictionaries/blob/master/ara-eng/ara-eng.tei) | GPL-2.0-or-later. Install alongside English→Arabic; do not assume reversing strings produces an equivalent dictionary. |
| **English Wiktionary via Kaikki: English entries** | Definitions, parts of speech, examples, forms, pronunciations, translations where supplied | [English download page](https://kaikki.org/dictionary/English/index.html); [recommended raw JSONL downloads](https://kaikki.org/dictionary/rawdata.html) and [schema/extractor](https://github.com/tatuylonen/wiktextract) | [Wiktionary copyright terms](https://en.wiktionary.org/wiki/Wiktionary:Copyrights): CC-BY-SA-4.0/GFDL for original entry text. Keep attribution/entry links; audio and quotations need their own provenance. Large; stream into an index. |
| **English Wiktionary via Kaikki: Arabic entries** | Arabic headwords with **English glosses**; forms and grammatical information | [Arabic download page](https://kaikki.org/dictionary/Arabic/index.html), raw JSONL as above | Same Wiktionary family/licensing. This is not an Arabic-language gloss dictionary. Postprocessed per-language files are marked deprecated; prefer raw extraction for new importers. |
| **Arabic Wiktionary** | Arabic-language edition; language coverage and gloss conventions differ | [Official Wikimedia dump service](https://dumps.wikimedia.org/) — select `arwiktionary` and the pages/articles XML dump; run an extractor supporting that edition | [Edition copyright page](https://ar.wiktionary.org/wiki/ويكاموس:حقوق_التأليف_والنشر). Do not presume Kaikki currently provides a supported Arabic-edition extraction; its current raw-edition list does not. |
| **Open English WordNet (OEWN)** | English definitions, synsets, examples and semantic relations; maintained descendant of Princeton WordNet | [Official repository and release links](https://github.com/globalwordnet/english-wordnet): [2025 JSON ZIP](https://en-word.net/static/english-wordnet-2025-json.zip), [LMF XML gzip](https://en-word.net/static/english-wordnet-2025.xml.gz), WNDB and RDF; core and Plus editions | CC-BY-4.0. Strong default English semantic source. Repository requires manual validation of automatically generated additions. |
| **Princeton WordNet** | Original English synset dictionary; useful stable alignment IDs | [Official download page](https://wordnet.princeton.edu/download) and [licence](https://wordnet.princeton.edu/license-and-commercial-use) | WordNet licence permits commercial use, copying/modification/distribution with notices. Historical baseline; OEWN adds maintenance. Do not count both as independent corroboration. |
| **Arabic WordNet AWN v2 / Open Multilingual WordNet** | Arabic lexicalizations connected to Princeton WordNet 3.0 senses | [OMW data and download instructions](https://omwn.org/omw1.html); [official data repository](https://github.com/omwn/omw-data); NLTK `omw-1.4` download | OMW lists AWN as CC-BY-SA-3.0. The Arabic table uses language code `arb`. Match the declared WordNet version, not unrelated synset numbers. Coverage is smaller than broad dictionaries. |
| **Qabas / قبس** | About 58,000 Arabic lemmas, morphology, links to 110 lexicons and corpora | [Official introduction/download form](https://sina.birzeit.edu/qabas/about) | CC-BY-ND-4.0; commercial use allowed with attribution, modified redistribution prohibited. The project deliberately links source gloss identifiers instead of redistributing the original definitions. Keep as a separately licensed optional resource; do not merge and redistribute a transformed derivative by default. |
| **Arabic Ontology / الأنطولوجيا العربية** | Arabic concepts/synsets and semantic classification; useful Arabic meaning distinctions | [SinaLab resources/download access](https://sina.birzeit.edu/resources/), [official resource catalogue](https://sinalab.github.io/resources/index.html) | Catalogue states CC-BY-4.0 for Arabic Ontology; free/premium editions differ. Obtain the actual published package and retain its terms; do not confuse it with the separately restricted 150-lexicon collection. |
| **Arramooz Alwaseet / الراموز الوسيط** | Arabic noun/verb lexicon, roots, vocalization, inflection features and some definitions | [Author repository](https://github.com/linuxscout/arramooz), [publisher downloads](https://sourceforge.net/projects/arramooz/files/): text/TSV, SQL, XML, StarDict | Project declares GPL; use the precise COPYING terms shipped with the selected artifact. Useful morphology/word-family complement; older releases require encoding/data-quality checks. |
| **GNU GCIDE** | English definitions and etymologies, including historical material | [Maintainer downloads](https://gcide.gnu.org.ua/download): tar.gz, tar.xz, ZIP, signatures; `CIDE.A`–`CIDE.Z` marked-up corpus | Inspect COPYING in the archive; project uses GPL-3.0-or-later. Good additional English dictionary, with historical usage clearly distinguished from contemporary advice. [GNU project page](https://www.gnu.org/software/gcide/). |
| **Webster's Unabridged / 1913 family** | Historical English dictionary | [Project Gutenberg ebook 29765](https://www.gutenberg.org/ebooks/29765): plain text, HTML ZIP, EPUB | Publisher marks public domain in USA; Gutenberg licence/trademark conditions still accompany the file. Historical definitions/OCR errors need labels; not a modern usage authority. |
| **OPTED conversions** | Webster-derived English word definitions in easier database formats | [Conversion author's SQLite/JSON repository](https://github.com/eddydn/DictionaryDatabase) | Convenience alternative, not independent evidence. Repository does not establish a blanket new content licence; inspect upstream text and transformation provenance before distributing. Prefer the original corpus if unclear. |
| **Arabic Almanac / ejtaal compilation** | Numerous classical/modern Arabic dictionaries including scanned reference works | [Maintainer's repository](https://github.com/ejtaal/mr) provides the web application and bundled reference assets | Useful catalogue/reference viewer. Treat each underlying dictionary separately; an application repository does not clear rights for modern books or scans. Image pages need OCR and manual correction before lexical indexing. |

FreeDict publishes a [machine-readable package catalogue](https://freedict.org/freedict-database.json) with release URLs, platforms and checksums. Prefer it over invented archive names; validate the selected archive checksum before import. Its [download documentation](https://freedict.org/downloads/) distinguishes compiled dictionaries from TEI sources and generated imports.

## Supporting data: morphology, spelling, examples

These complement dictionaries; they are not interchangeable with definitions.

| Resource | Download and use | Terms/limits |
| --- | --- | --- |
| **CAMeL Tools morphology databases** | [Official package catalogue](https://camel-tools.readthedocs.io/en/latest/reference/packages.html); `camel_data` installs MSA and Egyptian Arabic databases. Useful analysis/lemmatization before lookup. | The documented MSA/Egyptian morphology packages are GPL v2. Toolkit licensing does not replace package licensing. Models and other data have different terms. |
| **Qutrub / قطرب** | [Author repository](https://github.com/linuxscout/qutrub), [downloads](https://qutrub.arabeyes.org/download/), `libqutrub` package; Arabic verb conjugation with CSV/XML and other exports | GPL per project; a conjugator, not a general definition dictionary. |
| **LibreOffice/Hunspell dictionaries** | [Official dictionary repository](https://github.com/LibreOffice/dictionaries) includes Arabic and English; `.dic`/`.aff` spelling dictionaries, and some thesaurus files. [UTF-8 packaged dictionaries](https://github.com/wooorm/dictionaries) are another distribution. | Read each language pack's licence. Word acceptance/inflection rules do not supply bilingual meanings. Shared upstream packs are duplicates, not extra evidence. |
| **Quranic Arabic Corpus** | [Official morphology download](https://corpus.quran.com/download/) via email verification; annotated Quranic morphology | Download page specifies GPL. Specialized classical/religious corpus; label its domain and do not treat corpus glosses as general modern-Arabic defaults. |
| **Tatoeba** | [Official downloads](https://tatoeba.org/en/downloads): custom Arabic–English sentence-pair export, weekly TSV and sentence links | Sentences CC-BY-2.0-FR, with a CC0 subset; audio licences vary by contributor. Preserve IDs/attribution and direct translation links; examples are community contributions. |
| **OPUS** | [Official collection](https://opus.nlpl.eu/) and corpus/language selectors; parallel plain text, TMX and other formats vary by corpus | Review each corpus's licence and origin. Includes mined, subtitle, translated and synthetic collections; use quality filters, and do not label mined alignments as curated dictionary senses. |
| **DICT/dictd packages** | [DICT project](https://www.dict.org/) and FreeDict compiled downloads provide client/server-compatible dictionary databases | DICT is a protocol/container, not one dictionary or universal licence. Keep each hosted database's copyright and identity; a public server is not a bulk-download entitlement. |

## Publisher APIs and permission-dependent datasets

| Publisher/service | What is available | Offline/download decision |
| --- | --- | --- |
| **Birzeit lexicographic search engine (150 lexicons)** | [Official resources](https://sina.birzeit.edu/resources/) document authenticated REST lookup and a special licence for the lexicographic collection | Request a token/licence or permitted dataset. Qabas's openness does not open all linked books; do not scrape the full collection. |
| **Lexicala / K Dictionaries** | [API documentation](https://api.lexicala.com/documentation/) lists Arabic/English resources and JSON endpoints for definitions/translations/examples | Standard API does not permit local caching. Publisher offers separately purchased XML/JSON/JSON-LD datasets for direct integration. Useful commercial option after an appropriate contract. |
| **Oxford Dictionaries** | [Official API FAQ](https://developer.oxforddictionaries.com/faq) and licensed dictionary datasets | Offline storage requires an Enterprise licence. [Current API terms](https://developer.oxforddictionaries.com/api-terms-and-conditions) also restrict combining standard API content with AI tools, including prompting/grounding. Do not feed standard API results to Writy's model without separate permission. |
| **Merriam-Webster** | [Official API products](https://dictionaryapi.com/products/index): English general, learner, medical dictionaries and thesauri; JSON | [FAQ](https://dictionaryapi.com/info/frequently-asked-questions): noncommercial free use within 1,000 queries/day/key and two reference APIs; commercial use requires licensing. This is not an Arabic bilingual source or an unrestricted downloadable corpus. Follow [terms](https://dictionaryapi.com/info/terms-of-service). |
| **arabdict** | [Official downloads](https://www.arabdict.com/en/en-ar/downloads) provide Android/Apple applications and links to bilingual/Arabic dictionaries | Downloading its application is not an open dictionary-data grant. A bulk corpus licence was not verified; use as a human reference or negotiate permission. |
| **dictionaryapi.dev** | [Project repository](https://github.com/meetDeveloper/freeDictionaryAPI) offers an English lookup service | API service/software is distinct from an independently downloadable corpus. Inspect returned content licences and provenance; do not assume its software GPL covers all underlying definitions or Arabic support. |
| **Linguatools** | [Publisher data/API catalogue](https://linguatools.org/) offers collocations, dictionaries with bilingual examples, and other language products | Data packages and API rights depend on product/language/contract. Verify Arabic–English availability and offline rights before integration. |

Almaany, Cambridge, Collins, Longman, al-Mawrid, Hans Wehr, and modern Arabic academy dictionaries are valuable names to investigate with their publishers. This research does not establish an authorized reusable bulk download for them. Unofficial StarDict/MDX mirrors should not be installed merely because a download exists.

## Import and retrieval contract for Writy

The following are engineering recommendations inferred from the source formats and licensing differences above:

1. Ship an explicit source registry: source family, original title, release, original URL, download URL, SHA-256/checksum, language direction, licence URL/text, attribution, enabled state, imported entry count, and format. Show installed sources separately from possible downloads.
2. Keep independent source records and senses rather than flattening contradictory translations. Rank exact headword/phrase matches before normalized/lemma matches. Preserve original Arabic spelling; use diacritic/tatweel-insensitive keys without discarding the display form.
3. Label dictionary text by source and language. The model may select a contextually appropriate sense, but must not fabricate source citations. Keep generated content visibly separate.
4. Query all enabled local packs in one local retrieval step. Supply a bounded, relevant subset to the existing single model request; do not issue one model call per dictionary.
5. Preserve notices/source offers and attribution for redistributable packs. Keep restricted/API-only resources optional and outside automatic downloads. Store ND resources separately until the intended indexing/distribution arrangement is verified.
6. Prefer documented structured sources over PDF/OCR or unknown scraped databases. Benchmark common words, inflections, ambiguity, Arabic/English direction, and long passages. More dictionaries increase coverage, not necessarily correctness.

A good first installation is both FreeDict directions plus OEWN, followed by AWN and carefully attributed Wiktionary packs. This combines bilingual lookup, English semantic relations and Arabic meanings without treating a language model as a dictionary.

## Commands for the implemented importers

`writy dict list` reports what is actually installed. The built-in bilingual installer supports both directions:

```sh
writy dict install both
writy dict list
```

The following commands download per-language Kaikki JSONL archives, decompress them, and import with explicit attribution. These per-language convenience exports are currently marked deprecated upstream; keep the catalogue/raw-download page above bookmarked for future migrations. Downloads are substantial (approximately 510 MiB English and 69 MiB Arabic compressed when checked). The importer reads **plain JSONL**, not gzip. Use distinct paths and source names for the two languages.

```sh
mkdir -p "$HOME/.local/share/writy/packs/wiktionary"
cd "$HOME/.local/share/writy/packs/wiktionary"
curl --fail --location --continue-at - --output English.jsonl.gz \
  https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.jsonl.gz
curl --fail --location --continue-at - --output Arabic.jsonl.gz \
  https://kaikki.org/dictionary/Arabic/kaikki.org-dictionary-Arabic.jsonl.gz
gzip -dk English.jsonl.gz
gzip -dk Arabic.jsonl.gz
# From the Writy repository root, retain/log unusable upstream headwords:
python3 /path/to/writy/tools/clean_dictionary_jsonl.py English.jsonl English.usable.jsonl English.rejected.jsonl
writy dict import English.usable.jsonl --language en --name 'Wiktionary English (Kaikki)' \
  --license CC-BY-SA-4.0 --url https://en.wiktionary.org/
writy dict import Arabic.jsonl --language ar --name 'Wiktionary Arabic (Kaikki)' \
  --license CC-BY-SA-4.0 --url https://en.wiktionary.org/
```

Open English WordNet uses a different JSON structure; convert it using the repository's `tools/wordnet_to_jsonl.py` before importing. Run the converter from the Writy repository root:

```sh
mkdir -p "$HOME/.local/share/writy/packs/wordnet"
curl --fail --location --continue-at - \
  --output "$HOME/.local/share/writy/packs/wordnet/english-wordnet-2025-json.zip" \
  https://en-word.net/static/english-wordnet-2025-json.zip
python3 tools/wordnet_to_jsonl.py \
  "$HOME/.local/share/writy/packs/wordnet/english-wordnet-2025-json.zip" \
  "$HOME/.local/share/writy/packs/wordnet/english-wordnet-2025.jsonl"
writy dict import "$HOME/.local/share/writy/packs/wordnet/english-wordnet-2025.jsonl" \
  --language en --name 'Open English WordNet 2025' \
  --license CC-BY-4.0 --url https://github.com/globalwordnet/english-wordnet
```

The converter preserves distinct synset senses, definitions, examples and synonym members. It does not invent collocations, word families, translations, or antonym relations. Original OEWN data and licence remain the provenance for the converted records.

During this implementation the installed database was verified to contain both FreeDict directions (87,424 English→Arabic and 52,996 Arabic→English entries), 1,499,160 English Wiktionary records, 233,417 Arabic Wiktionary records and 185,129 OEWN word-sense records. `writy dict list` is the authoritative current inventory. Record counts are not unique headword counts and should not be summed as independent dictionary quality scores.

The checked 2026-10-21 English Kaikki export includes one whitespace-only headword
at line 385,903. The original gzip and JSONL were retained; that one record was
excluded from the imported copy and saved in `English.rejected.jsonl`. The cleanup
tool keeps valid records unchanged, logs rejected records, and fails on malformed
JSON. Do not apply that line number to a different release; run the cleaner.

Run converter/cleanup regression tests with `python3 -m unittest discover -s tools`.

## Additional free sources checked 2026-10-25

This follow-up separates newly identified reusable data from free reading websites. No additional packs were installed during this research pass.

| Source | Free download/access | Reuse status and value for Writy |
| --- | --- | --- |
| **Lane's Arabic–English Lexicon: SQLite edition** | Graeme Andrews's [LexiconDatabase repository](https://github.com/laneslexicon/LexiconDatabase) contains `lexicon.sqlite.zip`, release downloads and change notes. [Project overview](https://laneslexicon.github.io/) identifies Perseus XML as its source. | Repository supplies GPL-3.0 licence. Strong candidate for **additional classical Arabic–English evidence**. Needs schema conversion, original attribution retention and classical-usage labelling; not a drop-in Writy SQLite database. |
| **Lane's Lexicon: editable XML / corrected editions** | [Amended Perseus XML](https://github.com/laneslexicon/lexicon_xml), [proofreading project](https://github.com/naveedulislam/lan), and [maintainer's PDF/scan downloads](https://lanelexicon.com/updates/). The latter states its 2024 generated PDFs include CC-BY-SA-3.0 notices. | Prefer structured XML over scans for indexing. Verify the selected XML edition's terms separately; do not transfer the SQLite repository's GPL or PDF edition's licence to every XML file by assumption. All are the **same Lane source family**, not independent dictionaries. |
| **Moby Thesaurus II** | [Project Gutenberg 3202](https://www.gutenberg.org/ebooks/3202): text/HTML downloads and additional source files | Gutenberg marks it public domain in USA. Useful **new English related-word source**; broad related terms must not all become exact synonyms. Keep Gutenberg conditions and original author notices; no Arabic translations. |
| **KOReader Arabic thesaurus `ar-ar`** | [Official download catalogue](https://danielgregorini.github.io/koreader-dicts/en/downloads/): StarDict ZIP, 12,766 entries, approximately 1.1 MB | CC-BY-SA-3.0 per catalogue. [Builder explains](https://github.com/DanielGregorini/koreader-dicts) it is Arabic WordNet synonym sets, **not Arabic definitions**. Helpful packaged AWN access, not independent evidence. |
| **KOReader English→Arabic `en-ar`** | [Download and coverage page](https://danielgregorini.github.io/koreader-dicts/en/en-ar/): StarDict ZIP, approximately 12.3 MB | CC-BY-SA-4.0 compilation. 155,624 entries but only 20.85% contain a translation; total entries must not be advertised as translated headwords. Derived from Princeton WordNet/AWN and Wiktionary, with inflection indexing. |
| **KOReader Arabic→English `ar-en`** | [Download and coverage page](https://danielgregorini.github.io/koreader-dicts/en/ar-en/): StarDict ZIP, approximately 4.3 MB, 28,892 entries | CC-BY-SA-4.0 per catalogue. Derived WordNet/Wiktionary family; useful ready-made format, not another independently authored dictionary. Retain all archive attribution notices. |
| **Ayaspell Arabic thesaurus / spelling packs** | [Author project page](https://ayaspell.sourceforge.net/index.html) links Arabic thesaurus, Hunspell dictionary and office-extension downloads | Declares GPL/LGPL/MPL tri-licensing. Thesaurus may add Arabic synonym material; spelling pack is a different resource and does not provide meanings. Check selected archive/version and overlap with LibreOffice/Arramooz before counting independent sources. |
| **Arabeyes Technical Computing Dictionary** | [Publisher's downloadable bilingual PDF](https://www.arabeyes.org/~djihed/techdict/archive/arabdict-24-07-2015.pdf) | PDF explicitly specifies FreeBSD Documentation License. Useful English–Arabic **computing terminology**, separate from a general paragraph translator. Structured extraction/proofreading required; retain credits/licence. |
| **Wikidata Lexemes** | [Official data access/download documentation](https://www.wikidata.org/wiki/Wikidata:Data_access) describes JSON/RDF dumps and individual-entity access | Structured Wikidata data is CC0. Potential supplementary English/Arabic lemmas, forms and sense links; coverage varies. Filter lexeme language explicitly; item labels are not dictionary definitions. |
| **OpenITI classical lexicons** | [Official documentation](https://kitab-project.org/docs/openITI) and [text repositories](https://github.com/orgs/OpenITI/repositories) provide downloadable plain text/mARkdown | Corpus CC-BY-NC-SA-4.0 according to project documentation. Useful classical reference/extraction research, but **noncommercial restriction** prevents treating it as a general unrestricted redistribution pack. The Python tooling's MIT licence is separate from corpus terms. |
| **Ohod Arabic spelling dictionary** | [Author download page](https://sourceforge.net/projects/arabic-dictionary/files/) and [project summary](https://sourceforge.net/projects/arabic-dictionary/) | Project declares GPLv3 and describes an expanded Arabeyes spelling list. Spelling acceptance only, **not definitions/translations**. Distinguish dictionary archive from its builder script; advertised counts differ, so inspect actual artifact. |
| **ABC Thesaurus** | [Publisher data downloads](https://abcthesaurus.com/data/): English JSONL.gz, edges CSV and comparison notes | Publisher declares CC-BY-SA-4.0; Tatoeba examples retain CC-BY-2.0-FR. [Methodology page](https://abcthesaurus.com/about/) claims sense-level review, but detailed provenance for legacy lexical sources and independent quality verification remain incomplete. Optional evaluation candidate; do not equate publisher claims with established validation. |
| **WikDict** | [Official downloads](https://www.wikdict.com/page/download): StarDict, SQLite, TEI P5, derived from Wiktionary via DBnary under CC BY-SA | The [current StarDict index](https://download.wikdict.com/dictionaries/stardict/) checked here **does not list Arabic pairs**. Useful for English↔other languages; do not invent an English–Arabic pack URL or count it as Arabic coverage. |
| **Riyadh Dictionary / معجم الرياض** | [Official free lookup website](https://dictionary.ksaa.gov.sa/); publisher's [platform catalogue](https://ksaa.gov.sa/ar/platforms) also lists Siwar dictionary search and its digital library | Contemporary Arabic reference available to read. No reusable bulk dictionary grant/download was verified. Free website/mobile access is different from an open dataset; keep as a reference/link until publisher data permission is established. |
| **PONS English–Arabic** | [Official free bidirectional lookup](https://en.pons.com/translate/english-arabic), with word/phrase search, pronunciation and examples | Free online lookup; **open bulk download/reuse rights were not verified**. Good human reference; do not scrape/install its dictionary as though it were an open pack. |

For immediate future integration, the strongest distinct additions are **Lane SQLite/XML for classical Arabic**, **Moby for English related terms**, and **Ayaspell's Arabic thesaurus** after checking overlap and artifact notices. KOReader packages improve distribution and inflection coverage, while largely repeating already known WordNet/Wiktionary source families. Restricted corpora and free-to-read commercial websites should remain clearly labelled catalogue options.
