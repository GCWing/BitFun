from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any, Mapping


SKILL_ROOT = Path(__file__).resolve().parents[3]
REFERENCES = Path(__file__).resolve().parents[1] / "references"
DEFAULT_PALETTE = REFERENCES / "default-palette.json"
DEFAULT_SURFACE_PLAN = REFERENCES / "surface-plan.json"
sys.path.insert(0, str(SKILL_ROOT / "scripts"))

import bitfun_appearance as appearance  # noqa: E402
from build_support import atomic_write_json, read_json  # noqa: E402


PALETTE_SCHEMA = "bitfun.appearance.cinematic-palette"
SURFACE_PLAN_SCHEMA = "bitfun.appearance.style-surface-plan"
STYLE_ID = "cinematic-animated-wallpaper"
PALETTE_KEYS = {
    "background",
    "surface",
    "surfaceElevated",
    "text",
    "textSecondary",
    "textMuted",
    "textDisabled",
    "accent",
    "accentStrong",
    "accentContrast",
    "info",
    "success",
    "warning",
    "danger",
}
HEX_PATTERN = re.compile(r"^#[0-9a-fA-F]{6}$")


class CinematicContractError(ValueError):
    pass


def load_palette(path: Path = DEFAULT_PALETTE) -> dict[str, Any]:
    palette = read_json(path)
    if palette.get("schema") != PALETTE_SCHEMA or palette.get("schemaVersion") != 1:
        raise CinematicContractError("Unsupported cinematic palette schema")
    palette_id = palette.get("id")
    if not isinstance(palette_id, str) or not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", palette_id):
        raise CinematicContractError("Palette id must use lowercase letters, digits, and hyphens")
    colors = palette.get("colors")
    if not isinstance(colors, dict):
        raise CinematicContractError("Palette colors must be an object")
    missing = sorted(PALETTE_KEYS - set(colors))
    extra = sorted(set(colors) - PALETTE_KEYS)
    if missing or extra:
        details = []
        if missing:
            details.append(f"missing: {', '.join(missing)}")
        if extra:
            details.append(f"unknown: {', '.join(extra)}")
        raise CinematicContractError("Invalid palette keys (" + "; ".join(details) + ")")
    invalid = sorted(key for key, value in colors.items() if not isinstance(value, str) or not HEX_PATTERN.fullmatch(value))
    if invalid:
        raise CinematicContractError(f"Palette colors must be six-digit hex values: {', '.join(invalid)}")
    return palette


def load_surface_plan(path: Path = DEFAULT_SURFACE_PLAN) -> dict[str, Any]:
    plan = read_json(path)
    if plan.get("schema") != SURFACE_PLAN_SCHEMA or plan.get("schemaVersion") != 1:
        raise CinematicContractError("Unsupported cinematic surface plan schema")
    if plan.get("scope") != "example-style-selection" or plan.get("styleId") != STYLE_ID:
        raise CinematicContractError("Cinematic surface plan must declare its example style scope")
    if not isinstance(plan.get("validatedAgainstRegistryRevision"), str):
        raise CinematicContractError("Cinematic surface plan must record its validated registry revision")
    if not isinstance(plan.get("components"), dict) or not isinstance(plan.get("scenes"), dict):
        raise CinematicContractError("Surface plan must define component and scene objects")
    return plan


def hex_rgb(value: str) -> tuple[int, int, int]:
    return int(value[1:3], 16), int(value[3:5], 16), int(value[5:7], 16)


def color_value(value: str, alpha: float = 1) -> dict[str, Any]:
    red, green, blue = hex_rgb(value)
    return {"kind": "rgb", "r": red, "g": green, "b": blue, "a": alpha}


def rgba(value: str, alpha: float, spaced: bool = True) -> str:
    red, green, blue = hex_rgb(value)
    separator = ", " if spaced else ","
    return f"rgba({red}{separator}{green}{separator}{blue}{separator}{alpha:.2f})"


def rgb_triplet(value: str, spaced: bool = True) -> str:
    separator = ", " if spaced else ","
    return separator.join(str(channel) for channel in hex_rgb(value))


def resolve_palette_values(value: Any, colors: Mapping[str, str]) -> Any:
    if isinstance(value, dict):
        if value.get("kind") == "palette":
            role = value.get("role")
            alpha = value.get("alpha", 1)
            if role not in colors:
                raise CinematicContractError(f"Unknown palette role in surface plan: {role}")
            if not isinstance(alpha, (int, float)) or isinstance(alpha, bool) or not 0 <= alpha <= 1:
                raise CinematicContractError(f"Invalid palette alpha for role {role}")
            return color_value(colors[role], float(alpha))
        return {key: resolve_palette_values(child, colors) for key, child in value.items()}
    if isinstance(value, list):
        return [resolve_palette_values(child, colors) for child in value]
    return value


