/*
 * Copyright (c) 2026 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

const JSCRASH_FILE_RE = /\bjscrash-[A-Za-z0-9._-]+\.log\b/g;
const JSCRASH_NAME_RE = /^jscrash-(.+)-(\d{10,17})\.log$/i;

export function extractJscrashFaultlogNames(text) {
  const matches = text.match(JSCRASH_FILE_RE);
  if (!matches?.length) {
    return [];
  }

  return [...new Set(matches.map((item) => item.trim()))];
}

function parseTrailingTimestampMs(raw) {
  const n = Number(raw);
  if (!Number.isFinite(n)) {
    return null;
  }

  if (raw.length <= 11) {
    return n * 1000;
  }

  return n;
}

function extractBundlePartFromBody(body) {
  const uidSep = body.lastIndexOf('-');
  if (uidSep <= 0) {
    return body;
  }

  const maybeUid = body.slice(uidSep + 1);
  if (/^\d{5,}$/.test(maybeUid)) {
    return body.slice(0, uidSep);
  }

  return body;
}

function parseJscrashFaultlogName(name) {
  const match = JSCRASH_NAME_RE.exec(name);
  if (!match?.[1] || !match[2]) {
    return null;
  }

  return {
    bundlePart: extractBundlePartFromBody(match[1]),
    timestampMs: parseTrailingTimestampMs(match[2]),
  };
}

export function parseFaultlogEntry(name) {
  const parsed = parseJscrashFaultlogName(name);
  return {
    name,
    timestampMs: parsed?.timestampMs ?? null,
  };
}

function normalizeBundleToken(bundleName) {
  return bundleName.trim().toLowerCase();
}

export function faultlogMatchesBundle(name, bundleName) {
  const token = normalizeBundleToken(bundleName);
  if (!token) {
    return true;
  }

  const parsed = parseJscrashFaultlogName(name);
  if (parsed?.bundlePart) {
    return parsed.bundlePart.toLowerCase() === token;
  }

  return name.toLowerCase().startsWith(`jscrash-${token}-`);
}

export function filterFaultlogsByBundle(names, bundleName) {
  if (!bundleName.trim()) {
    return names;
  }

  const filtered = names.filter((name) => faultlogMatchesBundle(name, bundleName));
  return filtered.length ? filtered : names;
}

export function sortFaultlogsByRecency(names) {
  const withMeta = names.map((name) => {
    const entry = parseFaultlogEntry(name);
    return { name, timestampMs: entry.timestampMs };
  });

  withMeta.sort((a, b) => {
    const ta = a.timestampMs;
    const tb = b.timestampMs;
    if (ta !== null && tb !== null && ta !== tb) {
      return tb - ta;
    }
    if (ta !== null && tb === null) {
      return -1;
    }
    if (ta === null && tb !== null) {
      return 1;
    }

    return b.name.localeCompare(a.name);
  });

  return withMeta.map((item) => item.name);
}

export function selectWithinMaxAge(names, maxAgeMinutes, nowMs) {
  if (!Number.isFinite(maxAgeMinutes) || maxAgeMinutes <= 0) {
    return names;
  }

  const windowMs = maxAgeMinutes * 60 * 1000;
  const filtered = names.filter((name) => {
    const ts = parseFaultlogEntry(name).timestampMs;
    if (ts === null) {
      return true;
    }

    return nowMs - ts <= windowMs;
  });

  return filtered.length ? filtered : names;
}
