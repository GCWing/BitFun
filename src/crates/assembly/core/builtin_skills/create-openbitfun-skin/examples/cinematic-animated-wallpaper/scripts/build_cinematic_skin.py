from __future__ import annotations

import argparse
import json
import subprocess
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
SKILL_ROOT = SCRIPT_DIR.parents[2]
sys.path.insert(0, str(SKILL_ROOT / "scripts"))

import build_support as build  # noqa: E402
import media_support as media  # noqa: E402
from cinematic_recipe import (  # noqa: E402
    DEFAULT_PALETTE,
    DEFAULT_SURFACE_PLAN,
    CinematicContractError,
    load_palette,
)


ASSET_BUILDER = SCRIPT_DIR / "build_assets.py"
SCAFFOLD = SCRIPT_DIR / "scaffold_cinematic.py"
BUILD_CONFIG_NAME = "skin-build.json"
RUNTIME_CHECKLIST_NAME = "runtime-checklist.json"
BUILD_CONFIG_SCHEMA = "openbitfun.appearance.recipe-build"
RECIPE_ID = "cinematic-animated-wallpaper"


@dataclass
class BuildOptions:
    source: Path
    output: Path
    appearance_id: str
    name: str
    version: str
    mode: str
    author: str | None
    description: str | None
    frame_time: float | None
    frame_index: int
    video_quality: str
    video_crf: int | None
    static_quality_mode: str
    static_quality: int | None
    tint_color: str | None
    floating_crop: str
    floating_size: str
    sidebar_crop: str
    sidebar_size: str
    detail_crop: str
    detail_size: str
    portrait_crop: str
    portrait_size: str
    dialog_crop: str
    dialog_size: str
    palette: Path
    surface_plan: Path
    openbitfun_repo: Path | None
    allow_warnings: bool
    force: bool


def asset_command(options: BuildOptions) -> list[str]:
    command = [
        sys.executable,
        str(ASSET_BUILDER),
        "--source",
        str(options.source),
        "--output",
        str(options.output),
        "--frame-index",
        str(options.frame_index),
        "--video-quality",
        options.video_quality,
        "--static-quality-mode",
        options.static_quality_mode,
        "--floating-crop",
        options.floating_crop,
        "--floating-size",
        options.floating_size,
        "--sidebar-crop",
        options.sidebar_crop,
        "--sidebar-size",
        options.sidebar_size,
        "--detail-crop",
        options.detail_crop,
        "--detail-size",
        options.detail_size,
        "--portrait-crop",
        options.portrait_crop,
        "--portrait-size",
        options.portrait_size,
        "--dialog-crop",
        options.dialog_crop,
        "--dialog-size",
        options.dialog_size,
    ]
    if options.video_crf is not None:
        command.extend(["--video-crf", str(options.video_crf)])
    if options.static_quality is not None:
        command.extend(["--static-quality", str(options.static_quality)])
    if options.frame_time is not None:
        command.extend(["--frame-time", str(options.frame_time)])
    palette = load_palette(options.palette)
    tint = options.tint_color or palette["colors"]["background"]
    command.extend(["--tint-color", tint])
    return command


def scaffold_command(options: BuildOptions, palette: Path, surface_plan: Path, check: bool = False) -> list[str]:
    command = [
        sys.executable,
        str(SCAFFOLD),
        "--output",
        str(options.output),
        "--id",
        options.appearance_id,
        "--name",
        options.name,
        "--version",
        options.version,
        "--mode",
        options.mode,
        "--palette",
        str(palette),
        "--surface-plan",
        str(surface_plan),
    ]
    if options.author:
        command.extend(["--author", options.author])
    if options.description:
        command.extend(["--description", options.description])
    command.append("--check" if check else "--force")
    return command


def ensure_runtime_checklist(output: Path, appearance_id: str) -> Path:
    path = output / RUNTIME_CHECKLIST_NAME
    checks = [
        ("workbench", "Normal workbench and collapsed navigation"),
        ("floating-chat", "Toolbar Mode and in-app floating mini chat"),
        ("catalogs", "Skills, MiniApp gallery, Agents, and Insights"),
        ("settings", "Settings, archived sessions, and keyboard shortcuts"),
        ("panels", "Terminal navigation, bottom terminal, files, and auxiliary panels"),
        ("dialogs", "Generic and dedicated dialogs"),
        ("motion", "Reduced-motion poster fallback"),
        ("readability", "Text readability over calm and bright frames"),
    ]
    return build.ensure_runtime_checklist(path, appearance_id=appearance_id, checks=checks)


