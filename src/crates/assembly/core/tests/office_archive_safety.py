import importlib.util
import io
import stat
import sys
import tempfile
import unittest
import warnings
import zipfile
from pathlib import Path


sys.dont_write_bytecode = True


CORE_ROOT = Path(__file__).resolve().parents[1]
SKILLS_ROOT = CORE_ROOT / "builtin_skills"


def load_helpers(skill: str):
    path = SKILLS_ROOT / skill / "scripts" / "office" / "helpers" / "__init__.py"
    spec = importlib.util.spec_from_file_location(f"{skill}_office_helpers", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load Office helpers from {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def archive_bytes(entries, compression=zipfile.ZIP_STORED):
    data = io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", UserWarning)
        with zipfile.ZipFile(data, "w", compression=compression) as archive:
            for name, content in entries:
                archive.writestr(name, content)
    data.seek(0)
    return data


class OfficeArchiveSafetyTests(unittest.TestCase):
    def setUp(self):
        self.helpers = {skill: load_helpers(skill) for skill in ("docx", "pptx", "xlsx")}

    def assert_rejected(self, data, message):
        for skill, helpers in self.helpers.items():
            with self.subTest(skill=skill, message=message), tempfile.TemporaryDirectory() as temp:
                data.seek(0)
                with zipfile.ZipFile(data) as archive:
                    with self.assertRaisesRegex(ValueError, message):
                        helpers.safe_extract(archive, Path(temp))

    def test_rejects_traversal_absolute_symlink_and_duplicate_targets(self):
        self.assert_rejected(archive_bytes([("../escape.txt", b"x")]), "unsafe archive entry")
        self.assert_rejected(archive_bytes([("/absolute.txt", b"x")]), "unsafe archive entry")
        self.assert_rejected(archive_bytes([(".", b"x")]), "unsafe archive entry")

        symlink = zipfile.ZipInfo("link")
        symlink.create_system = 3
        symlink.external_attr = (stat.S_IFLNK | 0o777) << 16
        data = io.BytesIO()
        with zipfile.ZipFile(data, "w") as archive:
            archive.writestr(symlink, "target")
        data.seek(0)
        self.assert_rejected(data, "symlink archive entry")

        self.assert_rejected(
            archive_bytes([("duplicate.txt", b"a"), ("./duplicate.txt", b"b")]),
            "duplicate archive entry",
        )
        self.assert_rejected(
            archive_bytes([("file", b"a"), ("file/child", b"b")]),
            "file entry conflicts with child path",
        )

    def test_rejects_member_count_size_total_size_and_compression_ratio_limits(self):
        cases = [
            ("MAX_ARCHIVE_MEMBERS", 1, [("a", b""), ("b", b"")], "too many entries"),
            ("MAX_ARCHIVE_MEMBER_SIZE", 1, [("large", b"xx")], "entry is too large"),
            ("MAX_ARCHIVE_TOTAL_SIZE", 1, [("total", b"xx")], "allowed total size"),
        ]
        for constant, limit, entries, message in cases:
            for skill, helpers in self.helpers.items():
                with self.subTest(skill=skill, constant=constant), tempfile.TemporaryDirectory() as temp:
                    original = getattr(helpers, constant)
                    setattr(helpers, constant, limit)
                    try:
                        with zipfile.ZipFile(archive_bytes(entries)) as archive:
                            with self.assertRaisesRegex(ValueError, message):
                                helpers.safe_extract(archive, Path(temp))
                    finally:
                        setattr(helpers, constant, original)

        for skill, helpers in self.helpers.items():
            with self.subTest(skill=skill, constant="compression_ratio"), tempfile.TemporaryDirectory() as temp:
                original = helpers.MAX_ARCHIVE_COMPRESSION_RATIO
                helpers.MAX_ARCHIVE_COMPRESSION_RATIO = 1
                try:
                    data = archive_bytes([("compressed", b"A" * 4096)], zipfile.ZIP_DEFLATED)
                    with zipfile.ZipFile(data) as archive:
                        with self.assertRaisesRegex(ValueError, "unsafe compression ratio"):
                            helpers.safe_extract(archive, Path(temp))
                finally:
                    helpers.MAX_ARCHIVE_COMPRESSION_RATIO = original

    def test_extracts_valid_archive(self):
        for skill, helpers in self.helpers.items():
            with self.subTest(skill=skill), tempfile.TemporaryDirectory() as temp:
                with zipfile.ZipFile(archive_bytes([("word/document.xml", b"<document/>")])) as archive:
                    helpers.safe_extract(archive, Path(temp))
                self.assertEqual(
                    (Path(temp) / "word" / "document.xml").read_bytes(),
                    b"<document/>",
                )


if __name__ == "__main__":
    unittest.main()
