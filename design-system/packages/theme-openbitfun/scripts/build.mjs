import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  assertRequiredTokens,
  createTokenCatalog,
  diffResolvedTokens,
  mergeTokenDocuments,
  renderCss,
  resolveTokens,
  tokenNameToCssVariable,
} from "@openbitfun/token-engine";

const packageDirectory = fileURLToPath(new URL("../", import.meta.url));
const sourceDirectory = path.join(packageDirectory, "src");
const outputDirectory = path.join(packageDirectory, "dist");

async function readJson(fileName) {
  return JSON.parse(await readFile(path.join(sourceDirectory, fileName), "utf8"));
}

const [reference, light, dark, highContrastLight, highContrastDark] = await Promise.all([
  readJson("reference.tokens.json"),
  readJson("light.tokens.json"),
  readJson("dark.tokens.json"),
  readJson("high-contrast-light.tokens.json"),
  readJson("high-contrast-dark.tokens.json"),
]);

const referenceTokens = resolveTokens(reference);
const lightTokens = resolveTokens(mergeTokenDocuments(reference, light));
const darkTokens = resolveTokens(mergeTokenDocuments(reference, dark));
const highContrastLightTokens = resolveTokens(
  mergeTokenDocuments(reference, light, highContrastLight),
);
const highContrastDarkTokens = resolveTokens(
  mergeTokenDocuments(reference, dark, highContrastDark),
);

const PUBLIC_THEME_TOKEN_PREFIXES = ["color.", "effect.", "opacity.", "shadow."];
const REFERENCE_COLOR_TOKEN_PATTERN = /^ref\.color\.([a-z][a-z0-9-]*)\.(\d+)$/;

function createReferenceColorArtifacts(document, tokens) {
  const scaleNames = Object.keys(document.ref?.color ?? {})
    .filter((name) => !name.startsWith("$"));
  const entriesByScale = new Map(scaleNames.map((name) => [name, []]));

  for (const [name, token] of Object.entries(tokens)) {
    if (!name.startsWith("ref.color.")) {
      continue;
    }
    const match = name.match(REFERENCE_COLOR_TOKEN_PATTERN);
    if (!match) {
      throw new Error(
        `Reference color "${name}" must use the ref.color.<name>.<number> convention.`,
      );
    }
    if (token.type !== "color" || typeof token.value !== "string" || !/^#[0-9a-f]{6}$/i.test(token.value)) {
      throw new Error(`Reference color "${name}" must resolve to a six-digit hexadecimal color.`);
    }
    const scale = match[1];
    const steps = entriesByScale.get(scale);
    if (!steps) {
      throw new Error(`Reference color "${name}" belongs to an undeclared scale.`);
    }
    steps.push({
      name,
      scale,
      step: Number(match[2]),
      value: token.value.toLowerCase(),
    });
  }

  const scales = Object.fromEntries(
    [...entriesByScale].map(([scale, entries]) => {
      if (entries.length === 0) {
        throw new Error(`Reference color scale "${scale}" is empty.`);
      }
      entries.sort((left, right) => left.step - right.step);
      return [scale, entries];
    }),
  );

  return {
    catalog: Object.values(scales).flat(),
    scales,
  };
}

const referenceColorArtifacts = createReferenceColorArtifacts(reference, referenceTokens);

function selectSemanticTokens(tokens) {
  return Object.fromEntries(
    Object.entries(tokens).filter(([name]) =>
      PUBLIC_THEME_TOKEN_PREFIXES.some((prefix) => name.startsWith(prefix)),
    ),
  );
}

const semanticThemes = {
  light: selectSemanticTokens(lightTokens),
  dark: selectSemanticTokens(darkTokens),
  highContrastLight: selectSemanticTokens(highContrastLightTokens),
  highContrastDark: selectSemanticTokens(highContrastDarkTokens),
};

