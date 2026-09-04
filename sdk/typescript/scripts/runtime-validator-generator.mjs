import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import ts from "typescript";

export async function generateRuntimeValidators(
  directory,
  { typeImportPath = "./index.js" } = {},
) {
  const aliases = await readAliases(directory);
  const names = [...aliases.keys()].sort();

  for (const [name, declaration] of aliases) {
    validateSupportedType(declaration.type, aliases, name);
  }

  const header = [
    "// GENERATED CODE! DO NOT MODIFY BY HAND!",
    "// Source: openbitfun-sdk-host Rust protocol types via ts-rs.",
    "",
    `import type { ${names.join(", ")} } from ${JSON.stringify(typeImportPath)};`,
    "",
  ];
  const functions = names.flatMap((name) => {
    const declaration = aliases.get(name);
    const expression = emitTypeCheck(declaration.type, "value", aliases, name);
    return [
      `export function is${name}(value: unknown): value is ${name} {`,
      `  return ${expression};`,
      "}",
      "",
    ];
  });
  const helpers = [
    "function isRecord(value: unknown): value is Record<string, unknown> {",
    '  return typeof value === "object" && value !== null && !Array.isArray(value);',
    "}",
    "",
    "function isJsonValue(value: unknown): boolean {",
    '  if (value === null || typeof value === "string" || typeof value === "boolean") return true;',
    '  if (typeof value === "number") return Number.isFinite(value);',
    "  if (Array.isArray(value)) return value.every(isJsonValue);",
    "  return isRecord(value) && Object.values(value).every(isJsonValue);",
    "}",
    "",
    "function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {",
    "  const keys = Object.keys(value);",
    "  return keys.length <= allowed.length && keys.every((key) => allowed.includes(key));",
    "}",
    "",
  ];

  return [...header, ...functions, ...helpers].join("\n");
}