def build_skin(options: BuildOptions) -> dict[str, Any]:
    options.source = options.source.resolve()
    options.output = options.output.resolve()
    options.palette = options.palette.resolve()
    options.surface_plan = options.surface_plan.resolve()
    if options.openbitfun_repo is not None:
        options.openbitfun_repo = options.openbitfun_repo.resolve()
    if not options.source.is_file():
        raise FileNotFoundError(f"Source does not exist: {options.source}")

    options.output.mkdir(parents=True, exist_ok=True)
    package = options.output / "package"
    build.initialize_package(
        package,
        appearance_id=options.appearance_id,
        name=options.name,
        mode=options.mode,
        force=options.force,
    )
    asset_result = build.run_json(asset_command(options))

    sources = options.output / "sources"
    palette_copy = sources / "palette.json"
    plan_copy = sources / "surface-plan.json"
    build.copy_input(options.palette, palette_copy)
    build.copy_input(options.surface_plan, plan_copy)
    build.run(scaffold_command(options, palette_copy, plan_copy))

    archive = options.output / f"{options.appearance_id}.openbitfun-appearance"
    build.validate_project(package)
    build.build_archive(package, archive)
    build.validate_archive(archive)

    host_report: dict[str, Any] | None = None
    report_path = options.output / "host-verification.json"
    if options.openbitfun_repo is not None:
        host_report = build.verify_host(
            options.openbitfun_repo,
            archive,
            report=report_path,
            strict_warnings=not options.allow_warnings,
        )

    video = media.probe_video(package / "assets" / "background.webm")
    media.validate_host_video(video)
    background_compression = build.archive_member_compression(archive, "assets/background.webm")
    if background_compression != zipfile.ZIP_STORED:
        raise CinematicContractError("Background video must use stored ZIP compression")

    runtime_checklist = ensure_runtime_checklist(options.output, options.appearance_id)
    palette = load_palette(palette_copy)
    build_config = {
        "schema": BUILD_CONFIG_SCHEMA,
        "schemaVersion": 1,
        "recipe": {
            "id": RECIPE_ID,
            "surfacePlanScope": "example-style-selection",
        },
        "appearance": {
            "id": options.appearance_id,
            "name": options.name,
            "version": options.version,
            "mode": options.mode,
            "author": options.author,
            "description": options.description,
        },
        "source": {
            "path": str(options.source),
            "bytes": options.source.stat().st_size,
            "sha256": build.sha256(options.source),
        },
        "assetBuild": {
            "frameTime": options.frame_time,
            "frameIndex": options.frame_index,
            "videoQuality": options.video_quality,
            "videoCrf": options.video_crf,
            "staticQualityMode": options.static_quality_mode,
            "staticQuality": options.static_quality,
            "resolvedEncoding": asset_result.get("encoding"),
            "tintColor": options.tint_color or palette["colors"]["background"],
            "floatingCrop": options.floating_crop,
            "floatingSize": options.floating_size,
            "sidebarCrop": options.sidebar_crop,
            "sidebarSize": options.sidebar_size,
            "detailCrop": options.detail_crop,
            "detailSize": options.detail_size,
            "portraitCrop": options.portrait_crop,
            "portraitSize": options.portrait_size,
            "dialogCrop": options.dialog_crop,
            "dialogSize": options.dialog_size,
        },
        "inputs": {
            "palette": {"path": "sources/palette.json", "sha256": build.sha256(palette_copy)},
            "surfacePlan": {"path": "sources/surface-plan.json", "sha256": build.sha256(plan_copy)},
        },
        "registry": build.registry_provenance(),
        "host": {
            "repo": str(options.openbitfun_repo) if options.openbitfun_repo else None,
            "strictWarnings": not options.allow_warnings,
        },
        "outputs": {
            "archive": {
                "path": archive.name,
                "bytes": archive.stat().st_size,
                "sha256": build.sha256(archive),
                "backgroundCompression": "stored",
            },
            "backgroundVideo": video,
            "contactSheet": "contact-sheet.png",
            "assetPreviewSheet": "asset-preview-sheet.png",
            "hostReport": report_path.name if host_report else None,
            "runtimeChecklist": runtime_checklist.name,
        },
        "verification": {
            "projectValidation": True,
            "archiveValidation": True,
            "hostValidation": bool(host_report and host_report.get("valid")),
            "hostWarnings": host_report.get("warnings", []) if host_report else None,
            "runtimeVisualInspection": False,
        },
    }
    config_path = options.output / BUILD_CONFIG_NAME
    build.atomic_write_json(config_path, build_config)
    result = {
        "output": str(options.output),
        "archive": str(archive),
        "archiveBytes": archive.stat().st_size,
        "archiveSha256": build.sha256(archive),
        "hostValid": host_report.get("valid") if host_report else None,
        "hostWarnings": host_report.get("warnings") if host_report else None,
        "runtimeVisualInspection": False,
        "buildConfig": str(config_path),
    }
    print(json.dumps(result, indent=2))
    return result


