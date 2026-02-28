const fs = require("fs");
const path = require("path");

const INSTALLER_ROOT = path.resolve(__dirname, "..");
const PROJECT_ROOT = path.resolve(INSTALLER_ROOT, "..");

const THEME_IDS = [
  "bitfun-dark",
  "bitfun-light",
  "bitfun-midnight",
  "bitfun-china-style",
  "bitfun-china-night",
  "bitfun-cyber",
  "bitfun-slate",
];

function readJson(filePath) {
  const content = fs.readFileSync(filePath, "utf8");
  return JSON.parse(content);
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

function extractThemeNames(source, sourceLabel) {
  const presets = source?.theme?.presets;
  if (!presets || typeof presets !== "object") {
    throw new Error(`Invalid theme presets in ${sourceLabel}`);
  }

  const result = {};
  for (const themeId of THEME_IDS) {
    const name = presets?.[themeId]?.name;
    if (typeof name !== "string" || name.trim() === "") {
      throw new Error(`Missing theme name for '${themeId}' in ${sourceLabel}`);
    }
    result[themeId] = name;
  }

  return result;
}

function injectThemeNames(target, themeNames) {
  if (!target.themeSetup || typeof target.themeSetup !== "object") {
    target.themeSetup = {};
  }
  target.themeSetup.themeNames = {
    ...(target.themeSetup.themeNames || {}),
    ...themeNames,
  };
  return target;
}

function main() {
  const sourceEnPath = path.join(
    PROJECT_ROOT,
    "src",
    "web-ui",
    "src",
    "locales",
    "en-US",
    "settings",
    "theme.json"
  );
  const sourceZhPath = path.join(
    PROJECT_ROOT,
    "src",
    "web-ui",
    "src",
    "locales",
    "zh-CN",
    "settings",
    "theme.json"
  );

  const targetEnPath = path.join(
    INSTALLER_ROOT,
    "src",
    "i18n",
    "locales",
    "en.json"
  );
  const targetZhPath = path.join(
    INSTALLER_ROOT,
    "src",
    "i18n",
    "locales",
    "zh.json"
  );

  const sourceEn = readJson(sourceEnPath);
  const sourceZh = readJson(sourceZhPath);
  const targetEn = readJson(targetEnPath);
  const targetZh = readJson(targetZhPath);

  const enThemeNames = extractThemeNames(sourceEn, sourceEnPath);
  const zhThemeNames = extractThemeNames(sourceZh, sourceZhPath);

  writeJson(targetEnPath, injectThemeNames(targetEn, enThemeNames));
  writeJson(targetZhPath, injectThemeNames(targetZh, zhThemeNames));

  console.log("[sync-theme-i18n] Synced installer theme names from web-ui locales.");
}

main();
