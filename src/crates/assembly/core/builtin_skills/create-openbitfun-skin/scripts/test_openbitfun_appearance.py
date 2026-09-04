#!/usr/bin/env python3

from __future__ import annotations

import unittest

import openbitfun_appearance as appearance
import sync_registry as registry_sync


def base_manifest() -> dict[str, object]:
    return {
        "schema": "openbitfun.appearance",
        "schemaVersion": 2,
        "id": "validator-test",
        "name": "Validator Test",
        "version": "1.0.0",
        "mode": "dark",
    }


class ManifestValidatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.registry = appearance.load_registry()

    def validate(self, manifest: dict[str, object]) -> appearance.ManifestValidator:
        validator = appearance.ManifestValidator(self.registry)
        validator.validate(manifest)
        return validator

    def test_unknown_and_circular_references_are_rejected(self) -> None:
        unknown = base_manifest()
        unknown["globals"] = {
            "colors": {"accent": {"kind": "ref", "path": "globals.colors.missing"}},
        }
        self.assertIn(
            "UNKNOWN_TOKEN_REFERENCE",
            {issue["code"] for issue in self.validate(unknown).errors},
        )

        circular = base_manifest()
        circular["globals"] = {
            "colors": {
                "first": {"kind": "ref", "path": "globals.colors.second"},
                "second": {"kind": "ref", "path": "globals.colors.first"},
            },
        }
        self.assertIn(
            "CIRCULAR_TOKEN_REFERENCE",
            {issue["code"] for issue in self.validate(circular).errors},
        )

    def test_reference_type_mismatch_is_rejected(self) -> None:
        manifest = base_manifest()
        manifest["globals"] = {
            "colors": {"accent": {"kind": "hex", "value": "#ffffff"}},
            "lengths": {"spacing": {"kind": "ref", "path": "globals.colors.accent"}},
        }
        self.assertIn(
            "REFERENCE_TYPE_MISMATCH",
            {issue["code"] for issue in self.validate(manifest).errors},
        )

    def test_appearance_typography_ownership_is_rejected(self) -> None:
        manifest = base_manifest()
        manifest["globals"] = {
            "fontFamilies": {
                "body": {"kind": "fontFamily", "families": ["Inter"]},
            },
        }
        codes = {issue["code"] for issue in self.validate(manifest).errors}
        self.assertIn("UNKNOWN_GLOBAL_GROUP", codes)

        validator = appearance.ManifestValidator(self.registry)
        validator.validate_style(
            {"fontSize": {"kind": "px", "value": 14}},
            "style",
            None,
        )
        self.assertIn(
            "UNSUPPORTED_STYLE_PROPERTY",
            {issue["code"] for issue in validator.errors},
        )

    def test_xterm_typography_settings_are_rejected(self) -> None:
        colors = {
            "background": "#000000",
            "foreground": "#ffffff",
            "cursor": "#ffffff",
        }
        validator = appearance.ManifestValidator(self.registry)
        errors = validator.validate_xterm({
            "surfaces": {
                "terminal": colors,
                "output": colors,
            },
            "fontWeight": "normal",
            "fontWeightBold": "700",
        })

        self.assertIn("Unknown setting: fontWeight", errors)
        self.assertIn("Unknown setting: fontWeightBold", errors)

    def test_theme_token_renderer_accepts_only_canonical_root_and_scope_tokens(self) -> None:
        validator = appearance.ManifestValidator(self.registry)
        self.assertEqual([], validator.validate_theme_tokens({
            "tokens": {"--openbitfun-color-surface-canvas": "#101820"},
            "scopes": {"chrome": {"--openbitfun-color-content-primary": "#f5f7fa"}},
        }))

        errors = validator.validate_theme_tokens({
            "tokens": {"--openbitfun-appearance-token-color-bg-primary": "#101820"},
        })
        self.assertIn(
            "Unsupported root token name: --openbitfun-appearance-token-color-bg-primary",
            errors,
        )

    def test_normalization_and_visual_semantic_warnings_are_reported(self) -> None:
        manifest = base_manifest()
        manifest["components"] = {
            "card": {
                "parts": {
                    "root": {
                        "base": {"borderStyle": "solid"},
                    },
                },
            },
            "nav-panel": {
                "parts": {
                    "content": {
                        "base": {"borderRadius": {"kind": "px", "value": 4}},
                    },
                },
            },
        }
        validator = self.validate(manifest)
        self.assertEqual([], validator.errors)
        codes = {issue["code"] for issue in validator.warnings}
        self.assertIn("BORDER_WIDTH_NORMALIZED", codes)
        self.assertIn("CONTINUOUS_SURFACE_FRAMED", codes)

    def test_excessive_override_usage_is_reported(self) -> None:
        manifest = base_manifest()
        manifest["components"] = {
            "card": {
                "parts": {
                    part: {"cascade": "override"}
                    for part in ("root", "header", "body", "footer")
                },
            },
        }
        validator = self.validate(manifest)
        self.assertEqual([], validator.errors)
        self.assertIn(
            "EXCESSIVE_OVERRIDE_USAGE",
            {issue["code"] for issue in validator.warnings},
        )

    def test_contract_output_preserves_authoring_metadata(self) -> None:
        nav_panel = next(item for item in self.registry["components"] if item["id"] == "nav-panel")
        formatted = appearance.format_descriptor(nav_panel, "component")
        content = next(part for part in formatted["parts"] if part["id"] == "content")
        self.assertEqual("container", content["propertyProfile"])
        self.assertEqual("nav-panel", content["continuityGroup"])
        card = next(item for item in self.registry["components"] if item["id"] == "card")
        formatted_card = appearance.format_descriptor(card, "component")
        self.assertTrue(formatted_card["states"])
        self.assertTrue(all("selector" in state for state in formatted_card["states"]))

    def test_registry_contract_comparison_ignores_provenance_only_changes(self) -> None:
        current = {
            "schema": "openbitfun.appearance.registry",
            "schemaVersion": 2,
            "sourceRevision": "old-revision",
            "generatedAt": "old-time",
            **{key: [] for key in registry_sync.CONTRACT_KEYS},
        }
        checkout = {
            **current,
            "sourceRevision": "new-revision",
            "generatedAt": "new-time",
        }
        self.assertEqual(
            registry_sync.contract_view(current),
            registry_sync.contract_view(checkout),
        )

        checkout["components"] = [{"id": "new-component"}]
        self.assertNotEqual(
            registry_sync.contract_view(current),
            registry_sync.contract_view(checkout),
        )


if __name__ == "__main__":
    unittest.main()
