#!/usr/bin/env python3
"""Export the production BitFun Appearance registry into this standalone skill."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


DEFAULT_OUTPUT = Path(__file__).resolve().parent.parent / "references" / "appearance-registry.json"
CONTRACT_KEYS = (
    "components",
    "scenes",
    "renderers",
    "defaultForceableProperties",
    "themeTokenNames",
    "scopedThemeTokenNames",
    "themeScopeIds",
    "widgetVariableNames",
)
EXPORT_MARKER = "__BITFUN_APPEARANCE_REGISTRY__"


class SyncError(Exception):
    pass


def contract_view(value: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": value.get("schema"),
        "schemaVersion": value.get("schemaVersion"),
        **{key: value.get(key) for key in CONTRACT_KEYS},
    }


def run(command: list[str], cwd: Path) -> str:
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace", check=False)
    if result.returncode != 0:
        raise SyncError(result.stderr.strip() or result.stdout.strip() or f"Command failed: {' '.join(command)}")
    return result.stdout.strip()


def export_registry(repo: Path) -> dict[str, Any]:
    web_ui = repo / "src" / "web-ui"
    if not (web_ui / "src" / "infrastructure" / "appearance" / "registry" / "defaultAppearanceRegistry.ts").is_file():
        raise SyncError(f"Not a BitFun checkout with the Appearance registry: {repo}")
    source = r"""
import { createServer } from 'vite';
const server = await createServer({ root: process.cwd(), logLevel: 'silent', appType: 'custom', server: { middlewareMode: true } });
try {
  const registryModule = await server.ssrLoadModule('/src/infrastructure/appearance/registry/defaultAppearanceRegistry.ts');
  const tokenContract = await server.ssrLoadModule('/src/infrastructure/appearance/appearanceTokenContract.ts');
  const widget = await server.ssrLoadModule('/src/infrastructure/appearance/adapters/widgetAppearanceVariables.ts');
  const profiles = await server.ssrLoadModule('/src/infrastructure/appearance/appearancePropertyProfiles.ts');
  const registry = registryModule.createDefaultAppearanceRegistry();
  const defaultForceableProperties = [...profiles.getDefaultForceableAppearanceProperties()];
  const differsFromDefaultForceable = values => values
    && (values.length !== defaultForceableProperties.length
      || values.some((value, index) => value !== defaultForceableProperties[index]));
  const descriptor = item => ({
    id: item.id,
    parts: item.parts.map(part => ({
      id: part.id,
      ...(part.allowedProperties ? { allowedProperties: [...part.allowedProperties] } : {}),
      ...(part.propertyProfile ? { propertyProfile: part.propertyProfile } : {}),
      ...(differsFromDefaultForceable(part.forceableProperties) ? { forceableProperties: [...part.forceableProperties] } : {}),
      ...(part.visualRole ? { visualRole: part.visualRole } : {}),
      ...(part.continuityGroup ? { continuityGroup: part.continuityGroup } : {}),
    })),
    facets: (item.facets ?? []).map(facet => ({ id: facet.id, attribute: facet.attribute, values: [...facet.values] })),
    states: (item.states ?? []).map(state => ({ id: state.id, selector: state.selector })),
  });
  process.stdout.write('__BITFUN_APPEARANCE_REGISTRY__' + JSON.stringify({
    components: registry.getComponents().map(descriptor),
    scenes: registry.getScenes().map(descriptor),
    renderers: registry.getRendererAdapters().map(adapter => adapter.id),
    defaultForceableProperties,
    themeTokenNames: [...tokenContract.APPEARANCE_ROOT_TOKEN_NAMES],
    scopedThemeTokenNames: [...tokenContract.APPEARANCE_SCOPED_TOKEN_NAMES],
    themeScopeIds: Object.keys(tokenContract.APPEARANCE_THEME_SCOPE_SELECTORS),
    widgetVariableNames: [...widget.WIDGET_APPEARANCE_VARIABLE_NAMES],
  }));
} finally {
  await server.close();
}
"""
    raw = run(["node", "--input-type=module", "--eval", source], web_ui)
    marker_index = raw.rfind(EXPORT_MARKER)
    if marker_index < 0:
        raise SyncError(f"Registry exporter did not emit its output marker\n{raw[:500]}")
    raw = raw[marker_index + len(EXPORT_MARKER):]
    try:
        return json.loads(raw)
    except json.JSONDecodeError as error:
        raise SyncError(f"Registry exporter produced invalid JSON: {error}\n{raw[:500]}") from error


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Synchronize the standalone registry from a BitFun checkout")
    parser.add_argument("repo", help="BitFun repository root")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    parser.add_argument("--allow-dirty", action="store_true", help="record and export a dirty working tree")
    parser.add_argument("--check", action="store_true", help="fail when the bundled registry differs from the checkout")
    return parser


def main() -> int:
    args = create_parser().parse_args()
    try:
        repo = Path(args.repo).resolve()
        revision = run(["git", "rev-parse", "HEAD"], repo)
        status = run(["git", "status", "--porcelain=v1"], repo)
        dirty = bool(status)
        if dirty and not args.allow_dirty:
            raise SyncError("BitFun checkout is dirty; commit changes or pass --allow-dirty so provenance records the state")
        tree_evidence = run(["git", "diff", "--no-ext-diff", "HEAD"], repo) + "\n" + status
        snapshot = export_registry(repo)
        output_value = {
            "schema": "bitfun.appearance.registry",
            "schemaVersion": 2,
            "generatedFrom": revision,
            "generatedAt": dt.datetime.now(dt.timezone.utc).isoformat(),
            "sourceRevision": revision,
            "sourceDirty": dirty,
            "sourceTreeHash": hashlib.sha256(tree_evidence.encode("utf-8")).hexdigest(),
            **snapshot,
        }
        output = Path(args.output).resolve()
        if args.check:
            if not output.is_file():
                raise SyncError(f"Bundled registry is missing: {output}")
            try:
                current = json.loads(output.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as error:
                raise SyncError(f"Could not read bundled registry: {error}") from error
            if contract_view(current) != contract_view(output_value):
                raise SyncError(
                    "Bundled registry differs from the selected BitFun checkout; "
                    "run sync_registry.py without --check to refresh it"
                )
            print(json.dumps({
                "output": str(output),
                "snapshotRevision": current.get("sourceRevision"),
                "checkoutRevision": revision,
                "dirty": dirty,
                "synchronized": True,
            }, indent=2))
            return 0
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(output_value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({
            "output": str(output),
            "revision": revision,
            "dirty": dirty,
            "components": len(snapshot["components"]),
            "scenes": len(snapshot["scenes"]),
            "renderers": len(snapshot["renderers"]),
        }, indent=2))
        return 0
    except (SyncError, OSError, ValueError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
