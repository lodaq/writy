import io
import json
import unittest
from clean_dictionary_jsonl import clean


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


if __name__ == '__main__':
    unittest.main()
