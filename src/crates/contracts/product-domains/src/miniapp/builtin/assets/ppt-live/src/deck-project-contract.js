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

export async function persistDeckProjectSeed(fs, projectDir, seed) {
  try {
    await fs.mkdir(`${projectDir}/slides`, { recursive: true });
  } catch {
    throw seedPersistenceError('seed_fs_mkdir_failed', 'mkdir', ['slides']);
  }
  try {
    await fs.writeFile(`${projectDir}/project.json`, `${JSON.stringify(seed.plan, null, 2)}\n`);
  } catch {
    throw seedPersistenceError('seed_fs_write_failed', 'project-write', ['project.json']);
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

function parseProjectJson(raw) {
  try {
    const plan = JSON.parse(raw);
    if (!plan || Array.isArray(plan) || typeof plan !== 'object') throw new Error('root must be an object');
    return plan;
  } catch (error) {
    throw contractError(
      'invalid_project_json',
      '`project.json` is not valid JSON.',
      '修复 `project.json` JSON，使根值为对象；不要重写已有页面。修复后继续完成契约。',
      { cause: String(error?.message || error) },
    );
  }
}

export async function readProjectPlanWithRetry(readFile, options = {}) {
  const { requireComplete = false } = options;
  const result = await readVisibleFileWithRetry(readFile, 'project.json', {
    ...options,
    accept: (raw) => {
      if (!raw.trim()) return false;
      try {
        const parsed = JSON.parse(raw);
        return Boolean(parsed)
          && !Array.isArray(parsed)
          && typeof parsed === 'object'
          && (!requireComplete || parsed.status === 'complete');
      } catch {
        return false;
      }
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

export async function readDeckProjectContract(readFile, options = {}) {
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
