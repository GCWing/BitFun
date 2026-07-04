// BitFun backend adapter for PPT Live.
//
// The MiniApp agent bridge (`app.agent.*`) is the only generation path. A
// single cowork agent turn loads BitFun's pinned built-in `ppt-design` skill
// and produces the entire deck end to end — research, outline, design system,
// and every slide HTML — following the skill's native project.json +
// slides/slide-NN.html file protocol.
//
// Design principle: the prompt stays minimal. The ppt-design skill owns all
// design rules, schemas, templates, and quality bars via progressive disclosure
// (SKILL.md → references/*.md). The prompt only carries user intent + style
// preferences + MiniApp headless constraints.

const EVENT_LISTENERS = new Set();
export const PPT_DESIGN_SKILL_KEY = 'user::bitfun-system::ppt-design';

function emitEvent(event) {
  EVENT_LISTENERS.forEach((listener) => {
    try {
      listener(event);
    } catch {
      // A UI listener must not break the host stream.
    }
  });
}

// ─── Agent prompt builder (minimal — delegates to skill) ─────────────────────

function serializeInput(input) {
  try {
    return JSON.stringify(input ?? {}, null, 2);
  } catch {
    return '{}';
  }
}

function hasCurrentDeck(input) {
  return Array.isArray(input?.currentDeck?.slides) && input.currentDeck.slides.length > 0;
}

/**
 * Describe the user's style preferences in one concise line, so the skill can
 * apply them without the prompt restating design rules the skill already owns.
 */
function describeStyle(style = {}) {
  const parts = [];
  const font = style.fontFamily;
  if (font === 'serif') parts.push('衬线字体');
  else if (font === 'sans') parts.push('非衬线字体');

  const density = style.density === 'loose' ? 'spacious' : style.density;
  if (density === 'compact') parts.push('紧凑信息密度');
  else if (density === 'spacious') parts.push('宽松留白');

  const colorMode = style.colorMode || style.theme;
  if (colorMode === 'dark') parts.push('深色主题');

  if (style.stylePreset) parts.push(`风格预设: ${style.stylePreset}`);

  return parts.length ? parts.join('、') : '';
}

/**
 * Build the full agent user prompt for a `ppt.generate` run.
 *
 * Intentionally minimal: user intent + style line + "use ppt-design skill".
 * All design rules, file schemas, layout templates, quality bars, and
 * progressive-disclosure reference routing live inside the skill itself.
 * Restating them here creates prompt/skill duplication that confuses the model.
 */
function buildAgentPrompt(input) {
  const hasDeck = hasCurrentDeck(input);
  const styleLine = describeStyle(input?.style);
  const instruction = input?.instruction || input?.userInput || '';

  // --- Core task (one sentence) ---
  let prompt = hasDeck
    ? `使用 PPT-Design skill 编辑现有 PPT。编辑指令：${instruction || '（见 currentDeck 上下文）'}。`
    : `使用 PPT-Design skill 生成 PPT。用户需求：${instruction || '（见 input JSON）'}。`;

  // --- Style preferences (one line) ---
  if (styleLine) {
    prompt += `\n样式偏好：${styleLine}。`;
  }

  // --- MiniApp headless constraints (only what the skill doesn't know) ---
  prompt += `

## 约束

- 用户只能看到 PPT Live UI，无法回答提问。如有歧义自行判断最优方案并记录假设。
- 不要调用 AskUserQuestion、ControlHub、GenerativeUI、ComputerUse 等交互工具。
- 研究用 WebSearch / WebFetch 即可。
- **一次写对，禁止事后审计**：每页 HTML 在写入时就要满足所有约束（画布尺寸、四条 OOXML 硬约束、防溢出预算）。所有页面写完后不得再逐页 Read→Edit 返工或 Grep 批量检查。写完即结束。
`;

  // --- Context for edit operations ---
  if (hasDeck) {
    prompt += `
## 编辑上下文

- \`currentDeck\` 已提供。将用户指令视为对现有 deck 的增量编辑，除非指令明确要求全新生成。
- \`currentDeck.slides[].slideNumber\` 是从 1 开始的页码，与用户口语一致。
- 编辑时只重写变更的 \`slides/slide-NN.html\` 文件，不动其他页。
`;
  }

  // --- Full input JSON (for research context, outline hints, etc.) ---
  prompt += `
Input JSON:
\`\`\`json
${serializeInput(input)}
\`\``;

  // --- Interruption continuation prefix ---
  if (input?.continueAfterInterruption) {
    prompt = `上一次生成被中断了。请继续完成任务：检查 project.json 和已写的 slides/ 文件，只补写还没完成的页面，不要重写已有的页面。\n\n${prompt}`;
  }

  return prompt;
}

// ─── Agent-backed backend (primary path) ─────────────────────────────────────

function installAgentBackend(app) {
  let agentEventsHooked = false;
  const ensureAgentEvents = () => {
    if (agentEventsHooked) return;
    agentEventsHooked = true;
    // Host events already carry sessionId/turnId/sourceEvent/text/contentType/
    // toolEvent/error in the shape ui.js consumes; re-emit them as-is.
    app.agent.onEvent((event) => {
      if (!event || typeof event !== 'object') return;
      emitEvent(event);
    });
  };

  app.backend = {
    // The agent delivers through project files written using the ppt-design
    // skill's native workflow; 'files' tells ui.js to read them back.
    protocol: 'files',
    async call(action, input, options = {}) {
      if (action !== 'ppt.generate') {
        throw new Error(`Unsupported PPT Live action: ${action}`);
      }
      ensureAgentEvents();
      const prompt = buildAgentPrompt(input);
      const result = await app.agent.run(prompt, {
        runId: options.idempotencyKey,
        sessionName: 'PPT Live',
        // Reuse the session when the caller carries one so follow-up edits
        // resume with the loaded skill/preset/research context.
        sessionId: options.sessionId,
        // The agent works inside a dedicated deck project directory under
        // the app's own appdata storage (never the user's workspace).
        appDataWorkspace: options.appDataWorkspace,
      });
      if (!result?.sessionId || !result?.turnId) {
        throw new Error('PPT Live agent backend did not return sessionId/turnId');
      }
      return {
        sessionId: result.sessionId,
        turnId: result.turnId,
        actionRunId: result.actionRunId || result.turnId,
      };
    },
    onEvent(listener) {
      EVENT_LISTENERS.add(listener);
    },
    offEvent(listener) {
      EVENT_LISTENERS.delete(listener);
    },
    async cancel(sessionId, turnId) {
      await app.agent.cancel(sessionId, turnId);
    },
    async turnText(sessionId, turnId) {
      const result = await app.agent.turnText(sessionId, turnId);
      return { text: result?.text || '' };
    },
    async cancelStaleRuns() {
      await app.agent.cancelStaleRuns();
    },
  };
}

// ─── Install ─────────────────────────────────────────────────────────────────

export function installBitFunBackendAdapter(app = window.app) {
  if (!app || app.backend?.call) return;
  if (app.agent?.run) {
    installAgentBackend(app);
  }
}
