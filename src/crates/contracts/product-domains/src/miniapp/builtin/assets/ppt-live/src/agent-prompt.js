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

## Authoring subset（生成规则）

- **唯一导出链路**：editable HTML → EditableSlideScene → OOXML。每页严格为 **1280px × 720px**。
- 只使用 solid color；不得生成 CSS gradient 或 \`background-image\`。背景、border、圆角只放在 \`div\` 等几何容器。
- \`box-shadow\` 只支持单层 outer、非 inset、zero spread；多层、inset、blur+spread 等不支持形态必须 blocking。\`text-shadow\` 任何非 \`none\` 形态均必须 blocking。
- HTML 文字只可放在 \`<p>\`、\`<h1>\`–\`<h6>\`、\`<li>\` 中；\`span\` 只作文本 run，不得生成 \`div\` 裸文字。
- 禁止 CSS \`filter\`、\`mask\`、generated content、animation、外部资源和复杂/filled SVG path；禁止任意顶点/非严格对称 polygon，仅允许严格对称 triangle/diamond。
- 线与曲线优先直接生成 \`line\` 或 \`polyline\`；确有必要时才使用下面的兼容 path 子集。
- Authoring 流程箭头只由 editable line + CSS border triangle，或 SVG line + strict symmetric triangle polygon 构成。
- 表格必须写真实的 \`<table>\` 并导出为 native \`a:tbl\`；图表、流程箭头、虚线和曲线必须使用支持的可编辑原语。
- intentional 图片只允许内联 base64 PNG、JPEG、WebP，且不得承载文字、图表或几何；禁止 GIF，因为无法证明其为静态内容。
- 禁止任何正向 rasterize、screenshot 或 fallback 建议；无法表示时停止生成并报告具体元素。

## Converter legacy rewrite boundary（兼容边界，不是生成建议）

- 本边界只用于兼容既有输入，不是生成许可；authoring agent 仍必须遵守上面的更严格 subset。
- SVG \`text\` 是 converter 支持的 SVG 原语；\`div\` 裸文字仅属 repair 兼容，authoring 不应生成。
- path 仅支持 \`M/L/H/V/C/S/Q/T/Z\`，必须 \`fill:none\`；\`Z\` 可以闭合 path，但拒绝 \`A\` 和任何 path/ancestor \`transform\`。
- \`C/S/Q/T\` 曲线被采样为多段 editable line，不是 PowerPoint curve；authoring 优先 \`line\`/\`polyline\`，确需 path 时才使用上述子集。
- SVG polygon 只识别严格对称的 triangle 和 diamond；任意顶点或非严格对称 polygon 都会被拒绝。
- legacy CSS 仅兼容受限 \`linear-gradient\`：角度接受 \`deg\`、\`turn\`、\`rad\`、\`grad\` 与方向关键字；位置只接受 percentage stop，缺省 stop 均匀分配。
- converter 拒绝 \`radial-gradient\`、px/em stop、double-position stop、color hint、不支持颜色和非法 alpha；合法 gradient 被采样为 editable solid strips，这不是生成建议。
- legacy 单层 hard ring \`box-shadow\`（\`0 0 0 Npx\`、非 inset、blur=0）会被重写为同心可编辑 shape；authoring 仍应优先 zero-spread outer shadow，不得依赖 ring rewrite。

<!-- End editable contract -->

- **一次写对，禁止事后审计**：每页 HTML 在写入时就要满足上述 authoring contract 和防溢出预算。完成检查只核对生成文件协议，不逐页 Read→Edit 返工或 Grep 批量审计页面内容。
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
