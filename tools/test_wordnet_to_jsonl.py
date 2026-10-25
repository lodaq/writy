import unittest
from wordnet_to_jsonl import entries


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
