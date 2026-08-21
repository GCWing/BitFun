const appearanceToken = (name: string): string => 'var(' + '--bf-appearance-token-' + name + ')';

export const APPEARANCE_DOMAIN_TOKENS = {
  contextCompression: appearanceToken('domain-context-compression'),
  generativeUi: appearanceToken('domain-generative-ui'),
  miniApp: appearanceToken('domain-mini-app'),
  mermaidDiagram: appearanceToken('domain-mermaid-diagram'),
  gitGraphLane: Array.from({ length: 8 }, (_, index) => appearanceToken(`domain-git-lane-${index}`)),
  toolIdentity: {
    search: appearanceToken('domain-tool-search'),
    webSearch: appearanceToken('domain-tool-web-search'),
    git: appearanceToken('domain-tool-git'),
    terminal: appearanceToken('domain-tool-terminal'),
    mcp: appearanceToken('domain-tool-mcp'),
    assistantAction: appearanceToken('domain-tool-assistant-action'),
    reviewSummary: appearanceToken('domain-tool-review-summary'),
  },
  agentCapability: {
    docs: appearanceToken('domain-capability-docs'),
    testing: appearanceToken('domain-capability-testing'),
    creative: appearanceToken('domain-capability-creative'),
    ops: appearanceToken('domain-capability-ops'),
  },
  insights: {
    positive: appearanceToken('domain-insights-positive'),
    time: appearanceToken('domain-insights-time'),
    neutral: appearanceToken('domain-insights-neutral'),
    issue: appearanceToken('domain-insights-issue'),
  },
  progress: {
    compacting: appearanceToken('domain-progress-compacting'),
  },
  templateContext: {
    memories: appearanceToken('domain-template-memories'),
  },
  reviewTeam: {
    memberDefault: appearanceToken('domain-review-member-default'),
    worker: appearanceToken('domain-review-worker'),
    judge: appearanceToken('domain-review-judge'),
  },
  agentTeam: {
    accents: Array.from({ length: 6 }, (_, index) => appearanceToken(`domain-agent-team-accent-${index}`)),
    roleLeader: appearanceToken('domain-agent-team-role-leader'),
    roleMember: appearanceToken('domain-agent-team-role-member'),
    roleReviewer: appearanceToken('domain-agent-team-role-reviewer'),
  },
  tealAction: appearanceToken('domain-teal-action'),
  todo: appearanceToken('domain-todo'),
  textStroke: Array.from({ length: 5 }, (_, index) => appearanceToken(`domain-text-stroke-${index}`)),
  inspectorOverlay: {
    activeBorder: appearanceToken('domain-inspector-active-border'),
    activeBackground: appearanceToken('domain-inspector-active-background'),
    activeBorderSubtle: appearanceToken('domain-inspector-active-border-subtle'),
    selectedBorder: appearanceToken('domain-inspector-selected-border'),
    selectedBackground: appearanceToken('domain-inspector-selected-background'),
    browserTooltipBackground: appearanceToken('domain-inspector-browser-tooltip-background'),
    mainTooltipBackground: appearanceToken('domain-inspector-main-tooltip-background'),
    tooltipText: appearanceToken('domain-inspector-tooltip-text'),
    tooltipShadow: appearanceToken('domain-inspector-tooltip-shadow'),
    staticWhite: appearanceToken('color-static-white'),
  },
} as const;

const languageToken = (name: string): string => appearanceToken(`language-${name}`);

export const APPEARANCE_LANGUAGE_TOKENS = {
  typescript: languageToken('blue'),
  typescriptReact: languageToken('cyan'),
  javascript: languageToken('yellow'),
  javascriptReact: languageToken('cyan'),
  python: languageToken('blue'),
  rust: languageToken('orange'),
  go: languageToken('cyan'),
  java: languageToken('orange'),
  kotlin: languageToken('purple'),
  cpp: languageToken('red'),
  c: languageToken('slate'),
  csharp: languageToken('green'),
  swift: languageToken('red'),
  php: languageToken('purple'),
  ruby: languageToken('red'),
  scala: languageToken('red'),
  dart: languageToken('cyan'),
  lua: languageToken('blue'),
  r: languageToken('blue'),
  html: languageToken('red'),
  xml: languageToken('blue'),
  vue: languageToken('green'),
  svelte: languageToken('orange'),
  css: languageToken('blue'),
  scss: languageToken('purple'),
  sass: languageToken('purple'),
  less: languageToken('blue'),
  json: languageToken('yellow'),
  yaml: languageToken('red'),
  toml: languageToken('orange'),
  sql: languageToken('orange'),
  graphql: languageToken('purple'),
  dockerfile: languageToken('blue'),
  makefile: languageToken('green'),
  ini: languageToken('slate'),
  env: languageToken('yellow'),
  shell: languageToken('green'),
  powershell: languageToken('blue'),
  batch: languageToken('green'),
  markdown: languageToken('blue'),
  restructuredtext: languageToken('slate'),
  image: languageToken('purple'),
  audio: languageToken('green'),
  video: languageToken('red'),
  font: languageToken('orange'),
  archive: languageToken('purple'),
  binary: languageToken('slate'),
  plaintext: languageToken('slate'),
} as const;

const CODE_SNIPPET_LANGUAGE_TOKENS = {
  javascript: APPEARANCE_LANGUAGE_TOKENS.javascript,
  typescript: APPEARANCE_LANGUAGE_TOKENS.typescript,
  python: APPEARANCE_LANGUAGE_TOKENS.python,
  rust: APPEARANCE_LANGUAGE_TOKENS.rust,
  go: APPEARANCE_LANGUAGE_TOKENS.go,
  java: APPEARANCE_LANGUAGE_TOKENS.java,
  html: APPEARANCE_LANGUAGE_TOKENS.html,
  css: APPEARANCE_LANGUAGE_TOKENS.css,
  scss: APPEARANCE_LANGUAGE_TOKENS.scss,
  fallback: APPEARANCE_LANGUAGE_TOKENS.plaintext,
} as const;

export function getCodeSnippetLanguageAccent(language?: string): string {
  if (!language) return CODE_SNIPPET_LANGUAGE_TOKENS.fallback;
  return CODE_SNIPPET_LANGUAGE_TOKENS[language as keyof typeof CODE_SNIPPET_LANGUAGE_TOKENS]
    ?? CODE_SNIPPET_LANGUAGE_TOKENS.fallback;
}

const prismToken = (mode: 'light' | 'dark', role: string): string => appearanceToken(`prism-${mode}-${role}`);

export const APPEARANCE_PRISM_TOKENS = {
  light: {
    foreground: prismToken('light', 'foreground'),
    comment: prismToken('light', 'comment'),
    keyword: prismToken('light', 'keyword'),
    string: prismToken('light', 'string'),
    functionName: prismToken('light', 'function'),
    number: prismToken('light', 'number'),
    tag: prismToken('light', 'tag'),
    punctuation: prismToken('light', 'punctuation'),
    property: prismToken('light', 'property'),
  },
  dark: {
    foreground: prismToken('dark', 'foreground'),
    comment: prismToken('dark', 'comment'),
    keyword: prismToken('dark', 'keyword'),
    string: prismToken('dark', 'string'),
    functionName: prismToken('dark', 'function'),
    number: prismToken('dark', 'number'),
    tag: prismToken('dark', 'tag'),
    punctuation: prismToken('dark', 'punctuation'),
    property: prismToken('dark', 'property'),
  },
} as const;
