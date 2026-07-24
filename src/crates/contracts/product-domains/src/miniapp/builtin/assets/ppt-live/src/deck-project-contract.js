export class DeckProjectContractError extends Error {
  constructor(diagnostic) {
    super(`[${diagnostic.code}] ${diagnostic.summary}`);
    this.name = 'DeckProjectContractError';
    this.diagnostic = diagnostic;
  }
}

function contractError(code, summary, continuationPrompt, details = {}) {
  return new DeckProjectContractError({
    code,
    summary,
    continuationPrompt,
    ...details,
  });
}

function missingSlideFilesDiagnostic(missingPaths) {
  return {
    code: 'missing_slide_files',
    summary: `Missing or incomplete slide files: ${missingPaths.join(', ')}`,
    continuationPrompt: `只补写这些缺失或不完整页面：${missingPaths.join('、')}。保留其他页面不变；补齐后再把状态确认为 complete 并执行一次有界检查。`,
    missingPaths,
  };
}

const defaultSleep = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs));

async function readVisibleFileWithRetry(readFile, relPath, {
  maxAttempts = 6,
  delayMs = 120,
  sleep = defaultSleep,
  accept,
} = {}) {
  let lastValue = '';
  let lastError = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      lastValue = String(await readFile(relPath) || '');
      if (accept(lastValue)) return lastValue;
    } catch (error) {
      lastError = error;
    }
    if (attempt < maxAttempts) await sleep(delayMs);
  }
  return { lastValue, lastError };
}

export function createDeckProjectSkeleton({
  title = '',
  language = '',
  style = {},
} = {}) {
  return {
    status: 'planning',
    title,
    language,
    outline: [],
    slide_order: [],
    style,
    assumptions: [],
  };
}

export function createDeckProjectSeed({
  hasExistingDeck = false,
  title = '',
  language = '',
  style = {},
  slides = [],
  serializeElementSlide = null,
} = {}) {
  if (!hasExistingDeck) {
    return {
      plan: createDeckProjectSkeleton({ title, language, style }),
      slideFiles: [],
    };
  }
  const outline = slides.map((slide, index) => {
    const slideId = `slide-${String(index + 1).padStart(2, '0')}`;
    return {
      id: slideId,
      title: String(slide?.title || ''),
      bullets: [],
      slide_id: slideId,
    };
  });
  const slideFiles = [];
  const missingPaths = [];
  slides.forEach((slide, index) => {
    const relPath = `slides/slide-${String(index + 1).padStart(2, '0')}.html`;
    let html = String(slide?.html || '');
    if (!isCompleteSlideHtml(html) && Array.isArray(slide?.elements) && serializeElementSlide) {
      try {
        html = String(serializeElementSlide(slide) || '');
      } catch {
        html = '';
      }
    }
    if (isCompleteSlideHtml(html)) slideFiles.push({ relPath, html: html.trim() });
    else missingPaths.push(relPath);
  });
  const diagnostic = missingPaths.length ? missingSlideFilesDiagnostic(missingPaths) : null;
  return {
    plan: {
      status: diagnostic ? 'planning' : 'complete',
      title,
      language,
      outline,
      slide_order: outline.map((item) => item.slide_id),
      style,
      assumptions: [],
    },
    slideFiles,
    diagnostic,
  };
}

function seedPersistenceError(code, phase, missingPaths) {
  return new DeckProjectContractError({
    code,
    phase,
    summary: 'Deck project seed persistence failed.',
    continuationPrompt: `请在同一会话中补写这些 deck 项目路径：${missingPaths.join('、')}，保留已成功写入的文件并继续生成。`,
    missingPaths,
  });
}

function shouldPersistSeedProjectJson(seed) {
  const slideFiles = Array.isArray(seed?.slideFiles) ? seed.slideFiles : [];
  if (slideFiles.length > 0) return true;
  const outline = seed?.plan?.outline;
  if (Array.isArray(outline) && outline.length > 0) return true;
  if (String(seed?.plan?.status || '') === 'complete') return true;
  // Empty new-deck skeleton: skip project.json so the agent can Write it
  // fresh without a Read-before-Write round trip on a host-seeded file.
  return false;
}

export async function persistDeckProjectSeed(fs, projectDir, seed) {
  try {
    await fs.mkdir(`${projectDir}/slides`, { recursive: true });
  } catch {
    throw seedPersistenceError('seed_fs_mkdir_failed', 'mkdir', ['slides']);
  }
  if (shouldPersistSeedProjectJson(seed)) {
    try {
      await fs.writeFile(`${projectDir}/project.json`, `${JSON.stringify(seed.plan, null, 2)}\n`);
    } catch {
      throw seedPersistenceError('seed_fs_write_failed', 'project-write', ['project.json']);
    }
  }
  for (const slideFile of seed.slideFiles || []) {
    try {
      await fs.writeFile(`${projectDir}/${slideFile.relPath}`, slideFile.html);
    } catch {
      throw seedPersistenceError('seed_fs_write_failed', 'slide-write', [slideFile.relPath]);
    }
  }
}

