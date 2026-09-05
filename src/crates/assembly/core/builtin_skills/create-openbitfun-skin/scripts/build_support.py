from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


SKILL_ROOT = Path(__file__).resolve().parents[1]
APPEARANCE_TOOL = SKILL_ROOT / "scripts" / "openbitfun_appearance.py"
HOST_VERIFY = SKILL_ROOT / "scripts" / "verify_host.py"
SYNC_REGISTRY = SKILL_ROOT / "scripts" / "sync_registry.py"
REGISTRY_PATH = SKILL_ROOT / "references" / "appearance-registry.json"
RUNTIME_CHECKLIST_SCHEMA = "openbitfun.appearance.runtime-checklist"


class BuildSupportError(ValueError):
    pass


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BuildSupportError(f"Could not read JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise BuildSupportError(f"JSON root must be an object: {path}")
    return value


def atomic_write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", newline="\n", dir=path.parent, delete=False
    ) as handle:
        handle.write(payload)
        temporary = Path(handle.name)
    os.replace(temporary, path)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(command: Sequence[str], cwd: Path = SKILL_ROOT) -> None:
    subprocess.run(list(command), cwd=cwd, check=True)


def run_json(command: Sequence[str], cwd: Path = SKILL_ROOT) -> dict[str, Any]:
    result = subprocess.run(
        list(command),
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        details = result.stderr.strip() or result.stdout.strip() or "command failed without diagnostic output"
        raise BuildSupportError(
            f"Command failed with exit code {result.returncode}: {' '.join(command)}\n{details}"
        )
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise BuildSupportError(f"Command did not return a JSON object: {' '.join(command)}") from error
    if not isinstance(value, dict):
        raise BuildSupportError(f"Command did not return a JSON object: {' '.join(command)}")
    print(json.dumps(value, ensure_ascii=False, indent=2))
    return value


def copy_input(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.resolve() != destination.resolve():
        shutil.copy2(source, destination)


def initialize_package(
    package: Path,
    *,
    appearance_id: str,
    name: str,
    mode: str,
    force: bool,
) -> None:
    manifest = package / "appearance.json"
    if manifest.exists():
        if not force:
            raise BuildSupportError(f"Manifest already exists; use --force to rebuild: {manifest}")
        return
    if package.exists() and any(package.iterdir()):
        if not force:
            raise BuildSupportError(f"Package directory is non-empty; use --force to recover: {package}")
        return
    run([
        sys.executable,
        str(APPEARANCE_TOOL),
        "init",
        str(package),
        "--id",
        appearance_id,
        "--name",
        name,
        "--mode",
        mode,
    ])


def validate_project(package: Path) -> None:
    run([sys.executable, str(APPEARANCE_TOOL), "validate", str(package)])


def build_archive(package: Path, archive: Path) -> None:
    command = [
        sys.executable,
        str(APPEARANCE_TOOL),
        "build",
        str(package),
        "--output",
        str(archive),
    ]
    if archive.exists():
        command.append("--force")
    run(command)


def validate_archive(archive: Path) -> None:
    run([sys.executable, str(APPEARANCE_TOOL), "validate", str(archive)])


def verify_host(
    openbitfun_repo: Path,
    archive: Path,
    *,
    report: Path | None = None,
    strict_warnings: bool = True,
) -> dict[str, Any] | None:
    run([sys.executable, str(SYNC_REGISTRY), str(openbitfun_repo), "--check"])
    command = [sys.executable, str(HOST_VERIFY), str(openbitfun_repo), str(archive)]
    if report is not None:
        command.extend(["--report", str(report)])
    if strict_warnings:
        command.append("--strict-warnings")
    run(command)
    return read_json(report) if report is not None else None


def registry_provenance() -> dict[str, Any]:
    registry = read_json(REGISTRY_PATH)
    return {
        "sourceRevision": registry.get("sourceRevision"),
        "sourceDirty": registry.get("sourceDirty"),
        "generatedAt": registry.get("generatedAt"),
    }


def ensure_runtime_checklist(
    path: Path,
    *,
    appearance_id: str,
    checks: Iterable[tuple[str, str]],
) -> Path:
    if path.exists():
        return path
    atomic_write_json(path, {
        "schema": RUNTIME_CHECKLIST_SCHEMA,
        "schemaVersion": 1,
        "appearanceId": appearance_id,
        "status": "pending",
        "checks": [
            {"id": check_id, "label": label, "status": "pending", "evidence": []}
            for check_id, label in checks
        ],
    })
    return path


def archive_member_compression(archive: Path, member: str) -> int:
    with zipfile.ZipFile(archive) as bundle:
        return bundle.getinfo(member).compress_type


def check_recorded_file(
    root: Path,
    entry: Mapping[str, Any],
    *,
    label: str,
    issues: list[str],
) -> Path:
    path_value = entry.get("path")
    digest = entry.get("sha256")
    if not isinstance(path_value, str) or not isinstance(digest, str):
        issues.append(f"{label} record is invalid")
        return root
    path = root / path_value
    if not path.is_file() or sha256(path) != digest:
        issues.append(f"{label} hash mismatch")
    return path
