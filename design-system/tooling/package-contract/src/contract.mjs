import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

async function collectFiles(directory, extensions) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(entryPath, extensions)));
    } else if (extensions.some((extension) => entry.name.endsWith(extension))) {
      files.push(entryPath);
    }
  }

  return files;
}

export async function readPublicCssVariableContract({
  systemCssPath,
  themeCssPath,
  uiSourceDirectory,
}) {
  const [systemCss, themeCss, uiCssFiles] = await Promise.all([
    readFile(systemCssPath, "utf8"),
    readFile(themeCssPath, "utf8"),
    collectFiles(uiSourceDirectory, [".css"]),
  ]);
  const definitions = new Set();
  const references = new Map();
  const definitionPattern = /(--bf-[a-z0-9-]+)\s*:/gi;
  const referencePattern = /var\(\s*(--bf-[a-z0-9-]+)/gi;

  for (const source of [systemCss, themeCss]) {
    for (const match of source.matchAll(definitionPattern)) {
      definitions.add(match[1]);
    }
  }

  for (const file of uiCssFiles) {
    const source = await readFile(file, "utf8");
    for (const match of source.matchAll(referencePattern)) {
      const consumers = references.get(match[1]) ?? [];
      consumers.push(file);
      references.set(match[1], consumers);
    }
  }

  return {
    definitions,
    missing: [...references.keys()].filter((name) => !definitions.has(name)).sort(),
    references,
  };
}

export async function findForbiddenUiDependencies(uiSourceDirectory) {
  const sourceFiles = await collectFiles(uiSourceDirectory, [".css", ".ts", ".tsx"]);
  const forbiddenPatterns = [
    { label: "application source", pattern: /src[\\/]web-ui|@\// },
    { label: "Tauri API", pattern: /@tauri-apps/ },
    { label: "application store", pattern: /zustand/ },
    { label: "application locale catalog", pattern: /locales?[\\/]/ },
  ];
  const failures = [];

  for (const file of sourceFiles) {
    const source = await readFile(file, "utf8");
    for (const rule of forbiddenPatterns) {
      if (rule.pattern.test(source)) {
        failures.push({ file, rule: rule.label });
      }
    }
  }

  return failures;
}

export async function findRawUiColors(uiSourceDirectory) {
  const cssFiles = await collectFiles(uiSourceDirectory, [".css"]);
  const failures = [];
  const colorPattern = /#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|oklch|oklab)\(/gi;

  for (const file of cssFiles) {
    const source = await readFile(file, "utf8");
    const matches = [...source.matchAll(colorPattern)].map((match) => match[0]);
    if (matches.length > 0) {
      failures.push({ file, values: matches });
    }
  }

  return failures;
}

export async function validatePublishedPackageManifest(manifestPath) {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const failures = [];

  if (manifest.private === true) {
    failures.push("package is private");
  }
  if (!manifest.publishConfig || manifest.publishConfig.access !== "public") {
    failures.push("publishConfig.access must be public");
  }
  if (!Array.isArray(manifest.files) || !manifest.files.includes("dist")) {
    failures.push("files must include dist");
  }

  const serializedExports = JSON.stringify(manifest.exports ?? {});
  if (serializedExports.includes("/src/") || serializedExports.includes("./src")) {
    failures.push("exports reference source files");
  }

  return failures;
}