export function buildDeckRunRequestInput(baseInput, {
  sessionId = '',
  projectContractDiagnostic = null,
} = {}) {
  return {
    ...baseInput,
    ...(sessionId ? { continueAfterInterruption: true } : {}),
    ...(projectContractDiagnostic ? { projectContractDiagnostic } : {}),
  };
}

// ─── Tolerant project.json parsing ───────────────────────────────────────────
// Agent-written `project.json` frequently fails a strict JSON.parse: markdown
// fences, // or /* */ comments, trailing commas, or a truncated final write.
// Aborting the whole generation for those cases is unreasonable, so parsing
// falls back through a chain of conservative repairs. Contract validation
// (status/outline/slide_order/slide files) still runs afterwards, so a repair
// that loses required content surfaces as a targeted continuation diagnostic
// instead of silently succeeding.

/** Remove JS-style comments outside string literals. */
function stripJsonComments(text) {
  let output = '';
  let quote = null;
  for (let index = 0; index < text.length;) {
    const character = text[index];
    if (quote) {
      output += character;
      if (character === '\\' && index + 1 < text.length) {
        output += text[index + 1];
        index += 2;
        continue;
      }
      if (character === quote) quote = null;
      index += 1;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      index += 1;
      continue;
    }
    if (character === '/' && text[index + 1] === '/') {
      while (index < text.length && text[index] !== '\n') index += 1;
      continue;
    }
    if (character === '/' && text[index + 1] === '*') {
      const end = text.indexOf('*/', index + 2);
      index = end < 0 ? text.length : end + 2;
      continue;
    }
    output += character;
    index += 1;
  }
  return output;
}

/** Drop commas that directly precede a closing bracket (outside strings). */
function stripTrailingJsonCommas(text) {
  let output = '';
  let quote = null;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      output += character;
      if (character === '\\' && index + 1 < text.length) {
        output += text[index + 1];
        index += 1;
      } else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      continue;
    }
    if (character === ',') {
      let lookahead = index + 1;
      while (lookahead < text.length && /\s/.test(text[lookahead])) lookahead += 1;
      if (text[lookahead] === '}' || text[lookahead] === ']') continue;
    }
    output += character;
  }
  return output;
}

/**
 * Close a truncated JSON document: terminate an open string, drop trailing
 * incomplete fragments (dangling key/colon/comma), then append the closers
 * required by the open bracket stack. Returns null when the text does not
 * start an object.
 */
function closeTruncatedJson(text) {
  let quote = null;
  const stack = [];
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quote) {
      if (character === '\\') index += 1;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '{' || character === '[') stack.push(character);
    else if (character === '}' || character === ']') stack.pop();
  }
  if (!stack.length || stack[0] !== '{') return null;
  let trimmed = text.replace(/\s+$/, '');
  if (quote) trimmed += quote;
  // Drop trailing incomplete fragments: dangling comma/colon, or a lone key.
  for (;;) {
    trimmed = trimmed.replace(/\s+$/, '');
    if (/[,:]$/.test(trimmed)) {
      trimmed = trimmed.slice(0, -1);
      continue;
    }
    const bareKey = trimmed.match(/"[^"]*"$/);
    if (bareKey && /[{,]\s*"[^"]*"$/.test(trimmed)) {
      trimmed = trimmed.slice(0, trimmed.length - bareKey[0].length);
      continue;
    }
    break;
  }
  const closers = stack.map((open) => (open === '{' ? '}' : ']')).reverse().join('');
  return `${trimmed}${closers}`;
}

function parseJsonObjectCandidate(candidate) {
  try {
    const parsed = JSON.parse(candidate);
    if (parsed && !Array.isArray(parsed) && typeof parsed === 'object') return parsed;
  } catch {
    // Try the next candidate.
  }
  return null;
}

/**
 * Parse agent-written JSON tolerantly. `mode: 'lenient'` also attempts
 * truncated-document repair; `mode: 'clean'` only applies repairs that never
 * fabricate structure (fence/comment/trailing-comma stripping) so a file that
 * is still being written is not accepted early.
 */
