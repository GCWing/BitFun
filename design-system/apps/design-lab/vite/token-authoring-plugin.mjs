import { execFile } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import {
  collectTokenDefinitions,
  mergeTokenDocuments,
} from "@openbitfun/token-engine";

const execFileAsync = promisify(execFile);
const ENDPOINT = "/__openbitfun-design-lab/token-source";
const MAX_BODY_BYTES = 128 * 1024;

const targetDefinitions = {
  "system:comfortable": {
    allowedSources: ["system"],
    packageName: "@openbitfun/design-tokens",
    targetSource: "system",
  },
  "system:compact": {
    allowedSources: ["system", "compact"],
    packageName: "@openbitfun/design-tokens",
    targetSource: "compact",
  },
  "system:touch": {
    allowedSources: ["system", "touch"],
    packageName: "@openbitfun/design-tokens",
    targetSource: "touch",
  },
  "theme:light": {
    allowedSources: ["reference", "light"],
    packageName: "@openbitfun/theme-openbitfun",
    targetSource: "light",
  },
  "theme:dark": {
    allowedSources: ["reference", "dark"],
    packageName: "@openbitfun/theme-openbitfun",
    targetSource: "dark",
  },
  "theme:highContrastLight": {
    allowedSources: ["reference", "light", "highContrastLight"],
    packageName: "@openbitfun/theme-openbitfun",
    targetSource: "highContrastLight",
  },
  "theme:highContrastDark": {
    allowedSources: ["reference", "dark", "highContrastDark"],
    packageName: "@openbitfun/theme-openbitfun",
    targetSource: "highContrastDark",
  },
};

export function coerceTokenValue(type, input) {
  if (typeof input !== "string") {
    throw new Error("Token values must be strings.");
  }
  const value = input.trim();
  if (value.length === 0 || value.length > 512) {
    throw new Error("Token value must contain between 1 and 512 characters.");
  }

  switch (type) {
    case "color":
      if (!/^#[0-9a-f]{6}$/i.test(value)) {
        throw new Error("Color tokens require a six-digit hexadecimal value.");
      }
      return value.toLowerCase();
    case "dimension":
      if (!/^-?(?:\d+|\d*\.\d+)(?:px|rem|em|%)$/i.test(value)) {
        throw new Error("Dimension tokens require px, rem, em, or % units.");
      }
      return value;
    case "duration":
      if (!/^(?:\d+|\d*\.\d+)(?:ms|s)$/i.test(value)) {
        throw new Error("Duration tokens require ms or s units.");
      }
      return value;
    case "number": {
      const number = Number(value);
      if (!Number.isFinite(number)) {
        throw new Error("Number tokens require a finite numeric value.");
      }
      return number;
    }
    case "fontWeight": {
      const number = Number(value);
      if (!Number.isInteger(number) || number < 1 || number > 1000) {
        throw new Error("Font-weight tokens require an integer from 1 to 1000.");
      }
      return number;
    }
    case "cubicBezier":
      if (!/^cubic-bezier\(\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*\)$/i.test(value)) {
        throw new Error("Easing tokens require cubic-bezier(x1, y1, x2, y2)." );
      }
      return value;
    default:
      return value;
  }
}

export function setTokenDocumentValue(document, name, type, value) {
  const segments = name.split(".");
  if (
    segments.length === 0 ||
    segments.some((segment) => !/^[a-zA-Z0-9_-]+$/.test(segment))
  ) {
    throw new Error(`Invalid token name "${name}".`);
  }

  let current = document;
  for (const segment of segments.slice(0, -1)) {
    const child = current[segment];
    if (child === undefined) {
      current[segment] = {};
    } else if (!child || typeof child !== "object" || Array.isArray(child) || "$value" in child) {
      throw new Error(`Token path "${name}" conflicts with an existing value.`);
    }
    current = current[segment];
  }

  const leafName = segments.at(-1);
  const existing = current[leafName];
  if (existing !== undefined && (!existing || typeof existing !== "object" || Array.isArray(existing))) {
    throw new Error(`Token "${name}" is not stored as a token object.`);
  }
  current[leafName] = {
    ...(existing ?? {}),
    $type: existing?.$type ?? type,
    $value: value,
  };
  return document;
}

function sourcePaths(designSystemDirectory) {
  const systemDirectory = path.join(
    designSystemDirectory,
    "packages",
    "design-tokens",
    "src",
  );
  const themeDirectory = path.join(
    designSystemDirectory,
    "packages",
    "theme-openbitfun",
    "src",
  );
  return {
    compact: path.join(systemDirectory, "density-compact.tokens.json"),
    dark: path.join(themeDirectory, "dark.tokens.json"),
    highContrastDark: path.join(themeDirectory, "high-contrast-dark.tokens.json"),
    highContrastLight: path.join(themeDirectory, "high-contrast-light.tokens.json"),
    light: path.join(themeDirectory, "light.tokens.json"),
    reference: path.join(themeDirectory, "reference.tokens.json"),
    system: path.join(systemDirectory, "system.tokens.json"),
    touch: path.join(systemDirectory, "density-touch.tokens.json"),
  };
}

async function readSourceDocuments(designSystemDirectory) {
  const paths = sourcePaths(designSystemDirectory);
  const sourceEntries = await Promise.all(
    Object.entries(paths).map(async ([name, filePath]) => [
      name,
      {
        document: JSON.parse(await readFile(filePath, "utf8")),
        filePath,
      },
    ]),
  );
  return Object.fromEntries(sourceEntries);
}

