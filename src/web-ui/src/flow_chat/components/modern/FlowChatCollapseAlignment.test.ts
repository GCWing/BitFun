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

describe('FlowChat collapse spacing', () => {
  it('keeps every shared collapse body on the same outer leading edge', () => {
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
    expect(stylesheet).not.toContain('padding-inline-start: 0;');
  });

  it('keeps unframed explore and thinking disclosures on their header edge', () => {
    const exploreStyles = readSource('./ExploreRegion.scss');
    const thinkingStyles = readSource('../../tool-cards/ModelThinkingDisplay.scss');
    const exploreContent = extractBlock(exploreStyles, '.explore-region__content');
    const thinkingContent = extractBlock(thinkingStyles, '.thinking-content');

    expect(exploreContent).toContain('padding: 0;');
    expect(thinkingContent).toMatch(
      /padding:\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-y\)\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-x\)\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-y\)\s*0;/,
    );
  });

  it('keeps bordered tool and subagent bodies padded on every side', () => {
    const baseToolStyles = readSource('../../tool-cards/BaseToolCard.scss');
    const compactToolStyles = readSource('../../tool-cards/CompactToolCard.scss');
    const imageStyles = readSource('../../tool-cards/ViewImageToolCard.scss');
    const subagentStyles = readSource('./SubagentItems.scss');
    const subagentProjectionStyles = readSource('../subagent/SubagentProjectionView.scss');
    const taskStyles = readSource('../../tool-cards/TaskToolDisplay.scss');

    expect(extractBlock(baseToolStyles, '.base-tool-card-expanded')).toContain(
      'padding: var(--bf-appearance-token-flowchat-card-expanded-pad-y) var(--bf-appearance-token-tool-card-expanded-pad-x);',
    );
    expect(extractBlock(baseToolStyles, '.base-tool-card-error')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(baseToolStyles, '.base-tool-card-error')).toMatch(
      /padding:\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-y\)\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-x\)\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-y\)\s*var\(--bf-appearance-token-flowchat-card-expanded-pad-x\);/,
    );
    expect(extractBlock(compactToolStyles, '.compact-tool-card-expanded')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(compactToolStyles, '.compact-tool-card-expanded')).toContain(
      'padding: 12px;',
    );
    expect(extractBlock(compactToolStyles, '.flow-tool-card-note')).toContain(
      'margin-left: 0;',
    );
    expect(extractBlock(imageStyles, '.view-image-tool-card__content')).toContain(
      'margin: 8px 0 0;',
    );
    expect(extractBlock(subagentStyles, '.subagent-items-container')).toContain(
      'padding: var(--bf-appearance-token-flowchat-card-expanded-pad-y) var(--bf-appearance-token-flowchat-card-expanded-pad-x);',
    );
    expect(
      extractBlock(subagentProjectionStyles, '.subagent-projection-container--expanded'),
    ).toContain(
      'padding: var(--bf-appearance-token-flowchat-card-expanded-pad-y) var(--bf-appearance-token-flowchat-card-expanded-pad-x);',
    );
    expect(
      extractBlock(taskStyles, '.task-expanded-content .task-prompt-content'),
    ).toContain('padding: 0;');
    expect(taskStyles).toContain('--task-prompt-inline-pad: calc(');
    expect(taskStyles).toMatch(
      /\.subagent-projection-container--expanded\s*\{[\s\S]*?padding:\s*8px\s*var\(--task-prompt-inline-pad\)\s*10px\s*var\(--task-prompt-inline-pad\);/,
    );
  });

  it('lets full-bleed footer and list surfaces consume the shared body inset', () => {
    const terminalStyles = readSource('../../tool-cards/TerminalToolCard.scss');
    const gitStyles = readSource('../../tool-cards/GitToolDisplay.scss');
    const miniAppStyles = readSource('../../tool-cards/MiniAppToolDisplay.scss');
    const todoStyles = readSource('../../tool-cards/TodoWriteDisplay.scss');

    expect(terminalStyles).toContain(
      '.base-tool-card-wrapper.terminal-tool-card .terminal-result-footer {\n  margin-left: calc(-1 * var(--bf-appearance-token-flowchat-card-expanded-pad-x));',
    );
    expect(gitStyles).toMatch(/git-result-footer[\s\S]{0,120}margin-left:\s*-\d/);
    expect(miniAppStyles).toContain(
      '.base-tool-card-wrapper.miniapp-tool-display .miniapp-result-footer {\n  margin-left: calc(-1 * var(--bf-appearance-token-flowchat-card-expanded-pad-x));',
    );
    expect(extractBlock(todoStyles, '.todo-expanded-body')).toMatch(
      /margin:\s*calc\(var\(--bf-appearance-token-flowchat-card-expanded-pad-y\) \* -1\)\s*calc\(var\(--bf-appearance-token-tool-card-expanded-pad-x\) \* -1\);/,
    );
  });
});
