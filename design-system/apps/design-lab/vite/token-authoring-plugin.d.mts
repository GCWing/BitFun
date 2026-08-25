import type { Plugin } from "vite";

export function createTokenAuthoringPlugin(options: {
  designSystemDirectory: string;
}): Plugin;

export function coerceTokenValue(type: string, input: string): string | number;

export function setTokenDocumentValue(
  document: Record<string, unknown>,
  name: string,
  type: string,
  value: string | number,
): Record<string, unknown>;