def shadow(y: int, blur: int, spread: int, alpha: float) -> dict[str, Any]:
    return {
        "kind": "shadow",
        "x": {"kind": "zero"},
        "y": {"kind": "px", "value": y},
        "blur": {"kind": "px", "value": blur},
        "spread": {"kind": "px", "value": spread},
        "color": {"kind": "rgb", "r": 0, "g": 0, "b": 0, "a": alpha},
    }


def image_material(asset_id: str, background: str, alpha: float, role: str) -> dict[str, Any]:
    return {
        "visualRole": role,
        "style": {
            "backgroundColor": color_value(background, alpha),
            "backgroundImage": {"kind": "asset", "assetId": asset_id},
            "backgroundSize": "cover",
            "backgroundPosition": "center",
            "backgroundRepeat": "no-repeat",
            "borderColor": {"kind": "transparent"},
        },
    }


def build_materials(colors: Mapping[str, str]) -> dict[str, Any]:
    accent = colors["accent"]
    background = colors["background"]
    return {
        "transparent": {
            "visualRole": "continuous-surface",
            "style": {
                "backgroundColor": {"kind": "transparent"},
                "borderColor": {"kind": "transparent"},
            },
        },
        "workspace-art": {
            "visualRole": "workspace",
            "style": {"backgroundColor": {"kind": "transparent"}},
        },
        "sidebar-art": image_material("sidebar", background, 0.72, "panel"),
        "floating-art": image_material("floating", background, 0.34, "workspace"),
        "panel": {
            "visualRole": "panel",
            "style": {
                "backgroundColor": color_value(colors["surface"], 0.48),
                "backdropBlur": {"kind": "px", "value": 8},
                "borderColor": color_value(accent, 0.10),
            },
        },
        "card": {
            "visualRole": "card",
            "style": {
                "backgroundColor": color_value(colors["surfaceElevated"], 0.68),
                "backdropBlur": {"kind": "px", "value": 5},
                "borderColor": color_value(accent, 0.10),
                "boxShadow": [shadow(10, 28, -12, 0.48)],
            },
        },
        "control": {
            "visualRole": "control",
            "style": {
                "backgroundColor": color_value(colors["surfaceElevated"], 0.82),
                "backdropBlur": {"kind": "px", "value": 6},
                "borderColor": color_value(accent, 0.13),
            },
        },
        "detail-card": image_material("card-detail", background, 0.78, "card"),
        "portrait-card": image_material("card-portrait", background, 0.80, "card"),
        "dialog-art": {
            **image_material("dialog", background, 0.90, "dialog"),
            "style": {
                **image_material("dialog", background, 0.90, "dialog")["style"],
                "borderColor": color_value(accent, 0.12),
                "boxShadow": [shadow(18, 52, -16, 0.72)],
            },
        },
        "popup": {
            "visualRole": "popup",
            "style": {
                "backgroundColor": color_value(colors["surface"], 0.96),
                "borderColor": color_value(accent, 0.12),
            },
        },
        "code": {
            "visualRole": "content",
            "style": {
                "backgroundColor": {"kind": "hex", "value": background},
                "borderColor": {"kind": "transparent"},
            },
        },
    }


