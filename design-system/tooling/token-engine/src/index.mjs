const TOKEN_REFERENCE_PATTERN = /\{([a-zA-Z0-9_.-]+)\}/g;
const EXACT_TOKEN_REFERENCE_PATTERN = /^\{([a-zA-Z0-9_.-]+)\}$/;

function isPlainObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function cloneValue(value) {
  if (Array.isArray(value)) {
    return value.map(cloneValue);
  }

  if (isPlainObject(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, nestedValue]) => [key, cloneValue(nestedValue)]),
    );
  }

  return value;
}

function mergeObjects(base, override) {
  const result = cloneValue(base);

  for (const [key, value] of Object.entries(override)) {
    if (
      isPlainObject(value) &&
      isPlainObject(result[key]) &&
      !("$value" in value) &&
      !("$value" in result[key])
    ) {
      result[key] = mergeObjects(result[key], value);
    } else {
      result[key] = cloneValue(value);
    }
  }

  return result;
}

export function mergeTokenDocuments(...documents) {
  return documents.reduce(
    (result, document) => mergeObjects(result, document),
    {},
  );
}

export function collectTokenDefinitions(document) {
  const definitions = new Map();

  function visit(node, path, inheritedType) {
    if (!isPlainObject(node)) {
      throw new Error(`Token group "${path.join(".") || "<root>"}" must be an object.`);
    }

    const currentType = typeof node.$type === "string" ? node.$type : inheritedType;

    if ("$value" in node) {
      if (path.length === 0) {
        throw new Error("A token value cannot live at the document root.");
      }

      const name = path.join(".");
      definitions.set(name, {
        description: typeof node.$description === "string" ? node.$description : undefined,
        type: currentType,
        value: cloneValue(node.$value),
      });
      return;
    }

    for (const [key, value] of Object.entries(node)) {
      if (key.startsWith("$")) {
        continue;
      }
      visit(value, [...path, key], currentType);
    }
  }

  visit(document, [], undefined);
  return definitions;
}

function inferTokenType(value) {
  if (typeof value === "number") {
    return "number";
  }
  if (typeof value === "boolean") {
    return "boolean";
  }
  return "string";
}

export function toCssValue(value, type) {
  if (typeof value === "string" || typeof value === "number") {
    return String(value);
  }

  if (typeof value === "boolean") {
    return value ? "1" : "0";
  }

  if (type === "dimension" && isPlainObject(value)) {
    if (typeof value.value === "number" && typeof value.unit === "string") {
      return `${value.value}${value.unit}`;
    }
  }

  throw new Error(`Token value of type "${type ?? "unknown"}" cannot be emitted as CSS.`);
}

export function resolveTokens(document) {
  const definitions = collectTokenDefinitions(document);
  const resolved = new Map();
  const resolving = [];

  function resolveToken(name) {
    const cached = resolved.get(name);
    if (cached) {
      return cached;
    }

    const definition = definitions.get(name);
    if (!definition) {
      throw new Error(`Unknown token reference "${name}".`);
    }

    if (resolving.includes(name)) {
      throw new Error(`Circular token reference: ${[...resolving, name].join(" -> ")}.`);
    }

    resolving.push(name);

    let resolvedValue;
    let resolvedType = definition.type;

    if (typeof definition.value === "string") {
      const exactReference = definition.value.match(EXACT_TOKEN_REFERENCE_PATTERN);
      if (exactReference) {
        const referencedToken = resolveToken(exactReference[1]);
        resolvedValue = cloneValue(referencedToken.value);
        resolvedType ??= referencedToken.type;
      } else {
        resolvedValue = definition.value.replace(
          TOKEN_REFERENCE_PATTERN,
          (_match, referencedName) => {
            const referencedToken = resolveToken(referencedName);
            return toCssValue(referencedToken.value, referencedToken.type);
          },
        );
      }
    } else {
      resolvedValue = cloneValue(definition.value);
    }

    const token = {
      description: definition.description,
      sourceValue: cloneValue(definition.value),
      type: resolvedType ?? inferTokenType(resolvedValue),
      value: resolvedValue,
    };

    resolving.pop();
    resolved.set(name, token);
    return token;
  }

  for (const name of definitions.keys()) {
    resolveToken(name);
  }

  return Object.fromEntries([...resolved.entries()].sort(([left], [right]) => left.localeCompare(right)));
}

