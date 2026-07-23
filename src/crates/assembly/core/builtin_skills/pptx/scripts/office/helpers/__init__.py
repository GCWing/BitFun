import os
import posixpath
import re
import stat
import tempfile
import urllib.parse
import zipfile
from pathlib import Path

OOXML_FAMILY = {
    ".docx": "docx",
    ".dotx": "docx",
    ".pptx": "pptx",
    ".potx": "pptx",
    ".xlsx": "xlsx",
    ".xltx": "xlsx",
}

_SCHEME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9+.\-]*:")

SLIDE_REL_TYPE = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"

MAX_ARCHIVE_MEMBERS = 10_000
MAX_ARCHIVE_MEMBER_SIZE = 1 * 1024 * 1024 * 1024
MAX_ARCHIVE_TOTAL_SIZE = 4 * 1024 * 1024 * 1024
MAX_ARCHIVE_COMPRESSION_RATIO = 1_000


def opc_target(target: str, source_part: str, target_mode: str = "") -> str | None:
    if not target:
        return None
    if target_mode.lower() == "external":
        return None
    if _SCHEME_RE.match(target):
        return None

    target = urllib.parse.unquote(target)

    if "\\" in target:
        raise ValueError(f"relationship target is not a POSIX part name: {target!r}")

    if target.startswith("/"):
        joined = target.lstrip("/")
    else:
        joined = posixpath.join(posixpath.dirname(source_part), target)

    parts: list[str] = []
    for segment in posixpath.normpath(joined).split("/"):
        if segment in ("", "."):
            continue
        if segment == "..":
            if not parts:
                raise ValueError(f"relationship target escapes the package: {target!r}")
            parts.pop()
        else:
            parts.append(segment)

    if not parts:
        raise ValueError(f"relationship target resolves to nothing: {target!r}")
    return "/".join(parts)


def rels_source_part(rels_file: Path, unpacked_dir: Path) -> str:
    owner_dir = rels_file.parent.parent.relative_to(unpacked_dir)
    return posixpath.join(owner_dir.as_posix(), rels_file.name[: -len(".rels")]).lstrip("./")


def part_text(data: bytes) -> str:
    return data.decode("utf-8", "surrogateescape")


XML_SPACE = " \t\r\n"


def rendered_text(text: str, preserve: bool) -> str:
    return text if preserve else text.strip(XML_SPACE)


def safe_extract(zf: zipfile.ZipFile, dest: Path) -> None:
    dest = dest.resolve()
    members = zf.infolist()
    if len(members) > MAX_ARCHIVE_MEMBERS:
        raise ValueError(f"archive has too many entries: {len(members)}")

    total_size = 0
    targets: set[str] = set()
    file_targets: set[str] = set()
    validated: list[tuple[zipfile.ZipInfo, Path]] = []
    for m in members:
        if stat.S_ISLNK(m.external_attr >> 16):
            raise ValueError(f"symlink archive entry not allowed: {m.filename!r}")
        target = (dest / m.filename).resolve()
        if target == dest or not target.is_relative_to(dest):
            raise ValueError(f"unsafe archive entry: {m.filename!r}")
        target_key = os.path.normcase(str(target))
        if target_key in targets:
            raise ValueError(f"duplicate archive entry: {m.filename!r}")
        targets.add(target_key)
        if not m.is_dir():
            file_targets.add(target_key)
        validated.append((m, target))
        if m.file_size > MAX_ARCHIVE_MEMBER_SIZE:
            raise ValueError(f"archive entry is too large: {m.filename!r}")
        total_size += m.file_size
        if total_size > MAX_ARCHIVE_TOTAL_SIZE:
            raise ValueError("archive expands beyond the allowed total size")
        if m.file_size and (
            m.compress_size == 0
            or m.file_size > m.compress_size * MAX_ARCHIVE_COMPRESSION_RATIO
        ):
            raise ValueError(f"archive entry has an unsafe compression ratio: {m.filename!r}")

    for m, target in validated:
        for parent in target.parents:
            if parent == dest:
                break
            if os.path.normcase(str(parent)) in file_targets:
                raise ValueError(f"archive file entry conflicts with child path: {m.filename!r}")

    for m, _ in validated:
        zf.extract(m, dest)


def rezip(src_dir: Path, out_path: Path) -> None:
    files = sorted(p for p in src_dir.rglob("*") if p.is_file())
    ct = src_dir / "[Content_Types].xml"
    fd, tmp_name = tempfile.mkstemp(
        prefix=out_path.name + ".", suffix=".tmp", dir=out_path.parent
    )
    tmp_out = Path(tmp_name)
    try:
        with os.fdopen(fd, "wb") as fh:
            with zipfile.ZipFile(fh, "w", zipfile.ZIP_DEFLATED) as zf:
                if ct.exists():
                    zf.write(ct, ct.relative_to(src_dir), compress_type=zipfile.ZIP_STORED)
                for f in files:
                    if f == ct:
                        continue
                    zf.write(f, f.relative_to(src_dir))
        if out_path.exists():
            mode = out_path.stat().st_mode & 0o777
        else:
            umask = os.umask(0)
            os.umask(umask)
            mode = 0o666 & ~umask
        os.chmod(tmp_out, mode)
        os.replace(tmp_out, out_path)
    finally:
        if tmp_out.exists():
            tmp_out.unlink()