function validateChange(change) {
  if (!change || typeof change !== "object" || Array.isArray(change)) {
    throw new Error("Every token change must be an object.");
  }
  for (const field of ["collection", "mode", "name", "value"]) {
    if (typeof change[field] !== "string") {
      throw new Error(`Token change field "${field}" must be a string.`);
    }
  }
}

async function applySourceChanges(designSystemDirectory, changes) {
  if (!Array.isArray(changes) || changes.length === 0 || changes.length > 200) {
    throw new Error("Supply between 1 and 200 token changes.");
  }

  const sources = await readSourceDocuments(designSystemDirectory);
  const affectedSources = new Set();
  const affectedPackages = new Set();

  for (const change of changes) {
    validateChange(change);
    const targetKey = `${change.collection}:${change.mode}`;
    const target = targetDefinitions[targetKey];
    if (!target) {
      throw new Error(`Unsupported token target "${targetKey}".`);
    }
    if (change.collection === "theme" && !change.name.startsWith("color.")) {
      throw new Error("Only public semantic color tokens can be written to theme sources.");
    }
    if (change.collection === "system" && change.name.startsWith("color.")) {
      throw new Error("Color tokens do not belong to the system-token collection.");
    }

    const allowedDocument = mergeTokenDocuments(
      ...target.allowedSources.map((sourceName) => sources[sourceName].document),
    );
    const definition = collectTokenDefinitions(allowedDocument).get(change.name);
    if (!definition) {
      throw new Error(`Unknown ${change.collection} token "${change.name}".`);
    }
    const nextValue = coerceTokenValue(definition.type, change.value);
    setTokenDocumentValue(
      sources[target.targetSource].document,
      change.name,
      definition.type,
      nextValue,
    );
    affectedSources.add(target.targetSource);
    affectedPackages.add(target.packageName);
  }

  const originals = new Map(
    [...affectedSources].map((sourceName) => [
      sources[sourceName].filePath,
      null,
    ]),
  );
  for (const filePath of originals.keys()) {
    originals.set(filePath, await readFile(filePath, "utf8"));
  }

  const pnpmExecutable = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
  const runPackageCommand = (packageName, command) => execFileAsync(
    pnpmExecutable,
    ["--filter", packageName, "run", command],
    { cwd: designSystemDirectory, windowsHide: true },
  );

  try {
    await Promise.all(
      [...affectedSources].map((sourceName) => writeFile(
        sources[sourceName].filePath,
        `${JSON.stringify(sources[sourceName].document, null, 2)}\n`,
      )),
    );
    for (const packageName of affectedPackages) {
      await runPackageCommand(packageName, "build");
      await runPackageCommand(packageName, "test");
    }
  } catch (error) {
    await Promise.all(
      [...originals].map(([filePath, source]) => writeFile(filePath, source)),
    );
    for (const packageName of affectedPackages) {
      await runPackageCommand(packageName, "build").catch(() => undefined);
    }
    throw error;
  }

  return changes.length;
}

async function readRequestBody(request) {
  let body = "";
  for await (const chunk of request) {
    body += chunk.toString("utf8");
    if (Buffer.byteLength(body, "utf8") > MAX_BODY_BYTES) {
      throw new Error("Token change request is too large.");
    }
  }
  return JSON.parse(body);
}

function isAllowedOrigin(request) {
  const origin = request.headers.origin;
  if (!origin) {
    return true;
  }
  try {
    const url = new URL(origin);
    return (
      (url.hostname === "127.0.0.1" || url.hostname === "localhost") &&
      url.port === "4178"
    );
  } catch {
    return false;
  }
}

function sendJson(response, statusCode, payload) {
  response.statusCode = statusCode;
  response.setHeader("Content-Type", "application/json; charset=utf-8");
  response.setHeader("Cache-Control", "no-store");
  response.end(JSON.stringify(payload));
}

export function createTokenAuthoringPlugin({ designSystemDirectory }) {
  let writeQueue = Promise.resolve();

  return {
    name: "openbitfun-token-authoring",
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
        if (requestUrl.pathname !== ENDPOINT) {
          next();
          return;
        }
        if (!isAllowedOrigin(request)) {
          sendJson(response, 403, { error: "Token source access is limited to the local Design Lab." });
          return;
        }
        if (request.method === "GET") {
          sendJson(response, 200, {
            collections: ["system", "theme"],
            strategy: "validated-source-writeback",
            writable: true,
          });
          return;
        }
        if (request.method !== "PUT") {
          sendJson(response, 405, { error: "Method not allowed." });
          return;
        }
        if (!request.headers["content-type"]?.startsWith("application/json")) {
          sendJson(response, 415, { error: "Token changes require application/json." });
          return;
        }

        try {
          const payload = await readRequestBody(request);
          const operation = writeQueue.then(() =>
            applySourceChanges(designSystemDirectory, payload.changes)
          );
          writeQueue = operation.catch(() => undefined);
          const saved = await operation;
          sendJson(response, 200, { saved });
          setTimeout(() => server.ws.send({ type: "full-reload" }), 80);
        } catch (error) {
          const message = error instanceof Error ? error.message : "Token source update failed.";
          server.config.logger.error(`Token source update failed: ${message}`);
          sendJson(response, 400, { error: message });
        }
      });
    },
  };
}

