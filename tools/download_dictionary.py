#!/usr/bin/env python3
"""Prepare publisher dictionary downloads for Writy's existing importers."""
import argparse
import gzip
import hashlib
import html
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tempfile
import tarfile
import urllib.request
import zipfile

from clean_dictionary_jsonl import clean
from wordnet_to_jsonl import entries

SOURCES = {
    'gcide': ('https://ftp.gnu.org/gnu/gcide/gcide-0.52.zip', 'dictionary.jsonl'),
    'princeton': ('https://wordnetcode.princeton.edu/3.0/WordNet-3.0.tar.gz', 'dictionary.jsonl'),
    'arramooz': ('https://downloads.sourceforge.net/project/arramooz/arramooz-stardict.0.1.zip', 'arramooz.ifo'),
    'wordnet': ('https://en-word.net/downloads/english-wordnet-2025-json.zip', 'dictionary.jsonl'),
    'wiktionary-en': ('https://kaikki.org/dictionary/English/kaikki.org-dictionary-English.jsonl.gz', 'dictionary.jsonl'),
    'wiktionary-ar': ('https://kaikki.org/dictionary/Arabic/kaikki.org-dictionary-Arabic.jsonl.gz', 'dictionary.jsonl'),
}
MAX_DOWNLOAD = 1024 ** 3
MAX_EXTRACTED = 8 * 1024 ** 3
MAX_LINE = 2 * 1024 ** 2


class HTTPSRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, newurl):
        if not newurl.startswith('https://'):
            raise ValueError('Dictionary download redirected away from HTTPS')
        return super().redirect_request(request, file, code, message, headers, newurl)


def download(url, target):
    if not url.startswith('https://'):
        raise ValueError('Dictionary downloads require HTTPS')
    opener = urllib.request.build_opener(HTTPSRedirect)
    request = urllib.request.Request(url, headers={'User-Agent': 'Writy/0.1 dictionary installer'})
    digest = hashlib.sha256()
    total = 0
    with opener.open(request, timeout=60) as response, target.open('xb') as output:
        if int(response.headers.get('Content-Length', 0)) > MAX_DOWNLOAD:
            raise ValueError('Dictionary download exceeds the size limit')
        while chunk := response.read(1024 * 1024):
            total += len(chunk)
            if total > MAX_DOWNLOAD:
                raise ValueError('Dictionary download exceeds the size limit')
            digest.update(chunk)
            output.write(chunk)
    if not total:
        raise ValueError('The publisher returned an empty download')
    return digest.hexdigest()


def unpack_zip(archive_path, directory):
    total = 0
    with zipfile.ZipFile(archive_path) as archive:
        if len(archive.infolist()) > 1024:
            raise ValueError('Dictionary archive contains too many files')
        for member in archive.infolist():
            path = PurePosixPath(member.filename)
            mode = member.external_attr >> 16
            if (not path.parts or path.is_absolute() or '..' in path.parts
                    or '\\' in member.filename or len(path.parts) > 16
                    or len(member.filename) > 1024 or stat.S_ISLNK(mode)
                    or (stat.S_IFMT(mode) and not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)))):
                raise ValueError('Dictionary archive contains an unsafe path or file type')
            target = directory.joinpath(*path.parts)
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            total += member.file_size
            if member.file_size > 512 * 1024 ** 2 or total > MAX_EXTRACTED:
                raise ValueError('Dictionary archive exceeds the extraction limit')
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, target.open('xb') as output:
                shutil.copyfileobj(source, output, 1024 * 1024)


