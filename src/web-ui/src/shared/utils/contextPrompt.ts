import type { ContextItem } from '@/shared/types/context';

export function formatContextForPrompt(context: ContextItem): string {
  switch (context.type) {
    case 'file':
      return `[File: ${context.relativePath || context.filePath}]`;
    case 'directory':
      return `[Directory: ${context.directoryPath}]`;
    case 'code-snippet':
      return `[Code Snippet: ${context.filePath}:${context.startLine}-${context.endLine}]`;
    case 'pull-request':
      return [
        `[Pull Request Context: ${context.label}]`,
        context.repository ? `Repository: ${context.repository}` : '',
        context.remoteId ? `Remote ID: ${context.remoteId}` : '',
        context.pullRequestNumber ? `Pull Request: #${context.pullRequestNumber}${context.pullRequestTitle ? ` ${context.pullRequestTitle}` : ''}` : '',
        context.sourceUrl ? `URL: ${context.sourceUrl}` : '',
        `Section: ${context.section}`,
        '',
        context.content,
      ].filter(line => line !== '').join('\n');
    case 'image':
      return '';
    case 'terminal-command':
      return `[Command: ${context.command}]`;
    case 'mermaid-node':
      return `[Mermaid Node: ${context.nodeText}]`;
    case 'mermaid-diagram':
      return `[Mermaid Diagram${context.diagramTitle ? ': ' + context.diagramTitle : ''}]\n\`\`\`mermaid\n${context.diagramCode}\n\`\`\``;
    case 'git-ref':
      return `[Git Ref: ${context.refValue}]`;
    case 'url':
      return `[URL: ${context.url}]`;
    case 'web-element': {
      const isCanvasElement = typeof context.metadata?.artifactReference === 'string';
      const label = typeof context.metadata?.label === 'string' ? context.metadata.label : `<${context.tagName}>`;
      const attrStr = Object.entries(context.attributes)
        .map(([k, v]) => `${k}="${v}"`)
        .join(' ');
      const lines = [
        isCanvasElement
          ? `[Canvas Element: ${label}]`
          : `[Web Element: <${context.tagName}${attrStr ? ' ' + attrStr : ''}>]`,
        `CSS Path: ${context.path}`,
      ];
      if (context.sourceUrl) lines.push(`Source URL: ${context.sourceUrl}`);
      if (context.textContent) lines.push(`Text Content: ${context.textContent}`);
      if (context.outerHTML) lines.push(`${isCanvasElement ? 'Element Reference' : 'Outer HTML'}:\n\`\`\`html\n${context.outerHTML}\n\`\`\``);
      return lines.join('\n');
    }
    default: {
      const exhaustive: never = context;
      return String(exhaustive);
    }
  }
}
