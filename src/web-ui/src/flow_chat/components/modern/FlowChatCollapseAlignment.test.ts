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
      "[data-bf-component='flow-chat-tool-card'][data-bf-part='expanded']",
      "[data-bf-component='flow-chat-tool-card'][data-bf-part='error']",
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
      /padding:\s*var\(--bf-control-flow-chat-card-expanded-padding-block\)\s*var\(--bf-control-flow-chat-card-expanded-padding-inline\)\s*var\(--bf-control-flow-chat-card-expanded-padding-block\)\s*0;/,
    );
  });

  it('keeps bordered tool and subagent bodies padded on every side', () => {
    const publicToolCardStyles = readSource('../../../../../../design-system/packages/ui/src/flow-chat/tool-cards/FlowChatToolCard.module.css');
    const flowToolCardStyles = readSource('../FlowToolCard.scss');
    const subagentStyles = readSource('./SubagentItems.scss');
    const subagentProjectionStyles = readSource('../subagent/SubagentProjectionView.scss');
    const taskStyles = readSource('../../tool-cards/TaskToolDisplay.scss');

    expect(publicToolCardStyles).toMatch(
      /\.expanded,\s*\.error\s*\{[\s\S]*?padding:\s*var\(--bf-space-3\);/,
    );
    expect(extractBlock(flowToolCardStyles, '.flow-tool-card-note')).toContain(
      'margin-inline-start: 0;',
    );
    expect(extractBlock(subagentStyles, '.subagent-items-container')).toContain(
      'padding: var(--bf-control-flow-chat-card-expanded-padding-block) var(--bf-control-flow-chat-card-expanded-padding-inline);',
    );
    expect(
      extractBlock(subagentProjectionStyles, '.subagent-projection-container--expanded'),
    ).toContain(
      'padding: var(--bf-control-flow-chat-card-expanded-padding-block) var(--bf-control-flow-chat-card-expanded-padding-inline);',
    );
    expect(
      extractBlock(taskStyles, '.task-expanded-content .task-prompt-content'),
    ).toContain('padding: 0;');
    expect(taskStyles).toContain('--task-prompt-inline-pad: calc(');
    expect(taskStyles).toMatch(
      /\.subagent-projection-container--expanded\s*\{[\s\S]*?padding:\s*8px\s*var\(--task-prompt-inline-pad\)\s*10px\s*var\(--task-prompt-inline-pad\);/,
    );
  });

  it('lets product-owned full-bleed footer surfaces consume the shared body inset', () => {
    const miniAppStyles = readSource('../../tool-cards/MiniAppToolDisplay.scss');
    expect(miniAppStyles).toContain(
      ".miniapp-tool-display[data-bf-attention='prominent'] .miniapp-result-footer {\n  margin-left: calc(-1 * var(--bf-control-flow-chat-card-expanded-padding-inline));",
    );
  });
});
