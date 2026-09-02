import type { ComponentMeta } from "../../registry.types";

export const launcherButtonMeta = {
  category: "action",
  description: "A shell-edge launcher with a leading icon and distinct default, hover, and pressed emphasis states.",
  maturity: "stable",
  name: "LauncherButton",
  props: [
    { name: "children", type: "ReactNode" },
    { name: "leadingIcon", type: "ReactNode" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
  ],
  states: ["default", "hover", "active", "focus-visible", "disabled"],
  tokens: [
    "color.control.launcher.background",
    "color.control.launcher.backgroundHover",
    "color.control.launcher.backgroundPressed",
    "color.control.launcher.content",
    "color.control.launcher.contentHover",
    "color.control.launcher.contentPressed",
    "color.control.launcher.contentDisabled",
    "color.focus.ring",
    "control.launcherButton.minInlineSize",
    "control.launcherButton.blockSize",
    "control.launcherButton.paddingInline",
    "control.launcherButton.gap",
    "control.launcherButton.iconSize",
    "control.launcherButton.radius",
    "type.code.md.fontSize",
  ],
} as const satisfies ComponentMeta;
