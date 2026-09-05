#!/usr/bin/env python3
"""Validate and compile an Appearance manifest with a real OpenBitFun checkout."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any, Sequence

import openbitfun_appearance as appearance
import media_support
import sync_registry as registry_sync


class HostVerificationError(Exception):
    pass


def run(command: list[str], cwd: Path) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise HostVerificationError(
            result.stderr.strip() or result.stdout.strip() or f"Command failed: {' '.join(command)}"
        )
    return result.stdout.strip()


def verify_video_assets(source: Path, manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    video_assets = {
        asset_id: definition
        for asset_id, definition in (manifest.get("assets") or {}).items()
        if definition.get("kind") == "video"
    }
    if not video_assets:
        return {}

    kind = appearance.source_kind(source)
    results: dict[str, dict[str, Any]] = {}
    try:
        with tempfile.TemporaryDirectory(prefix="openbitfun-host-media-") as temporary:
            root = Path(temporary)
            archive = zipfile.ZipFile(source) if kind == "archive" else None
            try:
                for asset_id, definition in video_assets.items():
                    relative_path = definition["source"]["path"]
                    if archive is None:
                        video_path = source / relative_path
                    else:
                        video_path = root / f"{asset_id}{Path(relative_path).suffix}"
                        video_path.write_bytes(archive.read(relative_path))
                    metadata = media_support.probe_video(video_path)
                    media_support.validate_host_video(metadata, codec=None)
                    results[asset_id] = {"path": relative_path, **metadata}
            finally:
                if archive is not None:
                    archive.close()
    except (
        KeyError,
        OSError,
        RuntimeError,
        ValueError,
        subprocess.CalledProcessError,
        zipfile.BadZipFile,
    ) as error:
        raise HostVerificationError(f"Appearance video validation failed: {error}") from error
    return results


def verify_host(repo: Path, source: Path) -> dict[str, Any]:
    web_ui = repo / "src" / "web-ui"
    registry_module = web_ui / "src" / "infrastructure" / "appearance" / "registry" / "defaultAppearanceRegistry.ts"
    if not registry_module.is_file():
        raise HostVerificationError(f"Not a OpenBitFun checkout with the Appearance registry: {repo}")

    registry = registry_sync.export_registry(repo)
    kind = appearance.source_kind(source)
    standalone_warnings: list[dict[str, str]] = []
    manifest = (
        appearance.validate_project(source, registry, standalone_warnings)
        if kind == "project"
        else appearance.validate_archive(source, registry, standalone_warnings)
    )
    video_assets = verify_video_assets(source, manifest)
    node_source = r"""
import fs from 'node:fs';
import { createServer } from 'vite';
const manifestPath = process.argv[1];
const server = await createServer({ root: process.cwd(), logLevel: 'silent', appType: 'custom', server: { middlewareMode: true } });
try {
  const registryModule = await server.ssrLoadModule('/src/infrastructure/appearance/registry/defaultAppearanceRegistry.ts');
  const validatorModule = await server.ssrLoadModule('/src/infrastructure/appearance/schema/AppearancePackageValidator.ts');
  const compilerModule = await server.ssrLoadModule('/src/infrastructure/appearance/compiler/AppearanceCompiler.ts');
  const input = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const registry = registryModule.createDefaultAppearanceRegistry();
  const validation = validatorModule.appearancePackageValidator.validate(input, registry);
  const output = {
    valid: validation.valid,
    errors: validation.errors,
    warnings: validation.warnings,
    diagnostics: [],
    cssBytes: 0,
  };
  if (validation.valid) {
    const snapshot = new compilerModule.AppearanceCompiler(registry).compile(input, 1);
    output.diagnostics = snapshot.diagnostics;
    output.cssBytes = Buffer.byteLength(snapshot.cssText, 'utf8');
  }
  process.stdout.write(JSON.stringify(output));
} finally {
  await server.close();
}
"""
    with tempfile.TemporaryDirectory(prefix="openbitfun-host-appearance-") as temporary:
        manifest_path = Path(temporary) / appearance.MANIFEST_NAME
        manifest_path.write_bytes(appearance.manifest_bytes(manifest))
        raw = run(["node", "--input-type=module", "--eval", node_source, str(manifest_path)], web_ui)
    try:
        result = json.loads(raw)
    except json.JSONDecodeError as error:
        raise HostVerificationError(f"OpenBitFun host verifier produced invalid JSON: {error}\n{raw[:500]}") from error

    revision = run(["git", "rev-parse", "HEAD"], repo)
    dirty = bool(run(["git", "status", "--porcelain=v1"], repo))
    issues: dict[tuple[str, str, str], dict[str, Any]] = {}
    for item in [*standalone_warnings, *result.get("warnings", []), *result.get("diagnostics", [])]:
        if not isinstance(item, dict):
            continue
        key = (str(item.get("path", "$")), str(item.get("code", "HOST_WARNING")), str(item.get("message", "")))
        issues[key] = item
    return {
        "schema": "openbitfun.appearance.host-verification",
        "schemaVersion": 1,
        "appearanceId": manifest.get("id"),
        "source": str(source),
        "repo": str(repo),
        "sourceRevision": revision,
        "sourceDirty": dirty,
        "valid": bool(result.get("valid")) and not result.get("errors"),
        "errors": result.get("errors", []),
        "warnings": list(issues.values()),
        "cssBytes": result.get("cssBytes", 0),
        "videoAssets": video_assets,
    }


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate and compile an Appearance with the real OpenBitFun host")
    parser.add_argument("repo", help="OpenBitFun repository root")
    parser.add_argument("source", help="Appearance project directory or .openbitfun-appearance archive")
    parser.add_argument("--report", help="write a JSON verification report")
    parser.add_argument("--strict-warnings", action="store_true", help="fail verification when warnings are present")
    parser.add_argument("--allow-warnings", action="store_true", help=argparse.SUPPRESS)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = create_parser().parse_args(argv)
    try:
        result = verify_host(Path(args.repo).resolve(), Path(args.source).resolve())
        if args.report:
            report_path = Path(args.report).resolve()
            report_path.parent.mkdir(parents=True, exist_ok=True)
            report_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result, ensure_ascii=False, indent=2))
        passed = result["valid"] and (not args.strict_warnings or not result["warnings"])
        return 0 if passed else 1
    except (HostVerificationError, appearance.AppearanceError, registry_sync.SyncError, OSError, ValueError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
