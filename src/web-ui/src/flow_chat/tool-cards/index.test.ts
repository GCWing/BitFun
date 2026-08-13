// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import {
  DEDICATED_TOOL_CARD_NAMES,
  getToolCardComponent,
  isCollapsibleTool,
  TOOL_CARD_COMPONENTS,
  usesDefaultToolCard,
} from './index';
import { TaskToolDisplay } from './TaskToolDisplay';

describe('tool card registry', () => {
  it('projects managed Review workers through the unified coverage card', () => {
    expect(getToolCardComponent('LaunchReviewAgent')).toBe(TaskToolDisplay);
  });

  it('keeps lightweight dedicated-card classification aligned with the component registry', () => {
    expect([...DEDICATED_TOOL_CARD_NAMES].sort()).toEqual(
      Object.keys(TOOL_CARD_COMPONENTS).sort(),
    );
  });

  it.each(['ControlHub', 'FinalizeMiniApp', 'PublishMiniApp', 'PublishAppearance'])(
    'treats %s as a default-card explore tool',
    (toolName) => {
      expect(usesDefaultToolCard(toolName)).toBe(true);
      expect(isCollapsibleTool(toolName)).toBe(true);
    },
  );

  it('does not classify MCP tools as default-card explore tools', () => {
    expect(usesDefaultToolCard('mcp__server__tool')).toBe(false);
    expect(isCollapsibleTool('mcp__server__tool')).toBe(false);
  });
});
