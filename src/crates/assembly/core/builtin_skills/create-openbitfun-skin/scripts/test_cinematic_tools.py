#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SKILL_ROOT = Path(__file__).resolve().parents[1]
EXAMPLE_ROOT = SKILL_ROOT / "examples" / "cinematic-animated-wallpaper"
EXAMPLE_SCRIPTS = EXAMPLE_ROOT / "scripts"
sys.path.insert(0, str(SKILL_ROOT / "scripts"))
sys.path.insert(0, str(EXAMPLE_SCRIPTS))

import build_support
import build_cinematic_skin as pipeline
import cinematic_recipe as contract
import media_support
import verify_host as host_verifier


class CinematicContractTests(unittest.TestCase):
    def test_default_manifest_is_current_and_warning_free(self) -> None:
        palette = contract.load_palette()
        plan = contract.load_surface_plan()
        manifest = contract.build_manifest(
            appearance_id="cinematic-contract-test",
            name="Cinematic Contract Test",
            version="1.0.0",
            mode="dark",
            palette=palette,
            surface_plan=plan,
        )
        self.assertEqual([], contract.validate_manifest(manifest))
        self.assertEqual(2, manifest["schemaVersion"])
        self.assertIn("theme-tokens", manifest["renderers"])
        self.assertNotIn("css-tokens", manifest["renderers"])
        serialized_renderers = json.dumps(manifest["renderers"])
        self.assertNotIn("--openbitfun-appearance-token-", serialized_renderers)
        self.assertIn("flow-chat-turn-rail", manifest["components"])
        self.assertIn("copyable-text-preview", manifest["components"])
        self.assertNotIn("input", manifest["components"]["toolbar-mode"]["parts"])
        self.assertNotIn("sessionMenu", manifest["components"]["toolbar-mode"]["parts"])
        self.assertNotIn("inputBar", manifest["components"]["floating-mini-chat"]["parts"])

    def test_palette_requires_exact_semantic_keys_and_hex_values(self) -> None:
        palette = contract.load_palette()
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "palette.json"
            broken = json.loads(json.dumps(palette))
            broken["colors"].pop("accent")
            contract.atomic_write_json(path, broken)
            with self.assertRaisesRegex(contract.CinematicContractError, "missing: accent"):
                contract.load_palette(path)

            broken = json.loads(json.dumps(palette))
            broken["colors"]["accent"] = "rgba(1, 2, 3, 1)"
            contract.atomic_write_json(path, broken)
            with self.assertRaisesRegex(contract.CinematicContractError, "six-digit hex"):
                contract.load_palette(path)

    def test_surface_plan_contains_only_resolvable_palette_placeholders(self) -> None:
        palette = contract.load_palette()
        plan = contract.load_surface_plan()
        self.assertEqual("example-style-selection", plan["scope"])
        self.assertEqual("cinematic-animated-wallpaper", plan["styleId"])
        self.assertEqual(
            build_support.registry_provenance()["sourceRevision"],
            plan["validatedAgainstRegistryRevision"],
        )
        resolved = contract.resolve_palette_values(plan, palette["colors"])
        serialized = json.dumps(resolved)
        self.assertNotIn('"kind": "palette"', serialized)
        self.assertEqual(41, len(plan["components"]))
        self.assertEqual(7, len(plan["scenes"]))

    def test_generic_support_validates_media_and_recorded_hashes(self) -> None:
        self.assertEqual((0.1, 0.2, 0.9, 1.0), media_support.parse_crop("0.1,0.2,0.9,1"))
        media_support.validate_host_video({
            "codec": "vp9",
            "width": 1920,
            "height": 1080,
            "durationSeconds": 12,
            "bytes": 1024,
        })
        with self.assertRaisesRegex(media_support.MediaSupportError, "5160x2160"):
            media_support.validate_host_video({
                "codec": "vp9",
                "width": 3840,
                "height": 2160,
                "displayWidth": 5160,
                "displayHeight": 2160,
                "durationSeconds": 12,
                "bytes": 1024,
            })
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            recorded = root / "recorded.txt"
            recorded.write_text("stable", encoding="utf-8")
            issues: list[str] = []
            build_support.check_recorded_file(
                root,
                {"path": "recorded.txt", "sha256": build_support.sha256(recorded)},
                label="fixture",
                issues=issues,
            )
            self.assertEqual([], issues)

    def test_build_record_requires_current_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            build_support.atomic_write_json(output / "skin-build.json", {
                "schema": "openbitfun.appearance.unsupported-build",
                "schemaVersion": 1,
            })
            with self.assertRaisesRegex(contract.CinematicContractError, "Unsupported cinematic build config"):
                pipeline.options_from_config(output, None)

    @unittest.skipUnless(shutil.which("ffmpeg") and shutil.which("ffprobe"), "ffmpeg is required")
    def test_complete_build_and_check_pipeline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "fixture.mp4"
            output = root / "skin"
            subprocess.run(
                [
                    "ffmpeg",
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=0x7f1d1d:s=320x180:r=10",
                    "-t",
                    "1",
                    "-vf",
                    "setsar=43/32",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    str(source),
                ],
                check=True,
            )
            self.assertEqual((430, 180), media_support.extract_video_frame(source, 0.5).size)
            build_script = EXAMPLE_SCRIPTS / "build_cinematic_skin.py"
            subprocess.run(
                [
                    sys.executable,
                    "-B",
                    str(build_script),
                    "build",
                    "--source",
                    str(source),
                    "--output",
                    str(output),
                    "--id",
                    "pipeline-fixture",
                    "--name",
                    "Pipeline Fixture",
                    "--frame-time",
                    "0.5",
                ],
                cwd=SKILL_ROOT,
                check=True,
            )
            config_path = output / "skin-build.json"
            config = json.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual("openbitfun.appearance.recipe-build", config["schema"])
            self.assertEqual("cinematic-animated-wallpaper", config["recipe"]["id"])
            self.assertEqual("sources/surface-plan.json", config["inputs"]["surfacePlan"]["path"])
            self.assertEqual("auto", config["assetBuild"]["videoQuality"])
            self.assertEqual("auto", config["assetBuild"]["staticQualityMode"])
            resolved = config["assetBuild"]["resolvedEncoding"]
            self.assertTrue(resolved["video"]["codecLossless"])
            video = resolved["video"]["video"]
            self.assertEqual("1:1", video["sampleAspectRatio"])
            self.assertEqual(video["width"], video["displayWidth"])
            self.assertEqual(video["height"], video["displayHeight"])
            self.assertEqual(430, video["width"])
            self.assertEqual(180, video["height"])
            self.assertTrue(all(entry["lossless"] for entry in resolved["static"].values()))
            archive = output / "pipeline-fixture.openbitfun-appearance"
            manifest = json.loads((output / "package" / "appearance.json").read_text(encoding="utf-8"))
            host_video = host_verifier.verify_video_assets(archive, manifest)["background"]
            self.assertEqual("1:1", host_video["sampleAspectRatio"])
            self.assertEqual(host_video["width"], host_video["displayWidth"])

            subprocess.run(
                [
                    sys.executable,
                    "-B",
                    str(build_script),
                    "rebuild",
                    "--output",
                    str(output),
                ],
                cwd=SKILL_ROOT,
                check=True,
            )
            subprocess.run(
                [
                    sys.executable,
                    "-B",
                    str(build_script),
                    "check",
                    "--output",
                    str(output),
                    "--skip-host-verify",
                ],
                cwd=SKILL_ROOT,
                check=True,
            )
            config = json.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual("openbitfun.appearance.recipe-build", config["schema"])
            self.assertTrue(config["verification"]["projectValidation"])
            self.assertTrue(config["verification"]["archiveValidation"])
            self.assertFalse(config["verification"]["runtimeVisualInspection"])
            self.assertTrue((output / "asset-preview-sheet.png").is_file())
            self.assertTrue((output / "pipeline-fixture.openbitfun-appearance").is_file())


if __name__ == "__main__":
    unittest.main()
