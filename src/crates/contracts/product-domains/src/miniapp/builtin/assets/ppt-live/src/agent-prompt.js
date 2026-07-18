export const PPT_DESIGN_SKILL_KEY = 'user::bitfun-system::ppt-design';

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

function formatContractDiagnostic(diagnostic) {
  if (!diagnostic) return '';
  if (typeof diagnostic === 'string') return diagnostic.trim();
  const code = String(diagnostic.code || 'unknown_contract_error');
  const continuation = String(diagnostic.continuationPrompt || '').trim();
  return [`诊断代码：${code}`, continuation].filter(Boolean).join('\n');
}

export function buildAgentPrompt(input) {
  const hasDeck = hasCurrentDeck(input);
  const styleLine = describeStyle(input?.style);
  const instruction = input?.instruction || input?.userInput || '';
  let prompt = hasDeck
    ? `编辑现有 PPT。编辑指令：${instruction || '（见 currentDeck 上下文）'}。`
    : `生成 PPT。用户需求：${instruction || '（见 input JSON）'}。`;

  prompt = `先调用 Skill，并且 skill key 必须精确为 \`${PPT_DESIGN_SKILL_KEY}\`。\n${prompt}`;
  if (styleLine) prompt += `\n样式偏好：${styleLine}。`;

  prompt += `

## 生成文件协议

- 当前 agent 工作区根目录就是 deck 根目录；所有路径均相对该工作区根目录。
- 先写工作区根目录下的 \`project.json\`，再写工作区根目录下的 \`slides/slide-NN.html\`。
- 只有在 \`slide_order\` 引用的每一页都已有完整 HTML 后，才将 \`project.json\` 的 \`status\` 设为 \`"complete"\`。
- 完成前做一次有界检查：核对 \`outline[].slide_id\`、\`slide_order\` 和对应页面文件；缺什么只补什么，检查后立即结束。

## 约束

- 用户只能看到 PPT Live UI，无法回答提问。如有歧义自行判断最优方案并记录假设。
- 不要调用 AskUserQuestion、ControlHub、GenerativeUI、ComputerUse 等交互工具。
- 研究用 WebSearch / WebFetch 即可。
- **一次写对，禁止事后审计**：每页 HTML 在写入时就要满足所有约束（画布尺寸、四条 OOXML 硬约束、防溢出预算）。完成检查只核对生成文件协议，不逐页 Read→Edit 返工或 Grep 批量审计页面内容。
`;

  if (hasDeck) {
    prompt += `
## 编辑上下文

- \`currentDeck\` 已提供。将用户指令视为对现有 deck 的增量编辑，除非指令明确要求全新生成。
- \`currentDeck.slides[].slideNumber\` 是从 1 开始的页码，与用户口语一致。
- 编辑时只重写变更的 \`slides/slide-NN.html\` 文件，不动其他页。
`;
  }

  prompt += `
Input JSON:
\`\`\`json
${serializeInput(input)}
\`\`\``;

  if (input?.continueAfterInterruption) {
    const diagnostic = formatContractDiagnostic(input.projectContractDiagnostic);
    prompt = `上一次生成被中断或未通过文件契约。请在同一会话中定向续跑，不要重写已完成页面。
${diagnostic ? `\n${diagnostic}\n` : ''}
检查 \`project.json\` 和已写的 \`slides/\` 文件，只修复诊断指出的内容；完成后执行一次有界检查。\n\n${prompt}`;
  }

  return prompt;
}