const requiredSemanticTokens = [
  "color.surface.canvas",
  "color.surface.panel",
  "color.surface.raised",
  "color.overlay.scrim",
  "color.content.primary",
  "color.content.secondary",
  "color.content.disabled",
  "color.keyHint.background",
  "color.border.default",
  "color.action.primary.background",
  "color.action.primary.hover",
  "color.action.primary.pressed",
  "color.action.primary.content",
  "color.action.neutral.border",
  "color.action.neutral.content",
  "color.action.neutral.contentDisabled",
  "color.action.neutral.fillBorder",
  "color.action.neutral.surface",
  "color.action.neutral.surfaceHover",
  "color.action.neutral.surfacePressed",
  "color.selection.surface",
  "color.field.background",
  "color.field.border",
  "color.field.borderFocus",
  "color.control.switch.track",
  "color.control.switch.trackChecked",
  "color.control.switch.thumb",
  "color.focus.ring",
  "color.identity.globalSearch.newSession",
  "color.identity.globalSearch.openBrowser",
  "color.identity.globalSearch.openTerminal",
  "color.identity.globalSearch.openProject",
  "color.identity.globalSearch.newProject",
  "color.identity.globalSearch.openFiles",
  "color.status.info.content",
  "color.status.info.surface",
  "color.status.success.content",
  "color.status.success.surface",
  "color.status.warning.content",
  "color.status.warning.emphasis",
  "color.status.warning.surface",
  "color.status.danger.content",
  "color.status.danger.surface",
  "effect.blur.base",
  "opacity.disabled",
  "shadow.composer",
  "shadow.raised",
  "shadow.overlay",
];

for (const tokens of [lightTokens, darkTokens, highContrastLightTokens, highContrastDarkTokens]) {
  assertRequiredTokens(tokens, requiredSemanticTokens);
}

const css = [
  renderCss(semanticThemes.light, {
    layer: "openbitfun.tokens.theme",
    selector: ':where([data-openbitfun-design-system-root]), :where([data-openbitfun-design-system-root][data-color-scheme="light"])',
  }),
  renderCss(semanticThemes.dark, {
    layer: "openbitfun.tokens.theme",
    selector: ':where([data-openbitfun-design-system-root][data-color-scheme="dark"])',
  }),
  renderCss(diffResolvedTokens(semanticThemes.light, semanticThemes.highContrastLight), {
    layer: "openbitfun.tokens.theme",
    selector: ':where([data-openbitfun-design-system-root][data-color-scheme="light"][data-contrast="high"])',
  }),
  renderCss(diffResolvedTokens(semanticThemes.dark, semanticThemes.highContrastDark), {
    layer: "openbitfun.tokens.theme",
    selector: ':where([data-openbitfun-design-system-root][data-color-scheme="dark"][data-contrast="high"])',
  }),
].join("\n");

const themeData = {
  dark: Object.fromEntries(Object.entries(semanticThemes.dark).map(([name, token]) => [name, token.value])),
  highContrastDark: Object.fromEntries(
    Object.entries(semanticThemes.highContrastDark).map(([name, token]) => [name, token.value]),
  ),
  highContrastLight: Object.fromEntries(
    Object.entries(semanticThemes.highContrastLight).map(([name, token]) => [name, token.value]),
  ),
  light: Object.fromEntries(Object.entries(semanticThemes.light).map(([name, token]) => [name, token.value])),
};
const themeCssVariables = Object.fromEntries(
  Object.keys(semanticThemes.light).map((name) => [name, tokenNameToCssVariable(name)]),
);
const themeModes = ["light", "dark", "highContrastLight", "highContrastDark"];
const themeTokenCatalog = createTokenCatalog(semanticThemes, { defaultMode: "light" });
const themeTokenNameUnion = Object.keys(semanticThemes.light)
  .map((name) => JSON.stringify(name))
  .join(" | ");
const referenceColorNameUnion = referenceColorArtifacts.catalog
  .map((entry) => JSON.stringify(entry.name))
  .join(" | ");
