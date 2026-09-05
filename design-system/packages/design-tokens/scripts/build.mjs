import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  assertRequiredTokens,
  createTokenCatalog,
  createTokenArtifacts,
  diffResolvedTokens,
  mergeTokenDocuments,
  renderCss,
  resolveTokens,
} from "@openbitfun/token-engine";

const packageDirectory = fileURLToPath(new URL("../", import.meta.url));
const sourceDirectory = path.join(packageDirectory, "src");
const outputDirectory = path.join(packageDirectory, "dist");

async function readJson(fileName) {
  return JSON.parse(await readFile(path.join(sourceDirectory, fileName), "utf8"));
}

const [systemDocument, compactDocument, touchDocument] = await Promise.all([
  readJson("system.tokens.json"),
  readJson("density-compact.tokens.json"),
  readJson("density-touch.tokens.json"),
]);

const systemTokens = resolveTokens(systemDocument);
const compactTokens = resolveTokens(mergeTokenDocuments(systemDocument, compactDocument));
const touchTokens = resolveTokens(mergeTokenDocuments(systemDocument, touchDocument));

assertRequiredTokens(systemTokens, [
  "space.2",
  "space.component.inline",
  "space.component.block",
  "font.family.control",
  "font.family.sans",
  "font.size.micro",
  "font.size.base",
  "font.size.8xl",
  "font.weight.bold",
  "lineHeight.reading",
  "letterSpacing.normal",
  "type.body.md.fontSize",
  "type.label.md.fontWeight",
  "type.heading.page.fontSize",
  "type.code.md.fontFamily",
  "type.flow.body.lineHeight",
  "control.height.sm",
  "control.height.md",
  "control.height.lg",
  "control.hitTarget",
  "control.switch.trackWidth",
  "control.switch.trackHeight",
  "control.switch.thumbSize",
  "control.switch.thumbInset",
  "control.switch.thumbTravel",
  "control.switch.thumbTravelReverse",
  "control.tabGroup.gap",
  "control.tabGroup.itemGap",
  "control.tabGroup.itemHeight",
  "control.tabGroup.itemIconSize",
  "control.tabGroup.itemPaddingInline",
  "control.tabGroup.itemActionSize",
  "control.tabGroup.itemActionInset",
  "control.tabGroup.itemRadius",
  "layout.splitView.contentPanelRadius",
  "overlay.dialog.viewportGutter",
  "overlay.dialog.edgeGutter",
  "overlay.dialog.backdropBlur",
  "overlay.dialog.surfaceRadius",
  "overlay.dialog.headerGap",
  "overlay.dialog.headerPaddingBlockStart",
  "overlay.dialog.headerPaddingBlockEnd",
  "overlay.dialog.headerPaddingInline",
  "overlay.dialog.scrollbarWidth",
  "overlay.dialog.contentPaddingSm",
  "overlay.dialog.contentPaddingMd",
  "overlay.dialog.contentPaddingLg",
  "overlay.dialog.contentPaddingXl",
  "overlay.dialog.descriptionGap",
  "overlay.dialog.headerActionsGap",
  "overlay.dialog.footerGap",
  "overlay.dialog.footerPaddingBlockStart",
  "overlay.dialog.footerPaddingBlockEnd",
  "overlay.dialog.footerPaddingInline",
  "overlay.dialog.footerHeight",
  "overlay.dialog.footerFadeExtent",
  "overlay.dialog.footerContentInset",
  "overlay.dialog.footerBlur",
  "overlay.dialog.footerActionMinWidth",
  "overlay.dialog.maxInlineSizeSmall",
  "overlay.dialog.maxInlineSizeMedium",
  "overlay.dialog.maxInlineSizeLarge",
  "overlay.dialog.maxInlineSizeXlarge",
  "overlay.dialog.maxInlineSizeXxlarge",
  "overlay.dialog.maxInlineSizeWide",
  "radius.xs",
  "radius.md",
  "radius.3xl",
  "radius.4xl",
  "motion.duration.fast",
  "focus.width",
]);

const artifacts = createTokenArtifacts(systemTokens);
const tokenModes = ["comfortable", "compact", "touch"];
const tokenCatalog = createTokenCatalog(
  {
    comfortable: systemTokens,
    compact: compactTokens,
    touch: touchTokens,
  },
  { defaultMode: "comfortable" },
);
const javascript = [
  artifacts.javascript.trimEnd(),
  `export const tokenModes = Object.freeze(${JSON.stringify(tokenModes)});`,
  `export const tokenCatalog = Object.freeze(${JSON.stringify(tokenCatalog, null, 2)});`,
  "",
].join("\n");
const typescript = [
  artifacts.typescript.trimEnd(),
  `export type SystemTokenMode = ${tokenModes.map((mode) => JSON.stringify(mode)).join(" | ")};`,
  "export interface SystemTokenCatalogEntry {",
  "  readonly category: string;",
  "  readonly cssVariable: `--openbitfun-${string}`;",
  "  readonly description?: string;",
  "  readonly name: TokenName;",
  "  readonly type: string;",
  "  readonly values: Readonly<Record<SystemTokenMode, string>>;",
  "}",
  "export declare const tokenModes: readonly SystemTokenMode[];",
  "export declare const tokenCatalog: readonly SystemTokenCatalogEntry[];",
  "",
].join("\n");
const css = [
  "@layer openbitfun.tokens.system, openbitfun.tokens.theme, openbitfun.reset, openbitfun.base, openbitfun.components, openbitfun.overrides;\n",
  renderCss(systemTokens, {
    layer: "openbitfun.tokens.system",
    preserveReferences: true,
    selector: ":where([data-openbitfun-design-system-root])",
  }),
  renderCss(diffResolvedTokens(systemTokens, compactTokens), {
    layer: "openbitfun.tokens.system",
    preserveReferences: true,
    selector: ':where([data-openbitfun-design-system-root][data-density="compact"])',
  }),
  renderCss(diffResolvedTokens(systemTokens, touchTokens), {
    layer: "openbitfun.tokens.system",
    preserveReferences: true,
    selector: ':where([data-openbitfun-design-system-root][data-density="touch"])',
  }),
  "@media (prefers-reduced-motion: reduce) {\n",
  "  :where([data-openbitfun-design-system-root]) {\n",
  "    --openbitfun-motion-duration-fast: 0ms;\n",
  "    --openbitfun-motion-duration-normal: 0ms;\n",
  "    --openbitfun-motion-duration-slow: 0ms;\n",
  "    --openbitfun-motion-duration-loop: 0ms;\n",
  "  }\n",
  "}\n",
].join("\n");

await mkdir(outputDirectory, { recursive: true });
await Promise.all([
  writeFile(path.join(outputDirectory, "tokens.css"), css),
  writeFile(path.join(outputDirectory, "tokens.json"), artifacts.json),
  writeFile(path.join(outputDirectory, "index.js"), javascript),
  writeFile(path.join(outputDirectory, "index.d.ts"), typescript),
  copyFile(path.join(sourceDirectory, "typography-runtime.mjs"), path.join(outputDirectory, "typography-runtime.mjs")),
  copyFile(path.join(sourceDirectory, "typography-runtime.d.ts"), path.join(outputDirectory, "typography-runtime.d.ts")),
  copyFile(path.resolve(packageDirectory, "..", "..", "LICENSE"), path.join(outputDirectory, "LICENSE")),
]);
