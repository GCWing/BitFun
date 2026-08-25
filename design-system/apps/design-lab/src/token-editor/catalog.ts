import {
  tokenCatalog as systemTokenCatalog,
  type SystemTokenMode,
} from "@bitfun/design-tokens";
import {
  themeTokenCatalog,
  type ThemeDataName,
} from "@bitfun/theme-bitfun";

export type TokenCollection = "system" | "theme";
export type EditableTokenMode = SystemTokenMode | ThemeDataName;

export interface EditableToken {
  readonly category: string;
  readonly collection: TokenCollection;
  readonly cssVariable: `--bf-${string}`;
  readonly description?: string;
  readonly name: string;
  readonly type: string;
  readonly values: Readonly<Record<string, string>>;
}

export const editableTokenCatalog: readonly EditableToken[] = [
  ...systemTokenCatalog.map((token) => ({
    ...token,
    collection: "system" as const,
  })),
  ...themeTokenCatalog.map((token) => ({
    ...token,
    collection: "theme" as const,
  })),
];

export const colorTokenCatalog: readonly EditableToken[] = editableTokenCatalog.filter(
  (token) => token.category === "color",
);

export const nonColorTokenCatalog: readonly EditableToken[] = editableTokenCatalog.filter(
  (token) => token.category !== "color",
);

const categoryLabels: Readonly<Record<string, string>> = {
  border: "Borders",
  color: "Semantic colors",
  control: "Controls",
  focus: "Focus",
  font: "Typography",
  layer: "Layers",
  layout: "Layout",
  lineHeight: "Line height",
  motion: "Motion",
  opacity: "Opacity",
  radius: "Radius",
  shadow: "Shadows",
  space: "Spacing",
};

export function getCategoryLabel(category: string): string {
  return categoryLabels[category] ?? humanizeTokenSegment(category);
}

export function humanizeTokenSegment(value: string): string {
  const normalized = value
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[._-]+/g, " ")
    .trim();
  return normalized.length === 0
    ? value
    : `${normalized[0]?.toUpperCase() ?? ""}${normalized.slice(1)}`;
}
