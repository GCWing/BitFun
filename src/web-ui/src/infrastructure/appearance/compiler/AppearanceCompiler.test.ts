import { describe, expect, it } from 'vitest';
import { bitfunDarkPalette, builtinAppearancePalettes } from '../builtins/palettes';
import { buildBuiltinAppearance } from '../builtins/buildBuiltinAppearance';
import { composeAppearancePackage } from '../builtins/composeAppearancePackage';
import { createDefaultAppearanceRegistry } from '../registry/defaultAppearanceRegistry';
import { AppearancePackageValidationError } from '../schema/AppearancePackageValidationError';
import type { AppearancePackage } from '../types';
import { AppearanceCompiler } from './AppearanceCompiler';

describe('AppearanceCompiler', () => {
  it('preserves structured validation diagnostics when compilation is rejected', () => {
    const pkg = {
      ...buildBuiltinAppearance(bitfunDarkPalette),
      components: {
        'toolbar-mode': {
          parts: {
            sessionMenu: { base: { opacity: { kind: 'number', value: 1 } } },
          },
        },
      },
    };

    expect(() => new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1))
      .toThrow(AppearancePackageValidationError);
  });

  it('compiles a built-in appearance into host-owned component selectors', () => {
    const compiler = new AppearanceCompiler(createDefaultAppearanceRegistry());
    const snapshot = compiler.compile(buildBuiltinAppearance(bitfunDarkPalette), 3);

    expect(snapshot.id).toBe(bitfunDarkPalette.id);
    expect(snapshot.revision).toBe(3);
    expect(snapshot.cssText).toContain('--bf-appearance-colors-bg-primary');
    expect(snapshot.cssText).toContain('[data-bf-component="button"][data-bf-part="root"]');
    expect(snapshot.cssText).toContain('[data-bf-variant="primary"]:hover:not(:disabled)');
    expect(snapshot.cssText).not.toContain('@layer');
    expect(snapshot.cssText).not.toContain(' !important;');
    expect(snapshot.cssText).not.toContain('.btn-primary');
  });

  it('compiles every built-in appearance without raw CSS passthrough', () => {
    const compiler = new AppearanceCompiler(createDefaultAppearanceRegistry());
    builtinAppearancePalettes.forEach((palette, index) => {
      const snapshot = compiler.compile(buildBuiltinAppearance(palette), index + 1);
      expect(snapshot.mode).toBe(palette.type);
      expect(snapshot.cssText).not.toContain('url(');
    });
  });

  it('keeps built-in UI motion within the responsive interaction budget', () => {
    const seconds = (value: string) => Number.parseFloat(value.replace('s', ''));

    builtinAppearancePalettes.forEach((palette) => {
      expect(seconds(palette.motion.duration.fast)).toBeLessThanOrEqual(0.16);
      expect(seconds(palette.motion.duration.base)).toBeLessThanOrEqual(0.24);
      expect(seconds(palette.motion.duration.slow)).toBeLessThanOrEqual(0.45);
      expect(palette.motion.easing.standard).toBe('cubic-bezier(0.23, 1, 0.32, 1)');
    });
  });

  it('calculates real contrast diagnostics for structured colors', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.low-contrast',
      name: 'Low Contrast',
      version: '1.0.0',
      mode: 'light',
      components: {
        button: {
          parts: {
            root: {
              base: {
                backgroundColor: { kind: 'hex', value: '#ffffff' },
                color: { kind: 'hex', value: '#eeeeee' },
              },
            },
          },
        },
      },
    };
    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.diagnostics).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'LOW_CONTRAST' }),
    ]));
  });

  it('uses important declarations only for explicitly overriding parts', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.cascade-override',
      name: 'Cascade Override',
      version: '1.0.0',
      mode: 'dark',
      components: {
        button: {
          parts: {
            root: {
              cascade: 'override',
              base: { color: { kind: 'hex', value: '#ffffff' } },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('color:#ffffff !important;');
  });

  it('keeps non-forceable layout declarations in the normal cascade', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.property-level-override',
      name: 'Property Level Override',
      version: '1.0.0',
      mode: 'dark',
      components: {
        'content-canvas': {
          parts: {
            root: {
              cascade: 'override',
              base: {
                backgroundColor: { kind: 'hex', value: '#101010' },
                position: 'relative',
              },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('background-color:#101010 !important;');
    expect(snapshot.cssText).toContain('position:relative;');
    expect(snapshot.cssText).not.toContain('position:relative !important;');
    expect(snapshot.diagnostics).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'OVERRIDE_PROPERTY_NOT_FORCEABLE' }),
    ]));
  });

  it('normalizes side borders and outlines into deterministic CSS', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.border-normalization',
      name: 'Border Normalization',
      version: '1.0.0',
      mode: 'light',
      components: {
        button: {
          parts: {
            root: {
              base: {
                borderStyle: 'solid',
                borderBottomWidth: { kind: 'px', value: 1 },
                outlineWidth: { kind: 'px', value: 2 },
                outlineColor: { kind: 'hex', value: '#8866ff' },
              },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('border-width:0;border-bottom-width:1px;border-style:solid;');
    expect(snapshot.cssText).toContain('outline-color:#8866ff;outline-width:2px;outline-style:solid;');
    expect(snapshot.diagnostics).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'BORDER_WIDTH_NORMALIZED' }),
      expect.objectContaining({ code: 'OUTLINE_STYLE_NORMALIZED' }),
    ]));
  });

  it('combines materials in declaration order before applying the part base', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.material-composition',
      name: 'Material Composition',
      version: '1.0.0',
      mode: 'dark',
      materials: {
        surface: {
          visualRole: 'control',
          style: {
            backgroundColor: { kind: 'hex', value: '#202020' },
            borderRadius: { kind: 'px', value: 6 },
          },
        },
        accent: {
          visualRole: 'control',
          style: {
            color: { kind: 'hex', value: '#ff66cc' },
            backgroundColor: { kind: 'hex', value: '#303030' },
          },
        },
      },
      components: {
        button: {
          parts: {
            root: {
              materials: ['surface', 'accent'],
              base: { backgroundColor: { kind: 'hex', value: '#404040' } },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('background-color:#404040;');
    expect(snapshot.cssText).toContain('color:#ff66cc;');
    expect(snapshot.cssText).toContain('border-radius:6px;');
  });

  it('does not promote inherited baseline declarations when an imported part overrides its own fields', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.layered-cascade',
      name: 'Layered Cascade',
      version: '1.0.0',
      mode: 'light',
      components: {
        button: {
          parts: {
            root: {
              cascade: 'override',
              base: { borderRadius: { kind: 'px', value: 2 } },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(
      composeAppearancePackage(pkg),
      1,
    );
    expect(snapshot.cssText).toContain('border-radius:2px !important;');
    expect(snapshot.cssText).toContain('display:flex;');
    expect(snapshot.cssText).not.toContain('display:flex !important;');
  });

  it('compiles structured layout, media, transform, and filter values without CSS passthrough', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.structured-style',
      name: 'Structured Style',
      version: '1.0.0',
      mode: 'dark',
      components: {
        'content-canvas': {
          parts: {
            root: {
              base: {
                display: 'grid',
                width: { kind: 'lengthKeyword', value: 'fit-content' },
                paddingTop: { kind: 'vh', value: 2 },
                marginInline: { kind: 'lengthKeyword', value: 'auto' },
                gridTemplateColumns: {
                  kind: 'gridTracks',
                  tracks: [{ kind: 'fr', value: 1 }, { kind: 'px', value: 24 }],
                },
                aspectRatio: { kind: 'ratio', width: 16, height: 9 },
                objectFit: 'cover',
                justifyItems: 'center',
                order: { kind: 'number', value: 2 },
                caretColor: { kind: 'hex', value: '#33ffaa' },
                transform: { kind: 'transform', rotate: 12, scale: 1.05 },
                filter: [
                  { kind: 'blur', value: { kind: 'px', value: 2 } },
                  { kind: 'saturate', value: { kind: 'number', value: 1.2 } },
                ],
              },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('grid-template-columns:1fr 24px;');
    expect(snapshot.cssText).toContain('width:fit-content;');
    expect(snapshot.cssText).toContain('padding-top:2vh;');
    expect(snapshot.cssText).toContain('margin-inline:auto;');
    expect(snapshot.cssText).toContain('justify-items:center;');
    expect(snapshot.cssText).toContain('order:2;');
    expect(snapshot.cssText).toContain('caret-color:#33ffaa;');
    expect(snapshot.cssText).toContain('aspect-ratio:16 / 9;');
    expect(snapshot.cssText).toContain('object-fit:cover;');
    expect(snapshot.cssText).toContain('transform:rotate(12deg) scale(1.05);');
    expect(snapshot.cssText).toContain('filter:blur(2px) saturate(1.2);');
  });

  it('compiles layered package assets into aligned host-owned background declarations', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.layered-backgrounds',
      name: 'Layered Backgrounds',
      version: '1.0.0',
      mode: 'light',
      assets: {
        backdrop: { kind: 'image', mimeType: 'image/webp', source: { kind: 'package', path: 'assets/backdrop.webp' } },
        texture: { kind: 'image', mimeType: 'image/png', source: { kind: 'package', path: 'assets/texture.png' } },
        ornament: { kind: 'image', mimeType: 'image/png', source: { kind: 'package', path: 'assets/ornament.png' } },
      },
      components: {
        card: {
          parts: {
            root: {
              base: {
                backgroundImages: [
                  { kind: 'asset', assetId: 'ornament' },
                  { kind: 'asset', assetId: 'texture' },
                  { kind: 'asset', assetId: 'backdrop' },
                ],
                backgroundSizes: [
                  { kind: 'backgroundSize', width: { kind: 'px', value: 96 }, height: { kind: 'lengthKeyword', value: 'auto' } },
                  { kind: 'backgroundSize', width: { kind: 'px', value: 256 }, height: { kind: 'lengthKeyword', value: 'auto' } },
                  'cover',
                ],
                backgroundPositions: [
                  { kind: 'backgroundPosition', x: { kind: 'percent', value: 100 }, y: { kind: 'percent', value: 100 } },
                  { kind: 'backgroundPosition', x: { kind: 'percent', value: 0 }, y: { kind: 'percent', value: 0 } },
                  { kind: 'backgroundPosition', x: { kind: 'percent', value: 65 }, y: { kind: 'percent', value: 50 } },
                ],
                backgroundRepeats: ['no-repeat', 'repeat', 'no-repeat'],
                backgroundBlendModes: ['normal', 'soft-light', 'normal'],
              },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain(
      'background-image:var(--bf-appearance-asset-ornament, none), var(--bf-appearance-asset-texture, none), var(--bf-appearance-asset-backdrop, none);',
    );
    expect(snapshot.cssText).toContain('background-size:96px auto, 256px auto, cover;');
    expect(snapshot.cssText).toContain('background-position:100% 100%, 0% 0%, 65% 50%;');
    expect(snapshot.cssText).toContain('background-repeat:no-repeat, repeat, no-repeat;');
    expect(snapshot.cssText).toContain('background-blend-mode:normal, soft-light, normal;');
    expect(snapshot.cssText).not.toContain('url(');
  });

  it('compiles ancestor-owned state selectors for descendant parts', () => {
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.ancestor-state',
      name: 'Ancestor State',
      version: '1.0.0',
      mode: 'dark',
      components: {
        checkbox: {
          parts: {
            box: {
              states: {
                checked: { backgroundColor: { kind: 'hex', value: '#22cc88' } },
              },
            },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain(
      '[data-bf-component="checkbox"][data-bf-part="root"]:has(input:checked) [data-bf-component="checkbox"][data-bf-part="box"]',
    );
  });

  it('targets multiple parts, facets, and states across complex product surfaces', () => {
    const accent = { kind: 'hex', value: '#ff3366' } as const;
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.product-surfaces',
      name: 'Product Surfaces',
      version: '1.0.0',
      mode: 'dark',
      components: {
        'about-dialog': {
          parts: {
            hero: { base: { backgroundColor: accent } },
            title: { base: { color: { kind: 'hex', value: '#ffffff' } } },
            progressFill: { states: { downloading: { backgroundColor: accent } } },
          },
        },
        'nav-panel': {
          parts: {
            topAction: {
              contexts: [{
                when: { facets: { action: 'assistant' }, states: ['active'] },
                style: { backgroundColor: accent },
              }],
            },
          },
        },
        'canvas-editor-area': {
          parts: {
            root: {
              facets: { layout: { grid: { gap: { kind: 'px', value: 8 } } } },
            },
          },
        },
        'canvas-tab': {
          parts: {
            root: { states: { active: { borderBottomColor: accent } } },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('[data-bf-component="about-dialog"][data-bf-part="hero"]');
    expect(snapshot.cssText).toContain('[data-bf-component="about-dialog"][data-bf-part="updateCard"][data-bf-state~="downloading"] [data-bf-component="about-dialog"][data-bf-part="progressFill"]');
    expect(snapshot.cssText).toContain('[data-bf-component="nav-panel"][data-bf-part="topAction"][data-bf-action="assistant"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="canvas-editor-area"][data-bf-part="root"][data-bf-layout="grid"]');
    expect(snapshot.cssText).toContain('[data-bf-component="canvas-tab"][data-bf-part="root"][data-bf-state~="active"]');
  });

  it('compiles dedicated contracts for large interactive owners', () => {
    const accent = { kind: 'hex', value: '#33ffaa' } as const;
    const pkg: AppearancePackage = {
      schema: 'bitfun.appearance',
      schemaVersion: 1,
      id: 'test.large-owners',
      name: 'Large Owners',
      version: '1.0.0',
      mode: 'dark',
      components: {
        'chat-input': {
          parts: {
            target: {
              contexts: [{
                when: { facets: { target: 'btw' }, states: ['selected'] },
                style: { backgroundColor: accent },
              }],
            },
          },
        },
        'virtual-message-list': {
          parts: {
            boundaryStatus: {
              states: { unavailable: { color: accent } },
            },
          },
        },
        'ai-model-config': {
          parts: {
            root: {
              facets: { view: { selection: { backgroundColor: accent } } },
            },
          },
        },
        'external-sources-config': {
          parts: {
            conflict: { base: { borderColor: accent } },
          },
        },
        'review-platform': {
          parts: {
            listItem: { states: { selected: { backgroundColor: accent } } },
          },
        },
        'modern-flow-chat': {
          parts: {
            historyOpenIntent: { base: { backgroundColor: accent } },
          },
        },
        'session-config': {
          parts: {
            templateCard: { states: { expanded: { borderColor: accent } } },
          },
        },
        'mcp-tools-config': {
          parts: {
            root: { facets: { view: { json: { backgroundColor: accent } } } },
            authEditor: { base: { borderColor: accent } },
          },
        },
        'sessions-section': {
          parts: {
            row: { states: { active: { backgroundColor: accent } } },
          },
        },
        'files-panel': {
          parts: {
            search: { facets: { searchMode: { content: { borderColor: accent } } } },
          },
        },
        'remote-connect-dialog': {
          parts: {
            groupTab: {
              contexts: [{
                when: { facets: { group: 'account' }, states: ['active'] },
                style: { backgroundColor: accent },
              }],
            },
          },
        },
        'session-usage-panel': {
          parts: {
            tab: {
              contexts: [{
                when: { facets: { tab: 'models' }, states: ['active'] },
                style: { color: accent },
              }],
            },
          },
        },
        'file-operation-tool-card': {
          parts: {
            root: {
              contexts: [{
                when: { facets: { action: 'modify' }, states: ['expanded'] },
                style: { borderColor: accent },
              }],
            },
          },
        },
        'acp-agents-config': {
          parts: {
            root: { facets: { view: { json: { backgroundColor: accent } } } },
            remoteServer: { base: { borderColor: accent } },
          },
        },
        'tiptap-editor': {
          parts: {
            inlineAiPanel: { base: { backgroundColor: accent } },
          },
        },
        'scheduled-jobs-view': {
          parts: {
            root: { facets: { target: { workspace: { backgroundColor: accent } } } },
            job: { states: { expanded: { borderColor: accent } } },
          },
        },
        'deep-review-action-bar': {
          parts: {
            root: {
              contexts: [{
                when: { facets: { phase: 'review_completed', variant: 'success' } },
                style: { borderColor: accent },
              }],
            },
          },
        },
        'rich-text-input': {
          parts: {
            contextTag: { facets: { contextType: { 'widget-reference': { backgroundColor: accent } } } },
          },
        },
        'model-round-item': {
          parts: {
            root: { facets: { status: { streaming: { backgroundColor: accent } } } },
            action: { states: { copied: { color: accent } } },
          },
        },
        'flexible-panel': {
          parts: {
            code: { states: { needsFix: { borderColor: accent } } },
          },
        },
        'btw-session-panel': {
          parts: {
            root: { facets: { view: { session: { backgroundColor: accent } } } },
          },
        },
        'model-selector': {
          parts: {
            trigger: { states: { open: { borderColor: accent } } },
          },
        },
        'flow-chat-header': {
          parts: {
            turnBadge: { base: { backgroundColor: accent } },
          },
        },
        'session-files-badge': {
          parts: {
            file: { facets: { operation: { modify: { borderColor: accent } } } },
          },
        },
        'code-review-tool-card': {
          parts: {
            group: { states: { expanded: { backgroundColor: accent } } },
          },
        },
        'create-agent-page': {
          parts: {
            levelOption: { states: { active: { backgroundColor: accent } } },
          },
        },
        'keyboard-shortcuts': {
          parts: {
            keyBadge: { states: { recording: { borderColor: accent } } },
          },
        },
        'task-tool-display': {
          parts: {
            root: { states: { failed: { borderColor: accent } } },
          },
        },
        'basics-config': {
          parts: {
            notifications: { base: { backgroundColor: accent } },
          },
        },
        'markdown-editor': {
          parts: {
            root: { facets: { view: { source: { backgroundColor: accent } } } },
          },
        },
        'plan-viewer': {
          parts: {
            editorPanel: { states: { expanded: { borderColor: accent } } },
          },
        },
        'terminal-tool': {
          parts: {
            screen: { base: { backgroundColor: accent } },
          },
        },
        'app-layout': {
          parts: {
            root: { states: { toolbar: { backgroundColor: accent } } },
          },
        },
        'skill-group-picker': {
          parts: {
            token: { states: { selected: { borderColor: accent } } },
          },
        },
        'working-copy-view': {
          parts: {
            file: { states: { selected: { backgroundColor: accent } } },
          },
        },
        'assistant-config-page': {
          parts: {
            persona: { states: { selected: { borderColor: accent } } },
          },
        },
        'assistant-defaults-page': {
          parts: {
            skill: { states: { covered: { backgroundColor: accent } } },
          },
        },
        'task-detail-panel': {
          parts: {
            root: { states: { empty: { backgroundColor: accent } } },
          },
        },
        'toolbar-mode': {
          parts: {
            root: { states: { expanded: { borderColor: accent } } },
          },
        },
        'mcp-tool-display': {
          parts: {
            expanded: { states: { expanded: { backgroundColor: accent } } },
          },
        },
        'terminal-tool-card': {
          parts: {
            root: { facets: { status: { running: { borderColor: accent } } } },
          },
        },
        'skills-config': {
          parts: {
            marketItem: { states: { installed: { borderColor: accent } } },
          },
        },
        'diff-editor': {
          parts: {
            loading: { states: { loading: { backgroundColor: accent } } },
          },
        },
        'agent-companion-desktop-pet': {
          parts: { hitbox: { states: { attention: { borderColor: accent } } } },
        },
        'tool-group-picker': {
          parts: { token: { states: { selected: { backgroundColor: accent } } } },
        },
        'inline-diff-preview': {
          parts: { root: { states: { empty: { backgroundColor: accent } } } },
        },
        'export-image': {
          parts: { trigger: { states: { exporting: { color: accent } } } },
        },
        'user-message-item': {
          parts: { root: { states: { failed: { borderColor: accent } } } },
        },
        'session-usage-report-card': {
          parts: { loading: { states: { loading: { backgroundColor: accent } } } },
        },
        'ask-user-question-card': {
          parts: { root: { states: { completed: { borderColor: accent } } } },
        },
        'create-plan-display': {
          parts: { todos: { states: { expanded: { backgroundColor: accent } } } },
        },
        'exec-process-tool-card': {
          parts: { waiting: { states: { waiting: { backgroundColor: accent } } } },
        },
        'editor-config': {
          parts: { saving: { states: { saving: { color: accent } } } },
        },
        'workspace-project-permissions-dialog': {
          parts: { rule: { base: { borderColor: accent } } },
        },
        'workspace-session-batch-modal': {
          parts: { row: { states: { selected: { backgroundColor: accent } } } },
        },
        'archived-sessions-config': {
          parts: { group: { states: { collapsed: { borderColor: accent } } } },
        },
        'settings-nav': {
          parts: { item: { states: { active: { backgroundColor: accent } } } },
        },
        'background-command-output-panel': {
          parts: { root: { states: { error: { borderColor: accent } } } },
        },
        'chat-input-pixel-pet': {
          parts: { root: { facets: { mood: { working: { backgroundColor: accent } } } } },
        },
        'file-mention-picker': {
          parts: { item: { states: { selected: { backgroundColor: accent } } } },
        },
        'session-file-modifications-bar': {
          parts: { file: { facets: { operation: { edit: { borderColor: accent } } } } },
        },
        'editor-breadcrumb': {
          parts: { item: { states: { active: { backgroundColor: accent } } } },
        },
        'git-branch-history': {
          parts: { commit: { states: { expanded: { borderColor: accent } } } },
        },
        'git-diff-view': {
          parts: { typeOption: { states: { active: { backgroundColor: accent } } } },
        },
        'git-settings-view': {
          parts: { status: { states: { error: { borderColor: accent } } } },
        },
      },
      scenes: {
        agents: {
          parts: {
            coreGrid: { base: { borderColor: accent } },
          },
        },
      },
    };

    const snapshot = new AppearanceCompiler(createDefaultAppearanceRegistry()).compile(pkg, 1);
    expect(snapshot.cssText).toContain('[data-bf-component="chat-input"][data-bf-part="target"][data-bf-target="btw"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="virtual-message-list"][data-bf-part="boundaryStatus"][data-bf-state~="unavailable"]');
    expect(snapshot.cssText).toContain('[data-bf-component="ai-model-config"][data-bf-part="root"][data-bf-view="selection"]');
    expect(snapshot.cssText).toContain('[data-bf-component="external-sources-config"][data-bf-part="conflict"]');
    expect(snapshot.cssText).toContain('[data-bf-component="review-platform"][data-bf-part="listItem"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="modern-flow-chat"][data-bf-part="historyOpenIntent"]');
    expect(snapshot.cssText).toContain('[data-bf-component="session-config"][data-bf-part="templateCard"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="mcp-tools-config"][data-bf-part="root"][data-bf-view="json"]');
    expect(snapshot.cssText).toContain('[data-bf-component="mcp-tools-config"][data-bf-part="authEditor"]');
    expect(snapshot.cssText).toContain('[data-bf-component="sessions-section"][data-bf-part="row"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="files-panel"][data-bf-part="search"][data-bf-search-mode="content"]');
    expect(snapshot.cssText).toContain('[data-bf-component="remote-connect-dialog"][data-bf-part="groupTab"][data-bf-group="account"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-scene="agents"][data-bf-part="coreGrid"]');
    expect(snapshot.cssText).toContain('[data-bf-component="session-usage-panel"][data-bf-part="tab"][data-bf-tab="models"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="file-operation-tool-card"][data-bf-part="root"][data-bf-action="modify"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="acp-agents-config"][data-bf-part="root"][data-bf-view="json"]');
    expect(snapshot.cssText).toContain('[data-bf-component="acp-agents-config"][data-bf-part="remoteServer"]');
    expect(snapshot.cssText).toContain('[data-bf-component="tiptap-editor"][data-bf-part="inlineAiPanel"]');
    expect(snapshot.cssText).toContain('[data-bf-component="scheduled-jobs-view"][data-bf-part="root"][data-bf-target="workspace"]');
    expect(snapshot.cssText).toContain('[data-bf-component="scheduled-jobs-view"][data-bf-part="job"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="deep-review-action-bar"][data-bf-part="root"][data-bf-phase="review_completed"][data-bf-variant="success"]');
    expect(snapshot.cssText).toContain('[data-bf-component="rich-text-input"][data-bf-part="contextTag"][data-bf-context-type="widget-reference"]');
    expect(snapshot.cssText).toContain('[data-bf-component="model-round-item"][data-bf-part="root"][data-bf-status="streaming"]');
    expect(snapshot.cssText).toContain('[data-bf-component="model-round-item"][data-bf-part="action"][data-bf-state~="copied"]');
    expect(snapshot.cssText).toContain('[data-bf-component="flexible-panel"][data-bf-part="code"][data-bf-state~="needsFix"]');
    expect(snapshot.cssText).toContain('[data-bf-component="btw-session-panel"][data-bf-part="root"][data-bf-view="session"]');
    expect(snapshot.cssText).toContain('[data-bf-component="model-selector"][data-bf-part="trigger"][data-bf-state~="open"]');
    expect(snapshot.cssText).toContain('[data-bf-component="flow-chat-header"][data-bf-part="turnBadge"]');
    expect(snapshot.cssText).toContain('[data-bf-component="session-files-badge"][data-bf-part="file"][data-bf-operation="modify"]');
    expect(snapshot.cssText).toContain('[data-bf-component="code-review-tool-card"][data-bf-part="group"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="create-agent-page"][data-bf-part="levelOption"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="keyboard-shortcuts"][data-bf-part="keyBadge"][data-bf-state~="recording"]');
    expect(snapshot.cssText).toContain('[data-bf-component="task-tool-display"][data-bf-part="root"][data-bf-state~="failed"]');
    expect(snapshot.cssText).toContain('[data-bf-component="basics-config"][data-bf-part="notifications"]');
    expect(snapshot.cssText).toContain('[data-bf-component="markdown-editor"][data-bf-part="root"][data-bf-view="source"]');
    expect(snapshot.cssText).toContain('[data-bf-component="plan-viewer"][data-bf-part="editorPanel"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="terminal-tool"][data-bf-part="screen"]');
    expect(snapshot.cssText).toContain('[data-bf-component="app-layout"][data-bf-part="root"][data-bf-state~="toolbar"]');
    expect(snapshot.cssText).toContain('[data-bf-component="skill-group-picker"][data-bf-part="token"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="working-copy-view"][data-bf-part="file"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="assistant-config-page"][data-bf-part="persona"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="assistant-defaults-page"][data-bf-part="skill"][data-bf-state~="covered"]');
    expect(snapshot.cssText).toContain('[data-bf-component="task-detail-panel"][data-bf-part="root"][data-bf-state~="empty"]');
    expect(snapshot.cssText).toContain('[data-bf-component="toolbar-mode"][data-bf-part="root"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="mcp-tool-display"][data-bf-part="expanded"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="terminal-tool-card"][data-bf-part="root"][data-bf-status="running"]');
    expect(snapshot.cssText).toContain('[data-bf-component="skills-config"][data-bf-part="marketItem"][data-bf-state~="installed"]');
    expect(snapshot.cssText).toContain('[data-bf-component="diff-editor"][data-bf-part="loading"][data-bf-state~="loading"]');
    expect(snapshot.cssText).toContain('[data-bf-component="agent-companion-desktop-pet"][data-bf-part="hitbox"][data-bf-state~="attention"]');
    expect(snapshot.cssText).toContain('[data-bf-component="tool-group-picker"][data-bf-part="token"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="inline-diff-preview"][data-bf-part="root"][data-bf-state~="empty"]');
    expect(snapshot.cssText).toContain('[data-bf-component="export-image"][data-bf-part="trigger"][data-bf-state~="exporting"]');
    expect(snapshot.cssText).toContain('[data-bf-component="user-message-item"][data-bf-part="root"][data-bf-state~="failed"]');
    expect(snapshot.cssText).toContain('[data-bf-component="session-usage-report-card"][data-bf-part="loading"][data-bf-state~="loading"]');
    expect(snapshot.cssText).toContain('[data-bf-component="ask-user-question-card"][data-bf-part="root"][data-bf-state~="completed"]');
    expect(snapshot.cssText).toContain('[data-bf-component="create-plan-display"][data-bf-part="todos"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="exec-process-tool-card"][data-bf-part="waiting"][data-bf-state~="waiting"]');
    expect(snapshot.cssText).toContain('[data-bf-component="editor-config"][data-bf-part="saving"][data-bf-state~="saving"]');
    expect(snapshot.cssText).toContain('[data-bf-component="workspace-project-permissions-dialog"][data-bf-part="rule"]');
    expect(snapshot.cssText).toContain('[data-bf-component="workspace-session-batch-modal"][data-bf-part="row"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="archived-sessions-config"][data-bf-part="group"][data-bf-state~="collapsed"]');
    expect(snapshot.cssText).toContain('[data-bf-component="settings-nav"][data-bf-part="item"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="background-command-output-panel"][data-bf-part="root"][data-bf-state~="error"]');
    expect(snapshot.cssText).toContain('[data-bf-component="chat-input-pixel-pet"][data-bf-part="root"][data-bf-mood="working"]');
    expect(snapshot.cssText).toContain('[data-bf-component="file-mention-picker"][data-bf-part="item"][data-bf-state~="selected"]');
    expect(snapshot.cssText).toContain('[data-bf-component="session-file-modifications-bar"][data-bf-part="file"][data-bf-operation="edit"]');
    expect(snapshot.cssText).toContain('[data-bf-component="editor-breadcrumb"][data-bf-part="item"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="git-branch-history"][data-bf-part="commit"][data-bf-state~="expanded"]');
    expect(snapshot.cssText).toContain('[data-bf-component="git-diff-view"][data-bf-part="typeOption"][data-bf-state~="active"]');
    expect(snapshot.cssText).toContain('[data-bf-component="git-settings-view"][data-bf-part="status"][data-bf-state~="error"]');
  });
});
