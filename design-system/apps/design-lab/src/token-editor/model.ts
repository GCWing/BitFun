import type { SystemTokenMode } from "@openbitfun/design-tokens";
import type { ThemeDataName } from "@openbitfun/theme-openbitfun";
import type { TokenOverrides } from "@openbitfun/ui";
import {
  editableTokenCatalog,
  type EditableToken,
  type EditableTokenMode,
  type TokenCollection,
} from "./catalog";

export interface TokenEditorContext {
  density: SystemTokenMode;
  theme: ThemeDataName;
}

export interface TokenSourceChange {
  collection: TokenCollection;
  mode: EditableTokenMode;
  name: string;
  value: string;
}

export type TokenDrafts = Readonly<Record<string, string>>;

const STORAGE_KEY = "openbitfun.design-lab.token-drafts.v1";

export function getActiveTokenMode(
  token: EditableToken,
  context: TokenEditorContext,
): EditableTokenMode {
  return token.collection === "system" ? context.density : context.theme;
}

export function getTokenDraftKey(
  collection: TokenCollection,
  mode: EditableTokenMode,
  name: string,
): string {
  return `${collection}:${mode}:${name}`;
}

export function getTokenValue(
  token: EditableToken,
  context: TokenEditorContext,
  drafts: TokenDrafts,
): string {
  const mode = getActiveTokenMode(token, context);
  const key = getTokenDraftKey(token.collection, mode, token.name);
  return drafts[key] ?? token.values[mode] ?? "";
}

export function buildActiveTokenOverrides(
  context: TokenEditorContext,
  drafts: TokenDrafts,
): TokenOverrides {
  const overrides: TokenOverrides = {};

  for (const token of editableTokenCatalog) {
    const mode = getActiveTokenMode(token, context);
    const key = getTokenDraftKey(token.collection, mode, token.name);
    const value = drafts[key];
    if (value !== undefined) {
      overrides[token.cssVariable] = value;
    }
  }

  return overrides;
}

export function serializeTokenChanges(drafts: TokenDrafts): TokenSourceChange[] {
  const changes: TokenSourceChange[] = [];

  for (const token of editableTokenCatalog) {
    for (const mode of Object.keys(token.values) as EditableTokenMode[]) {
      const key = getTokenDraftKey(token.collection, mode, token.name);
      const value = drafts[key];
      if (value !== undefined) {
        changes.push({
          collection: token.collection,
          mode,
          name: token.name,
          value,
        });
      }
    }
  }

  return changes;
}

export function validateTokenValue(type: string, value: string): string | undefined {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return "Value is required.";
  }
  if (trimmed.length > 512) {
    return "Value is too long.";
  }

  switch (type) {
    case "color":
      return /^#[0-9a-f]{6}$/i.test(trimmed)
        ? undefined
        : "Use a six-digit hexadecimal color.";
    case "dimension":
      return /^-?(?:\d+|\d*\.\d+)(?:px|rem|em|%)$/i.test(trimmed)
        ? undefined
        : "Use a number with px, rem, em, or %.";
    case "duration":
      return /^(?:\d+|\d*\.\d+)(?:ms|s)$/i.test(trimmed)
        ? undefined
        : "Use a positive duration in ms or s.";
    case "number":
      return Number.isFinite(Number(trimmed)) ? undefined : "Use a finite number.";
    case "fontWeight": {
      const weight = Number(trimmed);
      return Number.isInteger(weight) && weight >= 1 && weight <= 1000
        ? undefined
        : "Use an integer from 1 to 1000.";
    }
    case "cubicBezier":
      return /^cubic-bezier\(\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*\)$/i.test(trimmed)
        ? undefined
        : "Use cubic-bezier(x1, y1, x2, y2).";
    default:
      return undefined;
  }
}

export function loadTokenDrafts(): TokenDrafts {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (!stored) {
      return {};
    }
    const parsed = JSON.parse(stored) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    return Object.fromEntries(
      Object.entries(parsed).filter((entry): entry is [string, string] =>
        typeof entry[1] === "string"
      ),
    );
  } catch {
    return {};
  }
}

export function persistTokenDrafts(drafts: TokenDrafts): void {
  if (Object.keys(drafts).length === 0) {
    window.localStorage.removeItem(STORAGE_KEY);
    return;
  }
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(drafts));
}

