import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const SOURCE_ROOT = fileURLToPath(new URL('../../', import.meta.url));
const readSource = (path: string): string => readFileSync(join(SOURCE_ROOT, path), 'utf8');

const portalStyleOverrides: Record<string, string> = {
  'app/components/NavPanel/components/AssistantSessionCreateMenu.tsx':
    'app/components/NavPanel/NavPanel.scss',
  'app/components/NavPanel/components/DeviceStatusControl.tsx':
    'app/components/NavPanel/NavPanel.scss',
  'app/components/NavPanel/components/PersistentFooterActions.tsx':
    'app/components/NavPanel/NavPanel.scss',
  'app/components/NavPanel/components/WorkspaceSessionFilterMenu.tsx':
    'app/components/NavPanel/sections/sessions/SessionsSection.scss',
  'app/components/NavPanel/MainNav.tsx': 'app/components/NavPanel/NavPanel.scss',
  'app/components/NavPanel/sections/workspaces/WorkspaceItem.tsx':
    'app/components/NavPanel/sections/workspaces/WorkspaceListSection.scss',
  'app/components/scheduled-jobs/DateTimePickerPopover.tsx':
    'app/components/scheduled-jobs/LocalizedDateTimeField.scss',
  'app/scenes/profile/views/AssistantAvatarPicker.tsx':
    'app/scenes/profile/views/NurseryView.scss',
  'app/scenes/shell/components/ShellNavWorkspaceSwitcher.tsx':
    'app/scenes/shell/ShellNav.scss',
  'flow_chat/components/WelcomePanel.tsx': 'flow_chat/components/WelcomePanelSurface.scss',
};

const portalSurfaceExceptions: Record<string, string> = {
  'app/components/panels/DiffFullscreenViewer.tsx': 'fullscreen viewer',
  'component-library/components/Tooltip/Tooltip.tsx': 'tooltip primitive',
  'flow_chat/components/modern/UserMessageItem.tsx': 'fullscreen image lightbox',
  'flow_chat/tool-cards/SnapshotFullscreenDiffViewer.tsx': 'fullscreen viewer',
};

const discoverPortalSourceFiles = (): string[] => {
  const discovered: string[] = [];

  const visit = (relativeDirectory: string): void => {
    const absoluteDirectory = join(SOURCE_ROOT, relativeDirectory);
    for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
      const relativePath = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        visit(relativePath);
        continue;
      }
      if (!entry.name.endsWith('.tsx') || entry.name.endsWith('.test.tsx')) continue;
      if (/\bcreatePortal\s*\(/.test(readSource(relativePath))) discovered.push(relativePath);
    }
  };

  visit('');
  return discovered.sort();
};

const resolvePortalStylePath = (sourcePath: string): string | undefined => {
  const override = portalStyleOverrides[sourcePath];
  if (override) return override;

  const stem = sourcePath.replace(/\.tsx$/, '');
  return [`${stem}.scss`, `${stem}.css`].find(candidate => existsSync(join(SOURCE_ROOT, candidate)));
};

