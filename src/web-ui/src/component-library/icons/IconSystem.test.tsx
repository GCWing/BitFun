import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import {
  BitFunIcon,
  HarnessCreativeIcon,
  NavigationExtensionsCompatibilityIcon,
  NavigationMiniAppIcon,
  NavigationSessionContextAddIcon,
  NavigationSessionViewAllIcon,
  NavigationSessionViewGroupedIcon,
  SessionGroupAssistantIcon,
  SessionGroupAssistantSelectedIcon,
  SessionGroupGlobalTaskIcon,
  SessionGroupRemoteWorkspaceIcon,
  SessionGroupRemoteWorkspaceSelectedIcon,
  SessionGroupWorkspaceIcon,
  SessionGroupWorkspaceSelectedIcon,
  bitFunIconMetadata,
  bitFunIconNames,
} from './index';

describe('BitFun icon system', () => {
  it('keeps first-party icon semantics stable and uniquely registered', () => {
    expect(bitFunIconNames).toEqual([
      'session-group-assistant',
      'session-group-remote-workspace',
      'session-group-workspace',
      'session-group-global-task',
      'session-group-assistant-selected',
      'session-group-remote-workspace-selected',
      'session-group-workspace-selected',
      'navigation-session-view-grouped',
      'navigation-session-view-all',
      'navigation-session-context-add',
      'navigation-mini-app',
      'navigation-extensions-compatibility',
      'harness-creative',
    ]);
    expect(new Set(bitFunIconNames).size).toBe(bitFunIconNames.length);
    expect(new Set(bitFunIconNames.map(name => bitFunIconMetadata[name].semantic)).size)
      .toBe(bitFunIconNames.length);
    expect(bitFunIconNames.every(name => bitFunIconMetadata[name].source.type === 'bitfun-svg'))
      .toBe(true);
    expect(bitFunIconNames.every(name => bitFunIconMetadata[name].license === 'BitFun Proprietary'))
      .toBe(true);
  });

  it('renders static semantic exports with their stable BitFun ids', () => {
    const markup = renderToStaticMarkup(
      <>
        <SessionGroupAssistantIcon />
        <SessionGroupRemoteWorkspaceIcon />
        <SessionGroupWorkspaceIcon />
        <SessionGroupGlobalTaskIcon />
        <SessionGroupAssistantSelectedIcon />
        <SessionGroupRemoteWorkspaceSelectedIcon />
        <SessionGroupWorkspaceSelectedIcon />
        <NavigationSessionViewGroupedIcon />
        <NavigationSessionViewAllIcon />
        <NavigationSessionContextAddIcon />
        <NavigationMiniAppIcon />
        <NavigationExtensionsCompatibilityIcon />
        <HarnessCreativeIcon />
      </>,
    );

    for (const name of bitFunIconNames) {
      expect(markup).toContain(`data-bf-icon="${name}"`);
    }
    expect(markup.match(/data-bf-source="bitfun-svg"/g)).toHaveLength(13);
    expect(markup).toContain('stroke="currentColor"');
    expect(markup).toContain('stroke-linecap="round"');
    expect(markup).toContain('fill="currentColor"');
  });

  it('keeps the Creative Harness reference redraw in the proprietary icon registry', () => {
    const metadata = bitFunIconMetadata['harness-creative'];
    const markup = renderToStaticMarkup(<HarnessCreativeIcon />);

    expect(metadata.semantic).toBe('harness.profile.creative');
    expect(metadata.source).toMatchObject({
      type: 'bitfun-svg',
      origin: 'bitfun-reference-redraw',
    });
    expect(markup).toContain('data-bf-icon="harness-creative"');
    expect(markup).toContain('data-bf-source="bitfun-svg"');
  });

  it('provides filled selected variants for every session-group type used in navigation', () => {
    const markup = renderToStaticMarkup(
      <>
        <SessionGroupAssistantSelectedIcon />
        <SessionGroupRemoteWorkspaceSelectedIcon />
        <SessionGroupWorkspaceSelectedIcon />
      </>,
    );

    expect(markup).toContain('data-bf-icon="session-group-assistant-selected"');
    expect(markup).toContain('data-bf-icon="session-group-remote-workspace-selected"');
    expect(markup).toContain('data-bf-icon="session-group-workspace-selected"');
    expect(markup.match(/fill="currentColor"/g)?.length).toBeGreaterThanOrEqual(3);
  });

  it('keeps both session-view states on three equal-length horizontal lines', () => {
    const groupedMarkup = renderToStaticMarkup(<NavigationSessionViewGroupedIcon />);
    const allMarkup = renderToStaticMarkup(<NavigationSessionViewAllIcon />);
    const equalLines = 'd="M4 5.25H20M4 12H20M4 18.75H20"';

    expect(groupedMarkup).toContain(equalLines);
    expect(allMarkup).toContain(equalLines);
    expect(allMarkup).toContain('d="M4.5 21.15L19.5 3.05"');
  });

  it('defaults to decorative and requires an explicit accessible presentation', () => {
    const decorativeMarkup = renderToStaticMarkup(
      <BitFunIcon name="session-group-workspace" />,
    );
    expect(decorativeMarkup).toContain('aria-hidden="true"');
    expect(decorativeMarkup).not.toContain('role="img"');

    const accessibleMarkup = renderToStaticMarkup(
      <BitFunIcon
        name="session-group-workspace"
        decorative={false}
        label="Workspace"
      />,
    );
    expect(accessibleMarkup).toContain('role="img"');
    expect(accessibleMarkup).toContain('aria-label="Workspace"');
    expect(accessibleMarkup).not.toContain('aria-hidden="true"');
  });
});
