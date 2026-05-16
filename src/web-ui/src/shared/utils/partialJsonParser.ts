 

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