def build_renderers(colors: Mapping[str, str], appearance_id: str) -> dict[str, Any]:
    background = colors["background"]
    surface = colors["surface"]
    elevated = colors["surfaceElevated"]
    accent = colors["accent"]
    return {
        "css-tokens": {
            "version": 1,
            "settings": {
                "background": rgba(background, 0.18),
                "tokens": {
                    "--bf-appearance-token-color-bg-primary": rgba(background, 0.78),
                    "--bf-appearance-token-color-bg-secondary": rgba(surface, 0.72),
                    "--bf-appearance-token-color-bg-tertiary": rgba(elevated, 0.76),
                    "--bf-appearance-token-color-bg-elevated": rgba(elevated, 0.74),
                    "--bf-appearance-token-color-bg-scene": rgba(background, 0.08),
                    "--bf-appearance-token-color-bg-workbench": rgba(background, 0.08),
                    "--bf-appearance-token-element-bg-subtle": rgba(surface, 0.50),
                    "--bf-appearance-token-element-bg-soft": rgba(surface, 0.58),
                    "--bf-appearance-token-element-bg-base": rgba(elevated, 0.66),
                    "--bf-appearance-token-element-bg-medium": rgba(elevated, 0.70),
                    "--bf-appearance-token-element-bg-hover": rgba(elevated, 0.78),
                    "--bf-appearance-token-color-text-primary": colors["text"],
                    "--bf-appearance-token-color-text-secondary": colors["textSecondary"],
                    "--bf-appearance-token-color-text-muted": colors["textMuted"],
                    "--bf-appearance-token-color-text-disabled": colors["textDisabled"],
                    "--bf-appearance-token-color-accent-500": accent,
                    "--bf-appearance-token-color-accent-500-rgb": rgb_triplet(accent),
                    "--bf-appearance-token-color-accent-600": colors["accentStrong"],
                    "--bf-appearance-token-color-cyan-500": colors["info"],
                    "--bf-appearance-token-border-subtle": rgba(accent, 0.08),
                    "--bf-appearance-token-border-base": rgba(accent, 0.12),
                    "--bf-appearance-token-border-medium": rgba(accent, 0.18),
                    "--bf-appearance-token-border-strong": rgba(accent, 0.28),
                    "--bf-appearance-token-glass-bg-base": rgba(surface, 0.58),
                    "--bf-appearance-token-glass-bg-hover": rgba(elevated, 0.68),
                    "--bf-appearance-token-glass-bg-active": rgba(elevated, 0.76),
                    "--bf-appearance-token-glass-border-base": rgba(accent, 0.12),
                    "--bf-appearance-token-glass-border-hover": rgba(accent, 0.22),
                    "--bf-appearance-token-color-error": colors["danger"],
                    "--bf-appearance-token-color-error-bg": rgba(colors["danger"], 0.28),
                    "--bf-appearance-token-color-error-border": rgba(colors["danger"], 0.38),
                    "--bf-appearance-token-color-warning": colors["warning"],
                    "--bf-appearance-token-color-success": colors["success"],
                    "--bf-appearance-token-color-info": colors["info"],
                    "--bf-appearance-token-scrollbar-thumb": rgba(accent, 0.22),
                    "--bf-appearance-token-scrollbar-thumb-hover": rgba(accent, 0.38),
                },
            },
        },
        "monaco": {
            "version": 1,
            "settings": {
                "id": appearance_id,
                "base": "vs-dark",
                "inherit": True,
                "rules": [
                    {"token": "comment", "foreground": colors["textDisabled"][1:], "fontStyle": "italic"},
                    {"token": "keyword", "foreground": accent[1:]},
                    {"token": "string", "foreground": colors["success"][1:]},
                    {"token": "number", "foreground": colors["warning"][1:]},
                ],
                "colors": {
                    "editor.background": background.upper(),
                    "editor.foreground": colors["text"].upper(),
                    "editorLineNumber.foreground": colors["textDisabled"].upper(),
                    "editorLineNumber.activeForeground": colors["textSecondary"].upper(),
                    "editor.selectionBackground": rgba(accent, 0.38),
                    "editor.inactiveSelectionBackground": rgba(accent, 0.24),
                    "editorCursor.foreground": accent.upper(),
                    "editorWidget.background": surface.upper(),
                    "editorHoverWidget.background": surface.upper(),
                },
            },
        },
        "xterm": {
            "version": 1,
            "settings": {
                "surfaces": {
                    name: {
                        "background": background,
                        "foreground": colors["text"],
                        "cursor": accent,
                        "selectionBackground": rgba(accent, 0.34),
                    }
                    for name in ("terminal", "output")
                },
            },
        },
        "mermaid": {
            "version": 1,
            "settings": {
                "mode": "dark",
                "palette": {
                    "nodeFill": elevated,
                    "nodeFillHover": surface,
                    "nodeText": colors["text"],
                    "nodeStroke": colors["accentStrong"],
                    "nodeStrokeHover": accent,
                    "clusterFill": background,
                    "clusterText": colors["textSecondary"],
                    "clusterStroke": colors["textDisabled"],
                    "edgeStroke": colors["accentStrong"],
                    "edgeLabelBackground": surface,
                    "edgeLabelText": colors["text"],
                    "noteFill": surface,
                    "noteText": colors["textSecondary"],
                    "noteStroke": colors["warning"],
                    "activationFill": elevated,
                    "activationStroke": accent,
                    "success": colors["success"],
                    "warning": colors["warning"],
                    "error": colors["danger"],
                    "errorBackground": surface,
                    "info": colors["info"],
                    "highlight": colors["accentStrong"],
                    "pieColors": [
                        accent,
                        colors["danger"],
                        colors["warning"],
                        colors["success"],
                        colors["info"],
                        colors["accentStrong"],
                        colors["textMuted"],
                        colors["textSecondary"],
                    ],
                },
            },
        },
        "generative-widget": {
            "version": 1,
            "settings": {
                "id": appearance_id,
                "mode": "dark",
                "vars": {
                    "--bf-appearance-token-color-bg-primary": background,
                    "--bf-appearance-token-color-bg-secondary": surface,
                    "--bf-appearance-token-color-bg-tertiary": elevated,
                    "--bf-appearance-token-color-bg-elevated": elevated,
                    "--bf-appearance-token-color-bg-scene": background,
                    "--bf-appearance-token-color-text-primary": colors["text"],
                    "--bf-appearance-token-color-text-secondary": colors["textSecondary"],
                    "--bf-appearance-token-color-text-muted": colors["textMuted"],
                    "--bf-appearance-token-color-text-disabled": colors["textDisabled"],
                    "--bf-appearance-token-color-accent-500": accent,
                    "--bf-appearance-token-color-accent-500-rgb": rgb_triplet(accent, spaced=False),
                    "--bf-appearance-token-color-accent-600": colors["accentStrong"],
                    "--bf-appearance-token-border-subtle": rgba(accent, 0.08, spaced=False),
                    "--bf-appearance-token-border-base": rgba(accent, 0.12, spaced=False),
                    "--bf-appearance-token-border-medium": rgba(accent, 0.18, spaced=False),
                    "--bf-appearance-token-element-bg-subtle": rgba(surface, 0.50, spaced=False),
                    "--bf-appearance-token-element-bg-soft": rgba(surface, 0.58, spaced=False),
                    "--bf-appearance-token-element-bg-base": rgba(elevated, 0.66, spaced=False),
                    "--bf-appearance-token-element-bg-medium": rgba(elevated, 0.70, spaced=False),
                    "--bf-appearance-token-element-bg-hover": rgba(elevated, 0.78, spaced=False),
                },
            },
        },
        "bitfun-canvas": {
            "version": 1,
            "settings": {
                "id": appearance_id,
                "mode": "dark",
                "bg": background,
                "panel": elevated,
                "fg": colors["text"],
                "muted": colors["textMuted"],
                "border": colors["textDisabled"],
                "accent": accent,
                "success": colors["success"],
                "warning": colors["warning"],
                "danger": colors["danger"],
                "info": colors["info"],
            },
        },
    }


