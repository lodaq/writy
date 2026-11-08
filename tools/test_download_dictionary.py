import gzip
import io
import json
from pathlib import Path
import stat
import tempfile
import tarfile
import unittest
from unittest.mock import patch
import zipfile

import download_dictionary as installer
from clean_dictionary_jsonl import clean
from wordnet_to_jsonl import entries


class DictionaryPreparationTests(unittest.TestCase):
    def test_arramooz_keeps_companion_files_and_licence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'source.zip'
            with zipfile.ZipFile(archive, 'w') as output:
                for name in ['arramooz.ifo', 'arramooz.idx', 'arramooz.dict.dz', 'LICENSE']:
                    output.writestr(name, 'fixture')
            installer.prepare(archive, root / 'prepared', 'arramooz')
            self.assertTrue((root / 'prepared/LICENSE').is_file())
            self.assertTrue((root / 'prepared/arramooz.dict.dz').is_file())

    def test_archive_paths_links_and_size_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ['../escape', '/absolute', 'nested\\escape']:
                with zipfile.ZipFile(root / 'source.zip', 'w') as output:
                    output.writestr(name, 'bad')
                with self.assertRaises(ValueError):
                    installer.unpack_zip(root / 'source.zip', root / 'prepared')
            member = zipfile.ZipInfo('link')
            member.create_system = 3
            member.external_attr = (stat.S_IFLNK | 0o777) << 16
            with zipfile.ZipFile(root / 'source.zip', 'w') as output:
                output.writestr(member, '../escape')
            with self.assertRaises(ValueError):
                installer.unpack_zip(root / 'source.zip', root / 'prepared')
            with patch.object(installer, 'MAX_EXTRACTED', 1):
                with zipfile.ZipFile(root / 'source.zip', 'w') as output:
                    output.writestr('large', '123')
                with self.assertRaises(ValueError):
                    installer.unpack_zip(root / 'source.zip', root / 'prepared')

    def test_wordnet_preserves_senses_synonyms_and_examples(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'source.zip'
            with zipfile.ZipFile(archive, 'w') as output:
                output.writestr('noun.fixture.json', json.dumps({'1': {
                    'members': ['choice', 'decision'], 'partOfSpeech': 'n',
                    'definition': ['A conclusion — café.'], 'example': ['Make a choice.'],
                }}, ensure_ascii=False))
                output.writestr('LICENSE', 'CC-BY-4.0')
            installer.prepare(archive, root / 'prepared', 'wordnet')
            rows = [json.loads(line) for line in (root / 'prepared/dictionary.jsonl').read_text().splitlines()]
            self.assertEqual(len(rows), 2)
            self.assertEqual(rows[0]['senses'][0]['synonyms'], [{'word': 'decision'}])
            self.assertEqual(rows[0]['senses'][0]['glosses'], ['A conclusion — café.'])
            self.assertTrue((root / 'prepared/source/LICENSE').is_file())

    def test_gcide_keeps_distinct_definitions_and_licence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with zipfile.ZipFile(root / 'source.zip', 'w') as archive:
                archive.writestr('gcide-0.52/COPYING', 'GNU GPL version 3')
                archive.writestr('gcide-0.52/CIDE.A', '<p><ent>Book</ent><hw>Book</hw><pos>n.</pos><def>A <i>volume</i> &amp; pages.</def></p><p><def>A record.</def></p><p><ent>Book</ent><pos>v.</pos><def>To reserve a reader’s copy.</def></p>'.encode('cp1252'))
            installer.prepare(root / 'source.zip', root / 'prepared', 'gcide')
            rows = [json.loads(line) for line in (root / 'prepared/dictionary.jsonl').read_text().splitlines()]
            self.assertEqual([r['word'] for r in rows], ['Book', 'Book'])
            self.assertEqual(rows[0]['senses'], [{'glosses': ['A volume & pages.']}, {'glosses': ['A record.']}])
            self.assertEqual(rows[0]['pos'], 'noun')
            self.assertEqual(rows[1]['pos'], 'verb')
            self.assertEqual(rows[1]['senses'][0]['glosses'], ['To reserve a reader’s copy.'])
            self.assertTrue((root / 'prepared/source/gcide-0.52/COPYING').is_file())

    def test_princeton_synsets_preserve_definitions_synonyms_and_examples(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with tarfile.open(root / 'source.tar.gz', 'w:gz') as archive:
                for name, data in {
                    'WordNet-3.0/LICENSE': 'Princeton University licence',
                    'WordNet-3.0/dict/data.noun': '  licence header\n00000001 03 n 02 book 0 written_work 1 000 | a volume; "read a book"\n',
                }.items():
                    content = data.encode()
                    member = tarfile.TarInfo(name)
                    member.size = len(content)
                    archive.addfile(member, io.BytesIO(content))
            installer.prepare(root / 'source.tar.gz', root / 'prepared', 'princeton')
            rows = [json.loads(line) for line in (root / 'prepared/dictionary.jsonl').read_text().splitlines()]
            self.assertEqual([r['word'] for r in rows], ['book', 'written work'])
            self.assertEqual(rows[0]['senses'][0], {'glosses': ['a volume'], 'examples': [{'text': 'read a book'}], 'synonyms': [{'word': 'written work'}]})
            self.assertTrue((root / 'prepared/source/WordNet-3.0/LICENSE').is_file())
            with self.assertRaises(ValueError):
                list(installer.princeton_entries(['malformed record']))

    def test_tar_paths_links_and_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, kind in [('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE), ('link', tarfile.SYMTYPE), ('hard', tarfile.LNKTYPE)]:
                with tarfile.open(root / 'bad.tar', 'w') as archive:
                    member = tarfile.TarInfo(name)
                    member.type = kind
                    member.linkname = '../escape'
                    archive.addfile(member)
                with self.assertRaises(ValueError):
                    installer.unpack_tar(root / 'bad.tar', root / 'prepared')
            with tarfile.open(root / 'bad.tar', 'w') as archive:
                member = tarfile.TarInfo('large')
                member.size = 3
                archive.addfile(member, io.BytesIO(b'123'))
            with patch.object(installer, 'MAX_EXTRACTED', 1):
                with self.assertRaises(ValueError):
                    installer.unpack_tar(root / 'bad.tar', root / 'prepared')

    def test_wiktionary_cleanup_preserves_excluded_records(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'source.gz'
            with gzip.open(archive, 'wt') as output:
                for word in ['book', '', 'b' * 4097]:
                    output.write(json.dumps({'word': word, 'lang_code': 'en'}) + '\n')
            self.assertEqual(installer.prepare(archive, root, 'wiktionary-en'), 2)
            self.assertEqual(json.loads((root / 'dictionary.jsonl').read_text())['word'], 'book')
            self.assertEqual(len((root / 'excluded.jsonl').read_text().splitlines()), 2)
            with patch.object(installer, 'MAX_LINE', 3):
                with self.assertRaises(ValueError):
                    list(installer.bounded_lines(io.BytesIO(b'12345\n')))

    def test_failed_download_does_not_publish_a_cache_and_can_retry(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            with patch.object(installer, 'download', side_effect=OSError('offline')):
                with self.assertRaises(OSError):
                    installer.install('arramooz', cache)
            self.assertEqual(list(cache.iterdir()), [])
            def fixture_download(url, target):
                with zipfile.ZipFile(target, 'w') as output:
                    output.writestr('arramooz.ifo', 'fixture')
                return 'fixture-hash'
            with patch.object(installer, 'download', side_effect=fixture_download) as download:
                first = installer.install('arramooz', cache)
                self.assertEqual(installer.install('arramooz', cache), first)
                self.assertEqual(download.call_count, 1)
            self.assertTrue(first.is_file())

    def test_downloads_reject_unencrypted_urls_and_redirects(self):
        with self.assertRaises(ValueError):
            installer.download('http://example.invalid/dictionary', Path('unused'))
        with self.assertRaises(ValueError):
            installer.HTTPSRedirect().redirect_request(None, None, 302, '', {}, 'http://example.invalid')


class CleaningTest(unittest.TestCase):
    def test_whitespace_headword_is_logged_without_changing_valid_record(self):
        valid = '{"word":"كتاب","senses":[{"glosses":["book"]}]}\n'
        output, rejected = io.StringIO(), io.StringIO()
        self.assertEqual(clean(io.StringIO(valid + '{"word":" "}\n'), output, rejected), (1, 1))
        self.assertEqual(output.getvalue(), valid)
        self.assertEqual(json.loads(rejected.getvalue())['line'], 2)

    def test_malformed_json_is_an_error(self):
        with self.assertRaises(json.JSONDecodeError):
            clean(io.StringIO('{broken'), io.StringIO(), io.StringIO())


class ConversionTest(unittest.TestCase):
    def test_preserves_distinct_senses_and_only_attested_synonyms(self):
        result = list(entries({
            '1-n': {'members': ['bank', 'depository'], 'partOfSpeech': 'n',
                    'definition': ['a financial institution'], 'example': ['a savings bank']},
            '2-n': {'members': ['bank'], 'partOfSpeech': 'n',
                    'definition': ['the land beside a river']},
        }))
        bank = [row for row in result if row['word'] == 'bank']
        self.assertEqual(len(bank), 2)
        self.assertEqual(bank[0]['senses'][0]['synonyms'], [{'word': 'depository'}])
        self.assertEqual(bank[1]['senses'][0]['synonyms'], [])
        self.assertEqual(bank[0]['senses'][0]['examples'], [{'text': 'a savings bank'}])
        self.assertNotIn('derived', bank[0]['senses'][0])


if __name__ == '__main__':
    unittest.main()