def options_from_config(output: Path, repo_override: Path | None) -> BuildOptions:
    config = build.read_json(output / BUILD_CONFIG_NAME)
    if config.get("schema") != BUILD_CONFIG_SCHEMA or config.get("schemaVersion") != 1:
        raise CinematicContractError("Unsupported cinematic build config")
    if config.get("recipe", {}).get("id") != RECIPE_ID:
        raise CinematicContractError("Build config belongs to a different style recipe")
    appearance = config["appearance"]
    source = config["source"]
    assets = config["assetBuild"]
    host_repo = repo_override or (Path(config["host"]["repo"]) if config["host"].get("repo") else None)
    return BuildOptions(
        source=Path(source["path"]),
        output=output,
        appearance_id=appearance["id"],
        name=appearance["name"],
        version=appearance["version"],
        mode=appearance["mode"],
        author=appearance.get("author"),
        description=appearance.get("description"),
        frame_time=assets.get("frameTime"),
        frame_index=assets.get("frameIndex", -1),
        video_quality=assets["videoQuality"],
        video_crf=assets.get("videoCrf"),
        static_quality_mode=assets["staticQualityMode"],
        static_quality=assets.get("staticQuality"),
        tint_color=assets.get("tintColor"),
        floating_crop=assets["floatingCrop"],
        floating_size=assets["floatingSize"],
        sidebar_crop=assets["sidebarCrop"],
        sidebar_size=assets["sidebarSize"],
        detail_crop=assets["detailCrop"],
        detail_size=assets["detailSize"],
        portrait_crop=assets["portraitCrop"],
        portrait_size=assets["portraitSize"],
        dialog_crop=assets["dialogCrop"],
        dialog_size=assets["dialogSize"],
        palette=output / config["inputs"]["palette"]["path"],
        surface_plan=output / config["inputs"]["surfacePlan"]["path"],
        openbitfun_repo=host_repo,
        allow_warnings=not config["host"].get("strictWarnings", True),
        force=True,
    )


def check_skin(output: Path, repo_override: Path | None, skip_host: bool) -> dict[str, Any]:
    output = output.resolve()
    config = build.read_json(output / BUILD_CONFIG_NAME)
    options = options_from_config(output, repo_override)
    issues: list[str] = []

    source = Path(config["source"]["path"])
    if not source.is_file() or build.sha256(source) != config["source"]["sha256"]:
        issues.append("source hash mismatch")
    for key in ("palette", "surfacePlan"):
        entry = config["inputs"][key]
        build.check_recorded_file(output, entry, label=key, issues=issues)

    build.run(scaffold_command(options, options.palette, options.surface_plan, check=True))
    package = output / "package"
    archive = output / config["outputs"]["archive"]["path"]
    build.validate_project(package)
    build.validate_archive(archive)
    if build.sha256(archive) != config["outputs"]["archive"]["sha256"]:
        issues.append("archive hash mismatch")

    if not skip_host and options.openbitfun_repo is not None:
        build.verify_host(
            options.openbitfun_repo,
            archive,
            strict_warnings=not options.allow_warnings,
        )

    checklist = build.read_json(output / RUNTIME_CHECKLIST_NAME)
    result = {
        "output": str(output),
        "valid": not issues,
        "issues": issues,
        "registry": build.registry_provenance(),
        "runtimeInspectionStatus": checklist.get("status"),
    }
    print(json.dumps(result, indent=2))
    if issues:
        raise CinematicContractError("Cinematic build check failed: " + ", ".join(issues))
    return result