const referenceColorScaleNameUnion = Object.keys(referenceColorArtifacts.scales)
  .map((name) => JSON.stringify(name))
  .join(" | ");

const indexSource = [
  'export const themeContractVersion = "0.1.0";',
  `export const themes = Object.freeze(${JSON.stringify(themeData, null, 2)});`,
  `export const themeCssVariables = Object.freeze(${JSON.stringify(themeCssVariables, null, 2)});`,
  `export const themeModes = Object.freeze(${JSON.stringify(themeModes)});`,
  `export const themeTokenCatalog = Object.freeze(${JSON.stringify(themeTokenCatalog, null, 2)});`,
  "",
].join("\n");

const declarationSource = [
  'export type ThemeDataName = "light" | "dark" | "highContrastLight" | "highContrastDark";',
  `export type ThemeTokenName = ${themeTokenNameUnion || "never"};`,
  'export declare const themeContractVersion: "0.1.0";',
  "export declare const themes: Readonly<Record<ThemeDataName, Readonly<Record<ThemeTokenName, string | number | boolean>>>>;",
  "export declare const themeCssVariables: Readonly<Record<ThemeTokenName, `--openbitfun-${string}`>>;",
  "export interface ThemeTokenCatalogEntry {",
  "  readonly category: string;",
  "  readonly cssVariable: `--openbitfun-${string}`;",
  "  readonly description?: string;",
  "  readonly name: ThemeTokenName;",
  "  readonly type: string;",
  "  readonly values: Readonly<Record<ThemeDataName, string>>;",
  "}",
  "export declare const themeModes: readonly ThemeDataName[];",
  "export declare const themeTokenCatalog: readonly ThemeTokenCatalogEntry[];",
  "",
].join("\n");

const authoringSource = [
  `export const referenceColorCatalog = Object.freeze(${JSON.stringify(referenceColorArtifacts.catalog, null, 2)});`,
  `export const referenceColorScales = Object.freeze(${JSON.stringify(referenceColorArtifacts.scales, null, 2)});`,
  "",
].join("\n");

const authoringDeclarationSource = [
  `export type ReferenceColorName = ${referenceColorNameUnion || "never"};`,
  `export type ReferenceColorScaleName = ${referenceColorScaleNameUnion || "never"};`,
  "export interface ReferenceColorEntry {",
  "  readonly name: ReferenceColorName;",
  "  readonly scale: ReferenceColorScaleName;",
  "  readonly step: number;",
  "  readonly value: `#${string}`;",
  "}",
  "export declare const referenceColorCatalog: readonly ReferenceColorEntry[];",
  "export declare const referenceColorScales: Readonly<Record<ReferenceColorScaleName, readonly ReferenceColorEntry[]>>;",
  "",
].join("\n");

await mkdir(outputDirectory, { recursive: true });
await Promise.all([
  writeFile(path.join(outputDirectory, "themes.css"), css),
  writeFile(
    path.join(outputDirectory, "default.css"),
    '@import "@openbitfun/design-tokens/tokens.css";\n@import "./themes.css";\n',
  ),
  writeFile(path.join(outputDirectory, "theme-data.json"), `${JSON.stringify(themeData, null, 2)}\n`),
  writeFile(path.join(outputDirectory, "index.js"), indexSource),
  writeFile(path.join(outputDirectory, "index.d.ts"), declarationSource),
  writeFile(path.join(outputDirectory, "authoring.js"), authoringSource),
  writeFile(path.join(outputDirectory, "authoring.d.ts"), authoringDeclarationSource),
  writeFile(
    path.join(outputDirectory, "reference-colors.json"),
    `${JSON.stringify(referenceColorArtifacts.scales, null, 2)}\n`,
  ),
  copyFile(path.resolve(packageDirectory, "..", "..", "LICENSE"), path.join(outputDirectory, "LICENSE")),
]);
