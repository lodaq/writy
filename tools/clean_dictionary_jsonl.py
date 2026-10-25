#!/usr/bin/env python3
"""Copy JSONL records with importable headwords; record exclusions separately.

Does not overwrite the original export. Malformed JSON is an error, not skipped.
The rejected-record log retains line numbers and original records for review.
"""
import argparse
import json


def clean(source, output, rejected):
    kept = skipped = 0
    for number, line in enumerate(source, 1):
        value = json.loads(line)
        word = value.get('word')
        if not isinstance(word, str) or not word.strip() or len(word.encode('utf-8')) > 4096:
            rejected.write(json.dumps({'line': number, 'reason': 'invalid headword',
                                       'record': value}, ensure_ascii=False) + '\n')
            skipped += 1
        else:
            output.write(line)
            kept += 1
    return kept, skipped


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source')
    parser.add_argument('output')
    parser.add_argument('rejected')
    args = parser.parse_args()
    with open(args.source, encoding='utf-8') as source, \
            open(args.output, 'x', encoding='utf-8') as output, \
            open(args.rejected, 'x', encoding='utf-8') as rejected:
        kept, skipped = clean(source, output, rejected)
    print(f'Kept {kept} records; excluded {skipped} (see {args.rejected})')


if __name__ == '__main__':
    main()
