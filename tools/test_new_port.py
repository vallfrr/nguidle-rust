"""The port template generates a complete, consistent repository (no network, no compiler)."""

import tempfile
import unittest
from pathlib import Path

import new_port


class NewPortTests(unittest.TestCase):
    def make(self, **kw):
        directory = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(directory, ignore_errors=True))
        args = dict(name="nguidle-test", out=directory / "port", platform="a test platform", engine_path=None,
                    tag="v0.5.0", revision="a" * 40, url="https://example.invalid/nguidle-rust.git")
        args.update(kw)
        return new_port.create(args["name"], args["out"], args["platform"], args["engine_path"], args["tag"], args["revision"], args["url"])

    def test_every_placeholder_is_replaced_and_files_are_complete(self):
        out = self.make()
        for path in out.rglob("*"):
            if path.is_file():
                self.assertNotIn("__PORT_", path.read_text(encoding="utf-8", errors="ignore"), path)
                self.assertNotIn("__ENGINE_", path.read_text(encoding="utf-8", errors="ignore"), path)
        for expected in ["Cargo.toml", "AGENTS.md", "README.md", "justfile", "engine.tag", "src/main.rs", "src/lib.rs",
                         "tests/contract.rs", ".github/workflows/check.yml", ".github/repo-check.py", "LICENSE", ".gitignore"]:
            self.assertTrue((out / expected).is_file(), expected)
        self.assertEqual((out / "engine.tag").read_text().split(), ["v0.5.0"])
        self.assertIn('tag = "v0.5.0"', (out / "Cargo.toml").read_text())
        self.assertIn("nguidle_test::", (out / "src/main.rs").read_text())

    def test_a_local_engine_checkout_replaces_the_git_dependency(self):
        out = self.make(engine_path="../nguidle-rust")
        self.assertIn('clr = { path = "../nguidle-rust/clr" }', (out / "Cargo.toml").read_text())

    def test_bad_names_and_non_empty_folders_are_refused_with_advice(self):
        with self.assertRaisesRegex(SystemExit, "nguidle- followed by lowercase words"):
            self.make(name="Bad_Name")
        out = self.make()
        with self.assertRaisesRegex(SystemExit, "not empty"):
            self.make(out=out)


if __name__ == "__main__":
    unittest.main()