const discoverSurfaceStyleFiles = (): string[] => {
  const discovered: string[] = [];

  const visit = (relativeDirectory: string): void => {
    const absoluteDirectory = join(SOURCE_ROOT, relativeDirectory);
    for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
      const relativePath = relativeDirectory ? `${relativeDirectory}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        visit(relativePath);
        continue;
      }
      if (!entry.name.endsWith('.scss')) continue;
      if (/@include surfaces\.(?:floating-surface|dialog-surface)/.test(readSource(relativePath))) {
        discovered.push(relativePath);
      }
    }
  };

  visit('');
  return discovered.sort();
};

const findDirectChromeOverrides = (path: string): string[] => {
  const lines = readSource(path).split(/\r?\n/);
  const violations: string[] = [];
  const chromeDeclaration = /^(?:border(?:-(?:color|width|style))?|border-radius|background(?:-color)?|box-shadow)\s*:/;

  for (let index = 0; index < lines.length; index += 1) {
    const includeMatch = /^(\s*)@include surfaces\.(?:floating-surface|dialog-surface)/.exec(lines[index]);
    if (!includeMatch) continue;

    const declarationIndent = includeMatch[1];
    let blockStart = index - 1;
    while (blockStart >= 0) {
      const line = lines[blockStart];
      const indent = /^(\s*)/.exec(line)?.[1] ?? '';
      if (line.includes('{') && indent.length < declarationIndent.length) break;
      blockStart -= 1;
    }

    for (let cursor = Math.max(0, blockStart + 1); cursor < lines.length; cursor += 1) {
      const line = lines[cursor];
      const indent = /^(\s*)/.exec(line)?.[1] ?? '';
      const trimmed = line.trim();
      if (trimmed.startsWith('}') && indent.length < declarationIndent.length) break;
      if (indent === declarationIndent && chromeDeclaration.test(trimmed)) {
        violations.push(`${path}:${cursor + 1}:${trimmed}`);
      }
    }
  }

  return violations;
};

describe('overlay surface contracts', () => {
  it('keeps exactly two public surface contracts on one private chrome owner', () => {
    const source = readSource('component-library/styles/_overlay-surfaces.scss');
    const publicMixins = [...source.matchAll(/@mixin\s+([a-z][\w-]*)/g)].map(
      match => match[1],
    );

    expect(source).toContain('@mixin -popup-card-chrome($background, $backdrop-blur: false)');
    expect(publicMixins).toEqual(['floating-surface', 'dialog-surface']);
    expect(source).toContain('@mixin floating-surface');
    expect(source).toContain('@mixin dialog-surface');
    expect(source.match(/@include -popup-card-chrome\(/g)).toHaveLength(2);

    expect(source.match(/border: 1px solid var\(--bf-appearance-token-border-base\)/g))
      .toHaveLength(1);
    expect(source.match(/border-radius: tokens\.\$size-radius-lg/g)).toHaveLength(1);
    expect(source.match(/background: \$background/g)).toHaveLength(1);
    expect(source.match(/box-shadow: var\(--bf-appearance-token-shadow-lg\)/g)).toHaveLength(1);
    expect(source).toContain('border: 1px solid var(--bf-appearance-token-border-base)');
    expect(source).toContain('border-radius: tokens.$size-radius-lg');
    expect(source).toContain('var(--bf-appearance-token-color-bg-elevated) 94%');
    expect(source).toContain(
      '$background: var(--bf-appearance-token-color-bg-elevated)',
    );
    expect(source).toContain('box-shadow: var(--bf-appearance-token-shadow-lg)');
    expect(source).toContain('backdrop-filter: blur(16px)');
    expect(source).not.toContain('border-medium');
    expect(source).not.toContain('0 14px 42px -12px');
  });

  it('keeps the surface contracts style-only without a redundant wrapper component', () => {
    expect(
      existsSync(join(SOURCE_ROOT, 'component-library/components/OverlaySurface')),
    ).toBe(false);
  });

  it('forbids every surface consumer from redefining canonical outer chrome', () => {
    const violations = discoverSurfaceStyleFiles().flatMap(findDirectChromeOverrides);
    expect(violations).toEqual([]);
  });

  it('keeps Modal Appearance and About content from creating another dialog shell', () => {
    const builtinSource = readSource(
      'infrastructure/appearance/builtins/buildBuiltinAppearance.ts',
    );
    const modalSource = builtinSource.slice(builtinSource.indexOf('      modal: {'));
    const dialogSource = modalSource.slice(
      modalSource.indexOf('          dialog: {'),
      modalSource.indexOf('            facets:'),
    );

    expect(dialogSource).toContain("maxHeight: { kind: 'percent', value: 100 }");
    expect(dialogSource).not.toMatch(
      /backgroundColor|borderColor|borderStyle|borderWidth|borderRadius|boxShadow/,
    );

    const aboutAppearance = readSource('app/components/AboutDialog/appearance.ts');
    expect(aboutAppearance).toContain("{ id: 'root', visualRole: 'content'");
    expect(aboutAppearance).not.toContain("{ id: 'root', visualRole: 'dialog'");
  });

  it('keeps the Modal close inset on the header shell', () => {
    const modalStyles = readSource('component-library/components/Modal/Modal.scss');

    expect(modalStyles).toContain('padding-inline-end: var(--modal-close-edge-gutter);');
    expect(modalStyles).toContain('margin-block: var(--modal-close-edge-gutter);');
    expect(modalStyles).not.toContain('margin-inline-end: var(--modal-close-edge-gutter);');
  });

  it('keeps About styles limited to rendered content without redundant visual branches', () => {
    const aboutStyles = readSource('app/components/AboutDialog/AboutDialog.scss');
    const aboutSource = readSource('app/components/AboutDialog/AboutDialog.tsx');
    const styleClasses = [...aboutStyles.matchAll(/\.bitfun-about-dialog__[a-z0-9_-]+/g)]
      .map(match => match[0].slice(1));
    const sourceClasses = new Set(
      [...aboutSource.matchAll(/bitfun-about-dialog__[a-z0-9_-]+/g)]
        .map(match => match[0]),
    );

    expect([...new Set(styleClasses)].filter(className => !sourceClasses.has(className)))
      .toEqual([]);
    expect(aboutSource).not.toContain('titleExtra=');
    expect(aboutSource).not.toContain('about-header-logo');
    expect(aboutStyles).not.toContain('bitfun-about-dialog__dependencies-');
    expect(aboutStyles).not.toContain('bitfun-about-dialog__dependency-');
  });

  it.each([
    'component-library/components/Select/Select.scss',
    'shared/context-menu-system/components/ui/ContextMenu.scss',
    'flow_chat/components/modern/SessionTreePopover.scss',
    'flow_chat/components/ChatInput.scss',
    'flow_chat/components/ModelSelector.scss',
    'app/components/NavPanel/NavPanel.scss',
    'app/layout/FloatingMiniChat.scss',
    'tools/lsp/components/ReferencesPanel/ReferencesPanel.scss',
  ])('%s consumes FloatingSurface instead of owning popup chrome', (path) => {
    expect(readSource(path)).toContain('@include surfaces.floating-surface');
  });

  it.each([
    'app/components/AgentCompanionDesktopPet/AgentCompanionDesktopPet.scss',
    'app/layout/AppLayout.scss',
    'app/layout/FloatingMiniChat.scss',
    'app/components/NavPanel/components/BranchQuickSwitch.scss',
    'infrastructure/i18n/components/LanguageSelector.scss',
    'shared/announcement-system/styles/AnnouncementToast.scss',
    'shared/notification-system/components/LoadingNotification.scss',
    'shared/notification-system/components/NotificationItem.scss',
    'shared/notification-system/components/ProgressNotification.scss',
    'tools/editor/meditor/components/TiptapEditor.scss',
    'tools/git/components/PushButton/PushButton.scss',
  ])('%s consumes FloatingSurface for its non-portal transient card', (path) => {
    expect(readSource(path)).toContain('@include surfaces.floating-surface');
  });

  it('keeps the transparent companion window on shared chrome without backdrop blur', () => {
    const source = readSource(
      'app/components/AgentCompanionDesktopPet/AgentCompanionDesktopPet.scss',
    );

    expect(source).toContain(
      '@include surfaces.floating-surface($backdrop-blur: false)',
    );
    expect(source).not.toContain('border-radius: 10px;');
    expect(source).not.toContain('box-shadow: 0 6px 18px');
  });

  it('requires every React portal surface to consume a contract or declare a semantic exception', () => {
    const portalSources = discoverPortalSourceFiles();

    for (const sourcePath of portalSources) {
      const exceptionReason = portalSurfaceExceptions[sourcePath];
      if (exceptionReason) {
        expect(exceptionReason.length).toBeGreaterThan(0);
        continue;
      }

      const stylePath = resolvePortalStylePath(sourcePath);
      expect(stylePath, `${sourcePath} must resolve to an overlay stylesheet`).toBeDefined();
      const styleSource = readSource(stylePath!);
      expect(
        styleSource.includes('@include surfaces.floating-surface')
          || styleSource.includes('@include surfaces.dialog-surface'),
        `${sourcePath} must consume FloatingSurface or DialogSurface through ${stylePath}`,
      ).toBe(true);
    }

    expect(
      Object.keys(portalSurfaceExceptions).filter(path => !portalSources.includes(path)),
    ).toEqual([]);
  });

  it.each([
    'component-library/components/Modal/Modal.scss',
    'app/components/panels/BranchSelectModal.scss',
    'infrastructure/peer-device/PeerDirectoryBrowser.scss',
    'features/ssh-remote/RemoteFileBrowser.scss',
    'shared/announcement-system/styles/FeatureModal.scss',
    'tools/git/components/GitDiffView/GitDiffView.scss',
  ])('%s consumes DialogSurface instead of owning modal chrome', (path) => {
    expect(readSource(path)).toContain('@include surfaces.dialog-surface');
  });

  it('keeps content cards outside the overlay contract', () => {
    expect(readSource('component-library/components/Card/Card.scss')).not.toContain('overlay-surfaces');
  });

  it('does not let product-specific Modal layouts replace the shared outer chrome', () => {
    const usageModal = readSource('flow_chat/components/usage/SessionUsageModal.scss');
    const globalSearch = readSource('app/global-search/GlobalSearchRoot.scss');
    const deepReview = readSource('flow_chat/components/DeepReviewConsentDialog.scss');
    const createBranchRoot = readSource(
      'tools/git/components/CreateBranchDialog/CreateBranchDialog.scss',
    ).split('&__header')[0];

    expect(usageModal).not.toContain('0 18px 48px var(--bf-appearance-token-color-overlay-black-20)');
    expect(usageModal).not.toContain('border-radius: 16px');
    expect(globalSearch).not.toContain('box-shadow: 0 18px 48px var(--bf-appearance-token-color-overlay-black-30)');
    expect(globalSearch).not.toContain('border-radius: 14px');
    expect(globalSearch).not.toMatch(
      /& > \.modal\.modal--xlarge \{[^}]*background:/,
    );
    expect(deepReview).not.toContain('.modal:has(.deep-review-consent)');
    expect(createBranchRoot).not.toContain('background:');
    expect(createBranchRoot).not.toContain('border:');
    expect(createBranchRoot).not.toContain('border-radius:');
    expect(createBranchRoot).not.toContain('box-shadow:');
  });

  it('removes legacy one-off chrome from transient cards migrated after the portal audit', () => {
    expect(readSource('app/layout/AppLayout.scss')).not.toContain(
      'box-shadow: 0 12px 28px var(--bf-appearance-token-color-overlay-black-30)',
    );
    expect(readSource('app/components/panels/content-canvas/quick-look/QuickLook.scss'))
      .not.toContain('box-shadow: 0 8px 32px var(--bf-appearance-token-color-overlay-black-50)');
    expect(readSource('shared/announcement-system/styles/AnnouncementToast.scss'))
      .not.toContain('box-shadow: 0 6px 24px');

    for (const path of [
      'shared/notification-system/components/LoadingNotification.scss',
      'shared/notification-system/components/NotificationItem.scss',
      'shared/notification-system/components/ProgressNotification.scss',
    ]) {
      expect(readSource(path)).not.toContain(
        'box-shadow: 0 2px 6px var(--bf-appearance-token-color-overlay-black-08)',
      );
    }
  });
});
