import unittest

from compare_saves import compare, same


class CompareSavesTests(unittest.TestCase):
    def test_numbers_use_a_relative_tolerance_and_other_values_must_match(self):
        self.assertTrue(same(("F64", "1000000.0"), ("F64", "1000000.0000001"), 1e-9))
        self.assertFalse(same(("F64", "1000000.0"), ("F64", "1000001.0"), 1e-9))
        self.assertFalse(same(("Bool", "true"), ("Bool", "false"), 1e-9))

    def test_missing_and_ignored_fields(self):
        original = {"root.a": ("I32", "1"), "root.time": ("F64", "5.0")}
        port = {"root.a": ("I32", "1"), "root.b": ("I32", "2"), "root.time": ("F64", "9.0")}
        self.assertEqual(compare(original, port, ignore=["time"]), [("root.b", None, ("I32", "2"))])


if __name__ == "__main__":
    unittest.main()
