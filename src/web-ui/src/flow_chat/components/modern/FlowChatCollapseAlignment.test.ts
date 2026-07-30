import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function readSource(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    'utf8',
  ).replace(/\r\n?/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(
    new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`),
  );
  return match?.groups?.body ?? '';
}

describe('FlowChat collapse leading-edge alignment', () => {
  it('guards every shared collapse body against leading indentation', () => {
    const stylesheet = readSource('./VirtualMessageList.scss');
    const projectionRoots = [
      '.explore-region__content',
      '.thinking-content',
      '.base-tool-card-expanded',
      '.base-tool-card-error',
      '.compact-tool-card-expanded',
      '.view-image-tool-card__content',
      '.subagent-items-container',
      '.subagent-projection-container--expanded',
    ];

    for (const selector of projectionRoots) {
      expect(stylesheet).toContain(selector);
    }
    expect(stylesheet).toContain('margin-inline-start: 0;');
    expect(stylesheet).toContain('padding-inline-start: 0;');
  });

  it('keeps custom explore and thinking collapse bodies on their header edge', () => {
    const exploreStyles = readSource('./ExploreRegion.scss');
    const thinkingStyles = readSource('../../tool-cards/ModelThinkingDisplay.scss');
    const exploreContent = extractBlock(exploreStyles, '.explore-region__content');
    const thinkingContent = extractBlock(thinkingStyles, '.thinking-content');

    expect(exploreContent).toContain('padding: 0;');
    expect(thinkingContent).toMatch(
      /padding:\s*var\(--flowchat-card-expanded-pad-y\)\s*var\(--flowchat-card-expanded-pad-x\)\s*var\(--flowchat-card-expanded-pad-y\)\s*0;/,
    );
  });

  it('keeps tool, image, and subagent detail roots free of leading offsets', () => {
    const baseToolStyles = readSource('../../tool-cards/BaseToolCard.scss');
    const compactToolStyles = readSource('../../tool-cards/CompactToolCard.scss');
    const imageStyles = readSource('../../tool-cards/ViewImageToolCard.scss');
    const subagentStyles = readSource('./SubagentItems.scss');
    const taskStyles = readSource('../../tool-cards/TaskToolDisplay.scss');

    expect(extractBlock(baseToolStyles, '.base-tool-card-expanded')).toMatch(
      /padding:\s*var\(--flowchat-card-expanded-pad-y\)\s*var\(--tool-card-expanded-pad-x\)\s*var\(--flowchat-card-expanded-pad-y\)\s*0;/,
    );
    expect(extractBlock(baseToolStyles, '.base-tool-card-error')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(compactToolStyles, '.compact-tool-card-expanded')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(compactToolStyles, '.flow-tool-card-note')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(imageStyles, '.view-image-tool-card__content')).toContain(
      'margin: 8px 0 0;',
    );
    expect(extractBlock(subagentStyles, '.subagent-items-container')).toContain(
      'padding: 10px 10px 10px 0;',
    );
    expect(
      extractBlock(taskStyles, '.task-expanded-content .task-prompt-content'),
    ).toContain('padding: 0 var(--flowchat-card-pad-x) 0 0;');
    expect(taskStyles).toContain(
      '.subagent-projection-container--expanded {\n' +
      '      padding:\n' +
      '        8px\n' +
      '        calc(var(--flowchat-card-expanded-pad-x) + var(--flowchat-card-pad-x))\n' +
      '        10px\n' +
      '        0;',
    );
  });

  it('does not pull expanded footer or list surfaces past the leading edge', () => {
    const terminalStyles = readSource('../../tool-cards/TerminalToolCard.scss');
    const gitStyles = readSource('../../tool-cards/GitToolDisplay.scss');
    const miniAppStyles = readSource('../../tool-cards/MiniAppToolDisplay.scss');
    const todoStyles = readSource('../../tool-cards/TodoWriteDisplay.scss');

    expect(terminalStyles).toContain(
      '.base-tool-card-wrapper.terminal-tool-card .terminal-result-footer {\n  margin-left: 0;',
    );
    expect(gitStyles).not.toMatch(/git-result-footer[\s\S]{0,120}margin-left:\s*-\d/);
    expect(miniAppStyles).toContain(
      '.base-tool-card-wrapper.miniapp-tool-display .miniapp-result-footer {\n  margin-left: 0;',
    );
    expect(extractBlock(todoStyles, '.todo-expanded-body')).toMatch(
      /margin:\s*calc\(var\(--flowchat-card-expanded-pad-y\) \* -1\)\s*calc\(var\(--tool-card-expanded-pad-x\) \* -1\)\s*calc\(var\(--flowchat-card-expanded-pad-y\) \* -1\)\s*0;/,
    );
  });
});