export function parseJsonObjectTolerant(raw, { mode = 'lenient' } = {}) {
  const text = String(raw || '').replace(/^\uFEFF/, '').trim();
  if (!text) return null;
  const candidates = [text];
  const fenceStart = text.indexOf('{');
  const fenceEnd = text.lastIndexOf('}');
  if (fenceStart > 0 || (fenceEnd >= 0 && fenceEnd < text.length - 1)) {
    if (fenceStart >= 0 && fenceEnd > fenceStart) candidates.push(text.slice(fenceStart, fenceEnd + 1));
  }
  const bases = [...candidates];
  for (const base of bases) {
    const uncommented = stripJsonComments(base);
    if (uncommented !== base) candidates.push(uncommented);
    const withoutTrailing = stripTrailingJsonCommas(uncommented);
    if (withoutTrailing !== uncommented) candidates.push(withoutTrailing);
  }
  for (const candidate of candidates) {
    const parsed = parseJsonObjectCandidate(candidate);
    if (parsed) return parsed;
  }
  if (mode !== 'lenient') return null;
  // Truncated write: progressively shorten to the last complete fragment,
  // close the bracket stack, and accept only objects with real content.
  let fragment = stripTrailingJsonCommas(stripJsonComments(text));
  for (let attempt = 0; attempt < 32 && fragment; attempt += 1) {
    const closed = closeTruncatedJson(fragment);
    const parsed = closed ? parseJsonObjectCandidate(closed) : null;
    if (parsed && Object.keys(parsed).length > 0) return parsed;
    const cutAt = Math.max(fragment.lastIndexOf(','), fragment.lastIndexOf('},{'));
    if (cutAt <= 0) break;
    fragment = fragment.slice(0, cutAt);
  }
  return null;
}

function parseProjectJson(raw) {
  const plan = parseJsonObjectTolerant(raw, { mode: 'lenient' });
  if (plan) return plan;
  throw contractError(
    'invalid_project_json',
    '`project.json` is not valid JSON.',
    '修复 `project.json` JSON，使根值为对象；不要重写已有页面。修复后继续完成契约。',
  );
}

export async function readProjectPlanWithRetry(readFile, options = {}) {
  const { requireComplete = false } = options;
  const result = await readVisibleFileWithRetry(readFile, 'project.json', {
    ...options,
    accept: (raw) => {
      if (!raw.trim()) return false;
      // Accept cheap, structure-preserving repairs immediately (fences,
      // comments, trailing commas) but not truncated-document repair: a file
      // that is still being written must keep retrying instead of being
      // accepted with fabricated closers.
      const parsed = parseJsonObjectTolerant(raw, { mode: 'clean' });
      return Boolean(parsed) && (!requireComplete || parsed.status === 'complete');
    },
  });
  if (typeof result === 'string') return parseProjectJson(result);
  if (!result.lastValue.trim()) {
    throw contractError(
      'missing_project_json',
      '`project.json` is missing or empty.',
      '在工作区根目录创建 `project.json`，先写 status、outline 和 slide_order，再继续补写页面；不要重写已有页面。',
      { cause: String(result.lastError?.message || result.lastError || '') },
    );
  }
  return parseProjectJson(result.lastValue);
}

function validateCompletedPlan(plan) {
  if (plan.status !== 'complete') {
    throw contractError(
      'project_incomplete',
      '`project.json` has not declared a complete deck.',
      '继续当前计划：先完成 outline 和页面文件，确认所有引用页面存在后，再把 `project.json.status` 设为 `"complete"`。',
    );
  }
  if (!Array.isArray(plan.outline) || !plan.outline.length) {
    throw contractError(
      'invalid_project_contract',
      '`outline` must be a non-empty array.',
      '修复 `project.json`：先写非空 `outline`，每项提供唯一 `slide_id`，并让 `slide_order` 精确对应这些 ID。',
    );
  }
  if (!Array.isArray(plan.slide_order) || !plan.slide_order.length) {
    throw contractError(
      'invalid_project_contract',
      '`slide_order` must be a non-empty array.',
      '修复 `project.json`：让 `slide_order` 按展示顺序列出全部 `outline[].slide_id`。',
    );
  }

  const outlineIds = [];
  const outlineItemIds = new Set();
  for (const item of plan.outline) {
    const requiredFields = [
      ['id', typeof item?.id === 'string' && Boolean(item.id.trim())],
      ['title', typeof item?.title === 'string' && Boolean(item.title.trim())],
      ['bullets', Array.isArray(item?.bullets) && item.bullets.every((bullet) => typeof bullet === 'string')],
    ];
    const invalidField = requiredFields.find(([, valid]) => !valid)?.[0];
    if (invalidField) {
      throw contractError(
        'invalid_project_contract',
        `Every outline item must have valid id, title, and bullets fields; invalid ${invalidField}.`,
        `修复 \`project.json\` 的 \`outline[].${invalidField}\`，确保 id/title 为非空字符串且 bullets 为字符串数组；不要改无关页面。`,
        { invalidOutlineField: invalidField },
      );
    }
    const itemId = item.id.trim();
    if (outlineItemIds.has(itemId)) {
      throw contractError(
        'invalid_project_contract',
        `Every outline item id must be unique; duplicate ${itemId}.`,
        '修复 `project.json` 的 `outline[].id`，确保每项 id 是唯一非空字符串；不要改无关页面。',
        { invalidOutlineField: 'id' },
      );
    }
    outlineItemIds.add(itemId);
    const slideId = String(item.slide_id || '');
    if (!/^slide-\d{2}$/.test(slideId)) {
      throw contractError(
        'invalid_project_contract',
        'Every outline item must have a `slide-NN` slide_id.',
        '修复 `project.json` 的 `outline[].slide_id`，统一使用两位数 `slide-NN`，并同步 `slide_order`；不要改无关页面。',
      );
    }
    outlineIds.push(slideId);
  }

  const orderedIds = plan.slide_order.map((value) => String(value || ''));
  const uniqueOutlineIds = new Set(outlineIds);
  const uniqueOrderedIds = new Set(orderedIds);
  const sameIds = outlineIds.length === orderedIds.length
    && uniqueOutlineIds.size === outlineIds.length
    && uniqueOrderedIds.size === orderedIds.length
    && outlineIds.every((id) => uniqueOrderedIds.has(id));
  if (!sameIds) {
    throw contractError(
      'invalid_project_contract',
      '`slide_order` and `outline[].slide_id` disagree.',
      '修复 `project.json`：让 `slide_order` 与 `outline[].slide_id` 一一对应且无重复；只修复计划或缺失页面，不重写已有页面。',
      { outlineSlideIds: outlineIds, slideOrder: orderedIds },
    );
  }
  return orderedIds;
}

