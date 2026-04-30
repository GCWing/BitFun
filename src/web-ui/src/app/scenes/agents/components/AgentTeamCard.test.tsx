import React from 'react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import AgentTeamCard from './AgentTeamCard';

function readAgentTeamCardStylesheet(): string {
  const stylesheet = readFileSync(
    fileURLToPath(new URL('./AgentTeamCard.scss', import.meta.url)),
    'utf8',
  );
  return stylesheet.replace(/\r\n/g, '\n');
}

function extractBlock(stylesheet: string, selector: string): string {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = stylesheet.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[\\s\\S]*?)\\n\\s*\\}`));
  return match?.groups?.body ?? '';
}

describe('AgentTeamCard', () => {
  it('keeps role summary compact when the review team grows', () => {
    const markup = renderToStaticMarkup(
      <AgentTeamCard
        title="Code Review Team"
        subtitle="Reviewers inspect the change from multiple angles."
        roleName="Code review"
        tagNames={[
          'Business logic',
          'Performance',
          'Security',
          'Architecture',
          'Frontend',
          'Judge',
        ]}
        onOpen={() => undefined}
      />,
    );

    const chipMatches = markup.match(/agent-team-card__tag-chip/g) ?? [];
    expect(chipMatches).toHaveLength(3);
    expect(markup).toContain('Business logic');
    expect(markup).toContain('Performance');
    expect(markup).toContain('Security');
    expect(markup).not.toContain('Architecture');
    expect(markup).not.toContain('Frontend');
    expect(markup).not.toContain('Judge');
  });

  it('keeps role summary tags shrinkable and wrapping instead of clipping chips', () => {
    const stylesheet = readAgentTeamCardStylesheet();
    const tagsBlock = extractBlock(stylesheet, '&__tags');
    const tagChipBlock = extractBlock(stylesheet, '&__tag-chip');

    expect(tagsBlock).toContain('flex-wrap: wrap;');
    expect(tagsBlock).toContain('min-width: 0;');
    expect(tagsBlock).toContain('max-width: 100%;');
    expect(tagChipBlock).toContain('white-space: nowrap;');
  });
});