export function tokenNameToCssVariable(name, prefix = "openbitfun") {
  const normalizedName = name
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/[._\s]+/g, "-")
    .replace(/[^a-zA-Z0-9-]/g, "-")
    .replace(/-+/g, "-")
    .toLowerCase();

  return `--${prefix}-${normalizedName}`;
}

export function diffResolvedTokens(baseTokens, nextTokens) {
  return Object.fromEntries(
    Object.entries(nextTokens).filter(([name, token]) => {
      const baseToken = baseTokens[name];
      return !baseToken || JSON.stringify(baseToken.value) !== JSON.stringify(token.value);
    }),
  );
}

export function renderCss(tokens, options = {}) {
  const {
    layer = "openbitfun.tokens",
    prefix = "openbitfun",
    preserveReferences = false,
    selector = ":root",
  } = options;

  function renderTokenValue(token) {
    if (!preserveReferences || typeof token.sourceValue !== "string") {
      return toCssValue(token.value, token.type);
    }

    return token.sourceValue.replace(
      TOKEN_REFERENCE_PATTERN,
      (_match, referencedName) => `var(${tokenNameToCssVariable(referencedName, prefix)})`,
    );
  }

  const declarations = Object.entries(tokens)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(
      ([name, token]) =>
        `    ${tokenNameToCssVariable(name, prefix)}: ${renderTokenValue(token)};`,
    )
    .join("\n");

  return `@layer ${layer} {\n  ${selector} {\n${declarations}\n  }\n}\n`;
}

export function createTokenArtifacts(tokens, options = {}) {
  const { exportName = "tokens", prefix = "openbitfun" } = options;
  const values = Object.fromEntries(
    Object.entries(tokens).map(([name, token]) => [name, token.value]),
  );
  const cssVariables = Object.fromEntries(
    Object.keys(tokens).map((name) => [name, tokenNameToCssVariable(name, prefix)]),
  );
  const names = Object.keys(values);
  const tokenNameUnion = names.length > 0
    ? names.map((name) => JSON.stringify(name)).join(" | ")
    : "never";

  return {
    cssVariables,
    javascript: [
      `export const ${exportName} = Object.freeze(${JSON.stringify(values, null, 2)});`,
      `export const cssVariables = Object.freeze(${JSON.stringify(cssVariables, null, 2)});`,
      "",
    ].join("\n"),
    json: `${JSON.stringify(values, null, 2)}\n`,
    typescript: [
      `export type TokenName = ${tokenNameUnion};`,
      `export declare const ${exportName}: Readonly<Record<TokenName, string | number | boolean>>;`,
      "export declare const cssVariables: Readonly<Record<TokenName, `--openbitfun-${string}`>>;",
      "",
    ].join("\n"),
    values,
  };
}

export function createTokenCatalog(modes, options = {}) {
  const {
    defaultMode = Object.keys(modes)[0],
    include = () => true,
    prefix = "openbitfun",
  } = options;
  const modeNames = Object.keys(modes);
  const defaultTokens = modes[defaultMode];

  if (!defaultMode || !defaultTokens) {
    throw new Error(`Unknown default token mode "${defaultMode ?? "<missing>"}".`);
  }

  return Object.entries(defaultTokens)
    .filter(([name, token]) => include(name, token))
    .map(([name, token]) => {
      const values = Object.fromEntries(
        modeNames.map((modeName) => {
          const modeToken = modes[modeName]?.[name];
          if (!modeToken) {
            throw new Error(`Token mode "${modeName}" is missing "${name}".`);
          }
          if (modeToken.type !== token.type) {
            throw new Error(
              `Token "${name}" changes type from "${token.type}" to "${modeToken.type}" in mode "${modeName}".`,
            );
          }
          return [modeName, toCssValue(modeToken.value, modeToken.type)];
        }),
      );

      return {
        category: name.split(".")[0],
        cssVariable: tokenNameToCssVariable(name, prefix),
        description: token.description,
        name,
        type: token.type,
        values,
      };
    });
}

export function assertRequiredTokens(tokens, requiredNames) {
  const missing = requiredNames.filter((name) => !(name in tokens));
  if (missing.length > 0) {
    throw new Error(`Missing required tokens: ${missing.join(", ")}.`);
  }
}