function isCompleteSlideHtml(raw) {
  const match = String(raw || '').match(
    /^\uFEFF?\s*(?:<!doctype\s+html\b[^>]*>\s*)?<html(?:\s[^>]*)?>[\s\S]*?<body(?:\s[^>]*)?>([\s\S]*?)<\/body>[\s\S]*?<\/html>\s*$/i,
  );
  if (!match) return false;
  return Boolean(match[1].replace(/<!--[\s\S]*?-->/g, '').trim());
}

async function readCompleteSlideWithRetry(readFile, relPath, options) {
  const result = await readVisibleFileWithRetry(readFile, relPath, {
    ...options,
    accept: isCompleteSlideHtml,
  });
  return typeof result === 'string' ? result.trim() : null;
}

/**
 * If outline/slide files are already complete but status is still "planning",
 * host-side mark complete so the agent need not spend an extra Glob/Edit round.
 * Returns true when project.json was updated (or already complete).
 */
export async function finalizeDeckProjectIfReady(readFile, writeFile, options = {}) {
  if (typeof writeFile !== 'function') return false;
  let plan;
  try {
    plan = await readProjectPlanWithRetry(readFile, { ...options, requireComplete: false });
  } catch {
    return false;
  }
  if (!plan) return false;
  if (plan.status === 'complete') return true;

  let slideOrder;
  try {
    slideOrder = validateCompletedPlan({ ...plan, status: 'complete' });
  } catch {
    return false;
  }

  for (const slideId of slideOrder) {
    const html = await readCompleteSlideWithRetry(readFile, `slides/${slideId}.html`, options);
    if (!html) return false;
  }

  const nextPlan = { ...plan, status: 'complete' };
  try {
    await writeFile('project.json', `${JSON.stringify(nextPlan, null, 2)}\n`);
  } catch {
    return false;
  }
  return true;
}

export async function readDeckProjectContract(readFile, options = {}) {
  const { writeFile } = options;
  // Prefer host-side complete marking before the requireComplete retry loop,
  // so a finished deck with status still "planning" does not sit in delays.
  if (typeof writeFile === 'function') {
    await finalizeDeckProjectIfReady(readFile, writeFile, options);
  }
  const plan = await readProjectPlanWithRetry(readFile, { ...options, requireComplete: true });
  const slideOrder = validateCompletedPlan(plan);
  const outlineById = new Map(plan.outline.map((item) => [String(item.slide_id), item]));
  const slides = [];
  const missingPaths = [];

  for (let index = 0; index < slideOrder.length; index += 1) {
    const slideId = slideOrder[index];
    const relPath = `slides/${slideId}.html`;
    const html = await readCompleteSlideWithRetry(readFile, relPath, options);
    if (!html) {
      missingPaths.push(relPath);
      continue;
    }
    slides.push({
      slideId,
      slideNumber: index + 1,
      relPath,
      outlineEntry: outlineById.get(slideId),
      html,
    });
  }

  if (missingPaths.length) {
    throw new DeckProjectContractError(missingSlideFilesDiagnostic(missingPaths));
  }
  return { plan, slides };
}
