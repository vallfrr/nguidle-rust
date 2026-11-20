import hashlib
from pathlib import Path
import tempfile
import unittest

from verify_game_data import verify


class VerifyGameDataTest(unittest.TestCase):
    def test_valid_file_and_extras(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "required").write_bytes(b"original")
            (root / "extra").touch()
            digest = hashlib.sha256(b"original").hexdigest()
            self.assertEqual(verify(root, {"required": digest}), [])

    def test_reports_missing_and_modified_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "modified").touch()
            digest = hashlib.sha256(b"original").hexdigest()
            self.assertEqual(
                verify(root, {"absent": digest, "modified": digest}),
                ["Missing: absent", "Wrong SHA-256: modified"],
            )

    def test_rejects_paths_outside_the_data_directory(self):
        with tempfile.TemporaryDirectory() as temp:
            self.assertEqual(
                verify(Path(temp), {"../outside": ""}),
                ["Path outside the game data: ../outside"],
            )
