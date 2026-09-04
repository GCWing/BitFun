from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(SKILL_ROOT / "scripts"))

from build_support import atomic_write_json  # noqa: E402
from cinematic_recipe import (  # noqa: E402
    DEFAULT_PALETTE,
    DEFAULT_SURFACE_PLAN,
    CinematicContractError,
    build_manifest,
    canonical_json,
    load_palette,
    load_surface_plan,
    scaffold_comparable_manifest,
    validate_manifest,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate a cinematic OpenBitFun Appearance manifest.")
    parser.add_argument("--output", type=Path, required=True, help="Skin root containing package/.")
    parser.add_argument("--id", required=True, dest="appearance_id")
    parser.add_argument("--name", required=True)
    parser.add_argument("--version", default="1.0.0")
    parser.add_argument("--mode", choices=("light", "dark"), default="dark")
    parser.add_argument("--author")
    parser.add_argument("--description")
    parser.add_argument("--palette", type=Path, default=DEFAULT_PALETTE)
    parser.add_argument("--surface-plan", type=Path, default=DEFAULT_SURFACE_PLAN)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--check", action="store_true", help="Check that the existing manifest matches generated output.")
    return parser.parse_args()


def generate(args: argparse.Namespace) -> tuple[Path, dict[str, object], list[dict[str, str]]]:
    palette = load_palette(args.palette.resolve())
    surface_plan = load_surface_plan(args.surface_plan.resolve())
    manifest = build_manifest(
        appearance_id=args.appearance_id,
        name=args.name,
        version=args.version,
        mode=args.mode,
        author=args.author,
        description=args.description,
        palette=palette,
        surface_plan=surface_plan,
    )
    warnings = validate_manifest(manifest)
    manifest_path = args.output.resolve() / "package" / "appearance.json"
    return manifest_path, manifest, warnings


def main() -> None:
    args = parse_args()
    try:
        manifest_path, manifest, warnings = generate(args)
        if args.check:
            if not manifest_path.is_file():
                raise CinematicContractError(f"Manifest does not exist: {manifest_path}")
            existing = json.loads(manifest_path.read_text(encoding="utf-8"))
            if canonical_json(scaffold_comparable_manifest(existing)) != canonical_json(
                scaffold_comparable_manifest(manifest)
            ):
                raise CinematicContractError(f"Manifest drift detected: {manifest_path}")
            action = "checked"
        else:
            if manifest_path.exists() and not args.force:
                raise CinematicContractError(f"Refusing to overwrite existing manifest: {manifest_path}")
            atomic_write_json(manifest_path, manifest)
            action = "written"
        print(json.dumps({
            "manifest": str(manifest_path),
            "action": action,
            "components": len(manifest["components"]),
            "scenes": len(manifest["scenes"]),
            "warnings": warnings,
        }, indent=2))
    except (CinematicContractError, OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
