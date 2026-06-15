 

import { parse, Allow } from 'partial-json';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PartialJsonParser');

function objectParams(value: unknown): Record<string, any> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, any>
    : {};
}

export function parsePartialJson(jsonStr: unknown): Record<string, any> {
  if (typeof jsonStr !== 'string' || jsonStr.trim() === '') {
    return {};
  }

  try {
    return objectParams(JSON.parse(jsonStr));
  } catch {
    try {
      const result = parse(jsonStr, Allow.ALL);
      return objectParams(result);
    } catch (error) {
      log.warn('Failed to parse partial JSON', error);
      return {};
    }
  }
}

export function isFieldComplete(jsonStr: string, fieldName: string): boolean {
  const parsed = parsePartialJson(jsonStr);
  return fieldName in parsed && parsed[fieldName] !== null && parsed[fieldName] !== undefined;
}

export function getFieldValue<T = any>(
  jsonStr: string,
  fieldName: string,
  defaultValue?: T,
): T | undefined {
  const parsed = parsePartialJson(jsonStr);
  return parsed[fieldName] !== undefined ? parsed[fieldName] : defaultValue;
}

export function getFirstAvailableField<T = any>(
  jsonStr: string,
  fieldNames: string[],
  defaultValue?: T,
): T | undefined {
  const parsed = parsePartialJson(jsonStr);

  for (const fieldName of fieldNames) {
    if (fieldName in parsed && parsed[fieldName] !== null && parsed[fieldName] !== undefined) {
      return parsed[fieldName];
    }
  }

  return defaultValue;
}

const DEFAULT_FILE_PATH_FIELD_NAMES = [
  'file_path',
  'filePath',
  'filepath',
  'target_file',
  'targetFile',
  'path',
  'filename',
] as const;

function decodeJsonStringFragment(raw: string): string {
  try {
    return JSON.parse(
      `"${raw.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`,
    ) as string;
  } catch {
    return raw.replace(/\\"/g, '"').replace(/\\\\/g, '\\');
  }
}

function extractQuotedFieldFromRegion(
  region: string,
  fieldName: string,
): string {
  const completePattern = new RegExp(
    `"${fieldName}"\\s*:\\s*"((?:\\\\.|[^"\\\\])*)"`,
  );
  const completeMatch = region.match(completePattern);
  if (completeMatch?.[1]) {
    return decodeJsonStringFragment(completeMatch[1]);
  }

  const partialPattern = new RegExp(
    `"${fieldName}"\\s*:\\s*"((?:\\\\.|[^"\\\\])*)$`,
  );
  const partialMatch = region.match(partialPattern);
  if (partialMatch?.[1]) {
    return decodeJsonStringFragment(partialMatch[1]);
  }

  return '';
}

/**
 * Best-effort file_path extraction from a streaming Write tool JSON buffer.
 *
 * Scans only the prefix before `"content":` so we do not false-match paths
 * embedded inside the file body string when content is streamed first.
 */
export function extractFilePathFromJsonBuffer(
  jsonStr: unknown,
  fieldNames: readonly string[] = DEFAULT_FILE_PATH_FIELD_NAMES,
): string {
  if (typeof jsonStr !== 'string' || jsonStr.trim() === '') {
    return '';
  }

  const contentKeyMatch = jsonStr.match(/"content"\s*:/);
  const searchRegion =
    contentKeyMatch?.index !== undefined
      ? jsonStr.slice(0, contentKeyMatch.index)
      : jsonStr;

  for (const fieldName of fieldNames) {
    const value = extractQuotedFieldFromRegion(searchRegion, fieldName);
    if (value) {
      return value;
    }
  }

  return '';
}
