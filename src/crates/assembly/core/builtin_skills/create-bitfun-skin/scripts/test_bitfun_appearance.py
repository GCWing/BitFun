#!/usr/bin/env python3

from __future__ import annotations

import unittest

import bitfun_appearance as appearance
import sync_registry as registry_sync


def base_manifest() -> dict[str, object]:
    return {
        "schema": "bitfun.appearance",
        "schemaVersion": 1,
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

    def test_normalization_and_visual_semantic_warnings_are_reported(self) -> None:
        manifest = base_manifest()
        manifest["components"] = {
            "button": {
                "parts": {
                    "root": {
                        "base": {"borderStyle": "solid"},
                    },
                },
            },
            "modal": {
                "parts": {
                    "headerShell": {
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
        modal = next(item for item in self.registry["components"] if item["id"] == "modal")
        formatted = appearance.format_descriptor(modal, "component")
        header = next(part for part in formatted["parts"] if part["id"] == "headerShell")
        self.assertEqual("container", header["propertyProfile"])
        self.assertEqual("modal-dialog", header["continuityGroup"])
        button = next(item for item in self.registry["components"] if item["id"] == "button")
        formatted_button = appearance.format_descriptor(button, "component")
        self.assertTrue(formatted_button["states"])
        self.assertTrue(all("selector" in state for state in formatted_button["states"]))

    def test_registry_contract_comparison_ignores_provenance_only_changes(self) -> None:
        current = {
            "schema": "bitfun.appearance.registry",
            "schemaVersion": 1,
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
