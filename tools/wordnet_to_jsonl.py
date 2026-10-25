#!/usr/bin/env python3
"""Convert an official OEWN JSON zip to Writy's Kaikki-compatible JSONL.

Keeps synset senses distinct. Synset membership supplies synonyms, never
collocations or word-family claims. Import with the original OEWN attribution.
"""
import argparse
import json
import zipfile


def entries(synsets):
    parts = {'n': 'noun', 'v': 'verb', 'a': 'adj', 's': 'adj', 'r': 'adv'}
    for synset in synsets.values():
        members = synset['members']
        for word in members:
            yield {
                'word': word,
                'lang_code': 'en',
                'pos': parts[synset['partOfSpeech']],
                'senses': [{
                    'glosses': synset.get('definition', []),
                    'examples': [{'text': example} for example in synset.get('example', [])],
                    'synonyms': [{'word': other} for other in members if other != word],
                }],
            }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive')
    parser.add_argument('output')
    args = parser.parse_args()
    count = 0
    with zipfile.ZipFile(args.archive) as archive, open(args.output, 'w') as output:
        for name in sorted(archive.namelist()):
            if not name.endswith('.json') or not name.startswith(('adj.', 'adv.', 'noun.', 'verb.')):
                continue
            for entry in entries(json.loads(archive.read(name))):
                output.write(json.dumps(entry, ensure_ascii=False) + '\n')
                count += 1
    print(f'Converted {count} word senses')


if __name__ == '__main__':
    main()