async function readAliases(directory) {
  const files = (await readdir(directory))
    .filter(
      (file) =>
        file.endsWith(".ts") &&
        file !== "index.ts" &&
        file !== "validators.ts",
    )
    .sort();
  const aliases = new Map();

  for (const file of files) {
    const sourceText = await readFile(join(directory, file), "utf8");
    const source = ts.createSourceFile(
      file,
      sourceText,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.TS,
    );
    if (source.parseDiagnostics.length > 0) {
      throw new Error(`cannot parse generated wire type ${file}`);
    }
    const unexpected = source.statements.find(
      (statement) =>
        !ts.isImportDeclaration(statement) &&
        !ts.isTypeAliasDeclaration(statement),
    );
    if (unexpected !== undefined) {
      throw new Error(
        `generated wire file ${file} contains unsupported declaration ${ts.SyntaxKind[unexpected.kind]}`,
      );
    }
    const declarations = source.statements.filter(ts.isTypeAliasDeclaration);
    if (declarations.length !== 1) {
      throw new Error(
        `generated wire file ${file} must contain exactly one type alias`,
      );
    }
    const declaration = declarations[0];
    const expectedName = file.slice(0, -3);
    if (declaration.name.text !== expectedName) {
      throw new Error(
        `generated wire file ${file} must declare ${expectedName}`,
      );
    }
    if (
      !declaration.modifiers?.some(
        (modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword,
      )
    ) {
      throw new Error(`generated wire type ${expectedName} must be exported`);
    }
    aliases.set(expectedName, declaration);
  }
  return aliases;
}

function validateSupportedType(node, aliases, owner) {
  emitTypeCheck(node, "value", aliases, owner);
}

function emitTypeCheck(node, value, aliases, owner) {
  if (ts.isParenthesizedTypeNode(node)) {
    return emitTypeCheck(node.type, value, aliases, owner);
  }
  if (ts.isUnionTypeNode(node)) {
    return `(${node.types
      .map((member) => emitTypeCheck(member, value, aliases, owner))
      .join(" || ")})`;
  }
  if (ts.isLiteralTypeNode(node)) {
    if (node.literal.kind === ts.SyntaxKind.NullKeyword) {
      return `${value} === null`;
    }
    if (
      ts.isStringLiteral(node.literal) ||
      ts.isNumericLiteral(node.literal)
    ) {
      return `${value} === ${JSON.stringify(
        ts.isStringLiteral(node.literal)
          ? node.literal.text
          : Number(node.literal.text),
      )}`;
    }
    if (node.literal.kind === ts.SyntaxKind.TrueKeyword) {
      return `${value} === true`;
    }
    if (node.literal.kind === ts.SyntaxKind.FalseKeyword) {
      return `${value} === false`;
    }
    throw unsupported(owner, node, "literal");
  }
  if (node.kind === ts.SyntaxKind.StringKeyword) {
    return `typeof ${value} === "string"`;
  }
  if (node.kind === ts.SyntaxKind.NumberKeyword) {
    return `(typeof ${value} === "number" && Number.isFinite(${value}))`;
  }
  if (node.kind === ts.SyntaxKind.BooleanKeyword) {
    return `typeof ${value} === "boolean"`;
  }
  if (node.kind === ts.SyntaxKind.UnknownKeyword) {
    return `isJsonValue(${value})`;
  }
  if (ts.isArrayTypeNode(node)) {
    const item = emitTypeCheck(node.elementType, "item", aliases, owner);
    return `(Array.isArray(${value}) && ${value}.every((item: unknown) => ${item}))`;
  }
  if (ts.isTypeLiteralNode(node)) {
    return emitObjectCheck(node, value, aliases, owner);
  }
  if (ts.isTypeReferenceNode(node)) {
    if (!ts.isIdentifier(node.typeName)) {
      throw unsupported(owner, node, "qualified type reference");
    }
    const name = node.typeName.text;
    if (aliases.has(name)) {
      if (node.typeArguments !== undefined) {
        throw unsupported(owner, node, `generic wire type ${name}`);
      }
      return `is${name}(${value})`;
    }
    if (name === "Array" || name === "ReadonlyArray") {
      if (node.typeArguments?.length !== 1) {
        throw unsupported(owner, node, `${name} arity`);
      }
      const item = emitTypeCheck(node.typeArguments[0], "item", aliases, owner);
      return `(Array.isArray(${value}) && ${value}.every((item: unknown) => ${item}))`;
    }
    if (name === "Record" && isEmptyRecord(node)) {
      return `(isRecord(${value}) && Object.keys(${value}).length === 0)`;
    }
    if (name === "Record" && isJsonRecord(node)) {
      return `(isRecord(${value}) && Object.values(${value}).every(isJsonValue))`;
    }
    throw new Error(
      `${owner}: unsupported type reference ${name}; runtime validator generation is fail-closed`,
    );
  }
  throw unsupported(owner, node, ts.SyntaxKind[node.kind] ?? "unknown syntax");
}

function emitObjectCheck(node, value, aliases, owner) {
  const properties = [];
  for (const member of node.members) {
    if (!ts.isPropertySignature(member) || member.type === undefined) {
      throw unsupported(owner, member, "object member");
    }
    properties.push({
      name: propertyName(member.name, owner),
      optional: member.questionToken !== undefined,
      type: member.type,
    });
  }
  const allowed = JSON.stringify(properties.map(({ name }) => name));
  const checks = properties.map(({ name, optional, type }) => {
    const access = `${value}[${JSON.stringify(name)}]`;
    const check = emitTypeCheck(type, access, aliases, owner);
    return optional ? `(${access} === undefined || ${check})` : check;
  });
  return [
    `(isRecord(${value})`,
    `hasOnlyKeys(${value}, ${allowed})`,
    ...checks,
  ].join(" && ") + ")";
}

function propertyName(node, owner) {
  if (ts.isIdentifier(node) || ts.isStringLiteral(node)) {
    return node.text;
  }
  if (ts.isNumericLiteral(node)) {
    return node.text;
  }
  throw unsupported(owner, node, "computed property name");
}

function isEmptyRecord(node) {
  if (node.typeArguments?.length !== 2) {
    return false;
  }
  return (
    node.typeArguments[0].kind === ts.SyntaxKind.SymbolKeyword &&
    node.typeArguments[1].kind === ts.SyntaxKind.NeverKeyword
  );
}

function isJsonRecord(node) {
  return (
    node.typeArguments?.length === 2 &&
    node.typeArguments[0].kind === ts.SyntaxKind.StringKeyword &&
    node.typeArguments[1].kind === ts.SyntaxKind.UnknownKeyword
  );
}

function unsupported(owner, node, detail) {
  return new Error(
    `${owner}: unsupported ${detail} (${ts.SyntaxKind[node.kind] ?? node.kind}); runtime validator generation is fail-closed`,
  );
}