def add_build_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--id", required=True, dest="appearance_id")
    parser.add_argument("--name", required=True)
    parser.add_argument("--version", default="1.0.0")
    parser.add_argument("--mode", choices=("light", "dark"), default="dark")
    parser.add_argument("--author")
    parser.add_argument("--description")
    parser.add_argument("--frame-time", type=float)
    parser.add_argument("--frame-index", type=int, default=-1)
    parser.add_argument(
        "--video-quality",
        choices=sorted(media.VIDEO_QUALITY_MODES),
        default="auto",
        help="Encoding policy; auto selects the highest quality that satisfies host limits.",
    )
    parser.add_argument("--video-crf", type=int, help="Explicit CRF override for reproducibility or manual tuning.")
    parser.add_argument(
        "--static-quality-mode",
        choices=sorted(media.STATIC_QUALITY_MODES),
        default="auto",
        help="WebP policy; auto tries lossless before quality fallback.",
    )
    parser.add_argument("--static-quality", type=int, help="Explicit lossy WebP quality override.")
    parser.add_argument("--tint-color")
    parser.add_argument("--floating-crop", default="0.18,0,0.82,1")
    parser.add_argument("--floating-size", default="820x720")
    parser.add_argument("--sidebar-crop", default="0.64,0,1,1")
    parser.add_argument("--sidebar-size", default="460x720")
    parser.add_argument("--detail-crop", default="0.41,0.54,1,1")
    parser.add_argument("--detail-size", default="760x330")
    parser.add_argument("--portrait-crop", default="0.21,0.06,0.80,0.79")
    parser.add_argument("--portrait-size", default="760x525")
    parser.add_argument("--dialog-crop", default="0.09,0.28,0.91,1")
    parser.add_argument("--dialog-size", default="1040x515")
    parser.add_argument("--palette", type=Path, default=DEFAULT_PALETTE)
    parser.add_argument("--surface-plan", type=Path, default=DEFAULT_SURFACE_PLAN)
    parser.add_argument("--openbitfun-repo", type=Path)
    parser.add_argument("--allow-warnings", action="store_true")
    parser.add_argument("--force", action="store_true")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build, rebuild, or check a cinematic OpenBitFun skin.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    contact = subparsers.add_parser("contact", help="Generate only the timestamped source contact sheet.")
    contact.add_argument("--source", type=Path, required=True)
    contact.add_argument("--output", type=Path, required=True)

    build = subparsers.add_parser("build", help="Run the complete cinematic package pipeline.")
    add_build_arguments(build)

    rebuild = subparsers.add_parser("rebuild", help="Rebuild from skin-build.json.")
    rebuild.add_argument("--output", type=Path, required=True)
    rebuild.add_argument("--openbitfun-repo", type=Path)

    check = subparsers.add_parser("check", help="Check reproducibility, hashes, validation, and host compatibility.")
    check.add_argument("--output", type=Path, required=True)
    check.add_argument("--openbitfun-repo", type=Path)
    check.add_argument("--skip-host-verify", action="store_true")
    return parser.parse_args()


def namespace_to_options(args: argparse.Namespace) -> BuildOptions:
    values = vars(args).copy()
    values.pop("command", None)
    return BuildOptions(**values)


def main() -> None:
    args = parse_args()
    try:
        if args.command == "contact":
            build.run([
                sys.executable,
                str(ASSET_BUILDER),
                "--source",
                str(args.source),
                "--output",
                str(args.output),
                "--contact-only",
            ])
        elif args.command == "build":
            build_skin(namespace_to_options(args))
        elif args.command == "rebuild":
            build_skin(options_from_config(args.output.resolve(), args.openbitfun_repo))
        else:
            check_skin(args.output, args.openbitfun_repo, args.skip_host_verify)
    except (
        CinematicContractError,
        build.BuildSupportError,
        media.MediaSupportError,
        FileNotFoundError,
        OSError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
    ) as error:
        raise SystemExit(f"ERROR: {error}") from error


if __name__ == "__main__":
    main()