def unpack_tar(archive_path, directory):
    total = 0
    with tarfile.open(archive_path) as archive:
        for count, member in enumerate(archive, 1):
            path = PurePosixPath(member.name)
            if (count > 1024 or not path.parts or path.is_absolute()
                    or '..' in path.parts or '\\' in member.name
                    or len(path.parts) > 16 or len(member.name) > 1024
                    or not (member.isdir() or member.isfile())):
                raise ValueError('Dictionary archive contains an unsafe path or file type')
            target = directory.joinpath(*path.parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            total += member.size
            if member.size < 0 or member.size > 512 * 1024 ** 2 or total > MAX_EXTRACTED:
                raise ValueError('Dictionary archive exceeds the extraction limit')
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('xb') as output:
                shutil.copyfileobj(source, output, 1024 * 1024)


def plain_text(text):
    return ' '.join(html.unescape(re.sub(r'<[^>]*>', '', text)).split())


def gcide_entries(text):
    parts = {'n.': 'noun', 'v.': 'verb', 'v. t.': 'verb', 'v. i.': 'verb',
             'a.': 'adj', 'adj.': 'adj', 'adv.': 'adv'}
    for entry in re.finditer(r'<(ent|h1)>(.*?)</\1>(.*?)(?=<(?:ent|h1)>|\Z)', text, re.S):
        word = plain_text(entry[2])
        body = entry[3]
        definitions = [plain_text(value) for value in re.findall(r'<def>(.*?)</def>', body, re.S)]
        pos = re.search(r'<pos>(.*?)</pos>', body, re.S)
        if word and definitions:
            yield {'word': word, 'lang_code': 'en',
                   'pos': parts.get(plain_text(pos[1]), plain_text(pos[1])) if pos else '',
                   'senses': [{'glosses': [definition]} for definition in definitions if definition]}


def princeton_entries(lines):
    parts = {'n': 'noun', 'v': 'verb', 'a': 'adj', 's': 'adj', 'r': 'adv'}
    for line in lines:
        if not line.strip() or line.startswith('  '):
            continue
        fields, separator, gloss = line.partition('|')
        tokens = fields.split()
        try:
            count = int(tokens[3], 16)
            pos = parts[tokens[2]]
            words = [re.sub(r'\((?:a|p|ip)\)$', '', tokens[4 + 2 * i]).replace('_', ' ') for i in range(count)]
            pointers = int(tokens[4 + 2 * count])
            if not separator or not tokens[0].isdigit() or not count or len(tokens) < 5 + 2 * count + 4 * pointers:
                raise ValueError('Incomplete WordNet record')
        except (IndexError, KeyError, ValueError) as error:
            raise ValueError('Invalid Princeton WordNet record') from error
        examples = re.findall(r'"([^"]+)"', gloss)
        definition = re.sub(r'"[^"]*"', '', gloss).strip(' ;\n')
        for word in words:
            yield {'word': word, 'lang_code': 'en', 'pos': pos,
                   'senses': [{'glosses': [definition],
                               'examples': [{'text': example} for example in examples],
                               'synonyms': [{'word': other} for other in words if other != word]}]}


def bounded_lines(source):
    total = 0
    while line := source.readline(MAX_LINE + 1):
        total += len(line)
        if len(line) > MAX_LINE or total > MAX_EXTRACTED:
            raise ValueError('Dictionary data exceeds its line or extraction limit')
        yield line.decode('utf-8')


def prepare(archive, directory, pack):
    if pack == 'arramooz':
        unpack_zip(archive, directory)
        if not (directory / 'arramooz.ifo').is_file():
            raise ValueError('Arramooz archive is missing arramooz.ifo')
    elif pack in ('gcide', 'princeton'):
        extracted = directory / 'source'
        if pack == 'gcide':
            unpack_zip(archive, extracted)
            paths = sorted(extracted.glob('gcide-*/CIDE.[A-Z]'))
            licence = list(extracted.glob('gcide-*/COPYING'))
        else:
            unpack_tar(archive, extracted)
            paths = sorted(extracted.glob('WordNet-*/dict/data.*'))
            licence = list(extracted.glob('WordNet-*/LICENSE'))
        if not licence or not paths:
            raise ValueError('Dictionary archive is missing its data or licence')
        count = 0
        with (directory / 'dictionary.jsonl').open('x', encoding='utf-8') as output:
            for path in paths:
                with path.open('rb') as source:
                    if pack == 'gcide':
                        records = gcide_entries(source.read().decode('cp1252'))
                    else:
                        records = princeton_entries(bounded_lines(source))
                    for record in records:
                        output.write(json.dumps(record, ensure_ascii=False) + '\n')
                        count += 1
        if not count:
            raise ValueError('Dictionary archive contains no usable entries')
    elif pack == 'wordnet':
        extracted = directory / 'source'
        unpack_zip(archive, extracted)
        count = 0
        with (directory / 'dictionary.jsonl').open('x', encoding='utf-8') as output:
            for path in sorted(extracted.iterdir()):
                if path.suffix != '.json' or not path.name.startswith(('adj.', 'adv.', 'noun.', 'verb.')):
                    continue
                for entry in entries(json.loads(path.read_text(encoding='utf-8'))):
                    output.write(json.dumps(entry, ensure_ascii=False) + '\n')
                    count += 1
        if not count:
            raise ValueError('WordNet archive contains no supported word senses')
    else:
        with gzip.open(archive, 'rb') as source, \
                (directory / 'dictionary.jsonl').open('x', encoding='utf-8') as output, \
                (directory / 'excluded.jsonl').open('x', encoding='utf-8') as rejected:
            kept, excluded = clean(bounded_lines(source), output, rejected)
        if not kept:
            raise ValueError('Wiktionary download contains no usable entries')
        return excluded
    return 0


def install(pack, cache):
    url, filename = SOURCES[pack]
    destination = cache / pack
    manifest = destination / 'ready.json'
    if manifest.is_file():
        if (destination / filename).is_file():
            return destination / filename
        raise ValueError('Cached dictionary is incomplete; import a fresh copy from Settings')
    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.dictionary-', dir=cache) as temporary:
        stage = Path(temporary)
        archive = stage / 'original-download'
        checksum = download(url, archive)
        excluded = prepare(archive, stage, pack)
        (stage / 'ready.json').write_text(json.dumps({
            'source': url, 'sha256': checksum, 'excluded_records': excluded,
        }), encoding='utf-8')
        if destination.exists():
            if not manifest.is_file() or not (destination / filename).is_file():
                raise ValueError('Dictionary cache already exists but is incomplete')
        else:
            stage.rename(destination)
    return destination / filename


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pack', choices=SOURCES)
    parser.add_argument('cache', type=Path)
    args = parser.parse_args()
    try:
        print(install(args.pack, args.cache))
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(f'Dictionary download failed: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
