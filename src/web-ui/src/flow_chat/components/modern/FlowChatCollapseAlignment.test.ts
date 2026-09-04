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
      "[data-openbitfun-component='flow-chat-tool-card'][data-openbitfun-part='expanded']",
      "[data-openbitfun-component='flow-chat-tool-card'][data-openbitfun-part='error']",
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
      /padding:\s*var\(--openbitfun-control-flow-chat-card-expanded-padding-block\)\s*var\(--openbitfun-control-flow-chat-card-expanded-padding-inline\)\s*var\(--openbitfun-control-flow-chat-card-expanded-padding-block\)\s*0;/,
    );
  });

  it('uses the thinking-style non-button disclosure and icon swap for explore', () => {
    const renderer = readSource('./ExploreGroupRenderer.tsx');
    const exploreStyles = readSource('./ExploreRegion.scss');

    expect(renderer).toMatch(
      /<div\s+data-openbitfun-component="explore-group"\s+data-openbitfun-part="header"[\s\S]*?data-testid="chat-explore-group-toggle"/,
    );
    expect(renderer).not.toMatch(
      /<button[\s\S]*?data-testid="chat-explore-group-toggle"/,
    );
    expect(renderer).not.toContain('aria-expanded={isExpanded}');
    expect(renderer).not.toContain('data-motion="none"');
    expect(renderer).toContain('name="search" size="sm" className="explore-region__leading-icon--default"');
    expect(renderer).toContain('name="chevron-right" size="sm" className="explore-region__leading-icon--collapsed-hover"');
    expect(renderer).toContain('name="chevron-down" size="sm" className="explore-region__leading-icon--expanded"');
    expect(exploreStyles).toContain('background: transparent;');
    expect(exploreStyles).not.toContain('background: var(--openbitfun-color-action-neutral-surface-hover);');
    expect(exploreStyles).not.toContain('transform: rotate(');
  });

  it('keeps bordered tool and subagent bodies padded on every side', () => {
    const publicToolCardStyles = readSource('../../../../../../design-system/packages/ui/src/flow-chat/tool-cards/FlowChatToolCard.module.css');
    const flowToolCardStyles = readSource('../FlowToolCard.scss');
    const subagentStyles = readSource('./SubagentItems.scss');
    const subagentProjectionStyles = readSource('../subagent/SubagentProjectionView.scss');
    const taskStyles = readSource('../../tool-cards/TaskToolDisplay.scss');

    expect(publicToolCardStyles).toMatch(
      /\.expanded,\s*\.error\s*\{[\s\S]*?padding:\s*var\(--openbitfun-space-3\);/,
    );
    expect(extractBlock(flowToolCardStyles, '.flow-tool-card-note')).toContain(
      'margin-inline-start: 0;',
    );
    expect(extractBlock(subagentStyles, '.subagent-items-container')).toContain(
      'padding: var(--openbitfun-control-flow-chat-card-expanded-padding-block) var(--openbitfun-control-flow-chat-card-expanded-padding-inline);',
    );
    expect(
      extractBlock(subagentProjectionStyles, '.subagent-projection-container--expanded'),
    ).toContain(
      'padding: var(--openbitfun-control-flow-chat-card-expanded-padding-block) var(--openbitfun-control-flow-chat-card-expanded-padding-inline);',
    );
    expect(
      extractBlock(taskStyles, '.task-prompt-content'),
    ).toContain('padding: 0;');
    expect(taskStyles).not.toContain('--task-prompt-inline-pad');
    expect(taskStyles).toMatch(
      /^    \.subagent-projection-container--expanded\s*\{\s*padding:\s*var\(--openbitfun-space-2\)\s*var\(--openbitfun-space-3\)\s*var\(--openbitfun-space-3\);/m,
    );
  });

  it('lets product-owned full-bleed footer surfaces consume the shared body inset', () => {
    const miniAppStyles = readSource('../../tool-cards/MiniAppToolDisplay.scss');
    expect(miniAppStyles).toContain(
      ".miniapp-tool-display[data-openbitfun-attention='prominent'] .miniapp-result-footer {\n  margin-left: calc(-1 * var(--openbitfun-control-flow-chat-card-expanded-padding-inline));",
    );
  });
});