def build_assets() -> dict[str, Any]:
    definitions = {
        "background": ("video", "video/webm", "assets/background.webm"),
        "floating": ("image", "image/webp", "assets/floating.webp"),
        "sidebar": ("image", "image/webp", "assets/sidebar.webp"),
        "card-detail": ("image", "image/webp", "assets/card-detail.webp"),
        "card-portrait": ("image", "image/webp", "assets/card-portrait.webp"),
        "dialog": ("image", "image/webp", "assets/dialog.webp"),
        "preview": ("image", "image/webp", "assets/preview.webp"),
    }
    return {
        asset_id: {
            "kind": kind,
            "mimeType": mime_type,
            "source": {"kind": "package", "path": path},
        }
        for asset_id, (kind, mime_type, path) in definitions.items()
    }


def build_manifest(
    *,
    appearance_id: str,
    name: str,
    version: str,
    mode: str,
    palette: Mapping[str, Any],
    surface_plan: Mapping[str, Any],
    author: str | None = None,
    description: str | None = None,
) -> dict[str, Any]:
    colors = palette["colors"]
    manifest: dict[str, Any] = {
        "schema": "bitfun.appearance",
        "schemaVersion": 1,
        "id": appearance_id,
        "name": name,
        "version": version,
        "mode": mode,
        "preview": {"kind": "asset", "assetId": "preview"},
        "requiredCapabilities": ["assets.v1", "background-media.v1"],
        "globals": {},
        "materials": build_materials(colors),
        "components": resolve_palette_values(surface_plan["components"], colors),
        "scenes": resolve_palette_values(surface_plan["scenes"], colors),
        "renderers": build_renderers(colors, appearance_id),
        "assets": build_assets(),
        "integrity": {"sha256": {}},
        "backgroundMedia": {
            "kind": "video",
            "assetId": "background",
            "posterAssetId": "preview",
            "fit": "cover",
            "position": "center",
        },
    }
    if author:
        manifest["author"] = author
    if description:
        manifest["description"] = description
    return manifest


def validate_manifest(manifest: Mapping[str, Any]) -> list[dict[str, str]]:
    validator = appearance.ManifestValidator(appearance.load_registry())
    validator.validate(dict(manifest))
    if validator.errors:
        formatted = appearance.format_issues(validator.errors)
        raise CinematicContractError(f"Generated cinematic manifest is invalid:\n{formatted}")
    return validator.warnings


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def scaffold_comparable_manifest(value: Mapping[str, Any]) -> dict[str, Any]:
    comparable = json.loads(json.dumps(value))
    comparable["integrity"] = {"sha256": {}}
    return comparable
