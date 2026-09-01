import { describe, expect, it } from 'vitest';
import { createDefaultAppearanceRegistry } from '../registry/defaultAppearanceRegistry';
import type { AppearancePackage } from '../types';
import { AppearancePackageValidationError } from './AppearancePackageValidationError';
import { appearancePackageValidator, assertValidAppearancePackage } from './AppearancePackageValidator';
import { migrateAppearancePackage } from './migrateAppearancePackage';

function validPackage(): AppearancePackage {
  return {
    schema: 'bitfun.appearance',
    schemaVersion: 1,
    id: 'test.appearance',
    name: 'Test Appearance',
    version: '1.0.0',
    mode: 'dark',
    components: {
      'gallery-layout': {
        parts: {
          root: {
            base: {
              backgroundColor: { kind: 'hex', value: '#101010' },
              color: { kind: 'hex', value: '#ffffff' },
            },
          },
        },
      },
    },
  };
}

describe('AppearancePackageValidator', () => {
  const registry = createDefaultAppearanceRegistry();

  it('accepts structured appearance data registered by the host', () => {
    const result = appearancePackageValidator.validate(validPackage(), registry);
    expect(result.valid).toBe(true);
    expect(result.errors).toEqual([]);
  });

  it('rejects missing, mistyped, and circular token references before compilation', () => {
    const raw = validPackage();
    raw.globals = {
      colors: {
        first: { kind: 'ref', path: 'globals.colors.second' },
        second: { kind: 'ref', path: 'globals.colors.first' },
      },
      lengths: {
        gap: { kind: 'px', value: 8 },
      },
    };
    raw.components = {
      'gallery-layout': {
        parts: {
          root: {
            base: {
              color: { kind: 'ref', path: 'globals.colors.missing' },
              backgroundColor: { kind: 'ref', path: 'globals.lengths.gap' },
            },
          },
        },
      },
    };

    const result = appearancePackageValidator.validate(raw, registry);

    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'UNKNOWN_TOKEN_REFERENCE' }),
      expect.objectContaining({ code: 'REFERENCE_TYPE_MISMATCH' }),
      expect.objectContaining({ code: 'CIRCULAR_TOKEN_REFERENCE' }),
    ]));
  });

  it('rejects unknown package schemas', () => {
    const result = appearancePackageValidator.validate({
      ...validPackage(),
      schema: 'example.unknown',
    }, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'INVALID_SCHEMA', path: 'schema' }),
    ]));
  });

  it('rejects raw CSS strings and unregistered parts', () => {
    const raw = validPackage() as unknown as Record<string, unknown>;
    const components = raw.components as Record<string, { parts: Record<string, unknown> }>;
    components['gallery-layout'].parts.root = { base: { backgroundColor: 'url(https://example.com/a.png)' } };
    components['gallery-layout'].parts.internalClass = { base: { color: { kind: 'hex', value: '#fff' } } };

    const result = appearancePackageValidator.validate(raw, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'INVALID_COLOR' }),
      expect.objectContaining({ code: 'UNKNOWN_PART' }),
    ]));
  });

  it('rejects unregistered component ids instead of compiling package selectors', () => {
    const raw = validPackage();
    raw.components = {
      'private-widget': { parts: { root: { base: { opacity: { kind: 'number', value: 1 } } } } },
    };
    const result = appearancePackageValidator.validate(raw, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'UNKNOWN_SURFACE', path: 'components.private-widget' }),
    ]));
  });

  it('drops retired menu parts and states while preserving supported appearance rules', () => {
    const legacy = validPackage() as unknown as Record<string, unknown>;
    legacy.components = {
      'branch-quick-switch': {
        parts: {
          root: {
            base: { opacity: { kind: 'number', value: 1 } },
            states: { selected: { opacity: { kind: 'number', value: 0.8 } } },
            contexts: [{
              when: { states: ['current'] },
              style: { opacity: { kind: 'number', value: 0.9 } },
            }],
          },
          item: { base: { opacity: { kind: 'number', value: 1 } } },
        },
      },
      'context-menu': {
        parts: {
          root: {
            base: { opacity: { kind: 'number', value: 1 } },
            states: { disabled: { opacity: { kind: 'number', value: 0.5 } } },
          },
          submenu: { base: { opacity: { kind: 'number', value: 1 } } },
        },
      },
    };

    const migrated = migrateAppearancePackage(legacy);
    const components = migrated.components as Record<string, {
      parts: Record<string, Record<string, unknown>>;
    }>;
    expect(components['branch-quick-switch']?.parts.item).toBeUndefined();
    expect(components['branch-quick-switch']?.parts.root?.states).toBeUndefined();
    expect(components['branch-quick-switch']?.parts.root?.contexts).toBeUndefined();
    expect(components['context-menu']?.parts.submenu).toBeUndefined();
    expect(components['context-menu']?.parts.root?.states).toBeUndefined();
    expect(appearancePackageValidator.validate(legacy, registry).valid).toBe(true);
  });

  it('keeps incompatible component contracts as grouped structured diagnostics', () => {
    const raw = validPackage();
    raw.components = {
      'toolbar-mode': {
        parts: {
          sessionMenu: { base: { opacity: { kind: 'number', value: 1 } } },
          input: { base: { opacity: { kind: 'number', value: 1 } } },
        },
      },
      'floating-mini-chat': {
        parts: {
          sessionMenu: { base: { opacity: { kind: 'number', value: 1 } } },
          inputBar: { base: { opacity: { kind: 'number', value: 1 } } },
          input: { base: { opacity: { kind: 'number', value: 1 } } },
        },
      },
    } as AppearancePackage['components'];

    const result = appearancePackageValidator.validate(raw, registry);
    expect(result.valid).toBe(false);
    expect(result.errors).toHaveLength(5);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({
        code: 'UNKNOWN_PART',
        path: 'components.toolbar-mode.parts.sessionMenu',
        context: expect.objectContaining({
          surfaceKind: 'component',
          surfaceId: 'toolbar-mode',
          partId: 'sessionMenu',
          allowedParts: expect.arrayContaining(['root', 'sessionSurface', 'controls']),
        }),
      }),
      expect.objectContaining({
        code: 'UNKNOWN_PART',
        path: 'components.floating-mini-chat.parts.inputBar',
        context: expect.objectContaining({
          surfaceId: 'floating-mini-chat',
          partId: 'inputBar',
          allowedParts: expect.arrayContaining(['root', 'panel', 'body']),
        }),
      }),
    ]));

    try {
      assertValidAppearancePackage(raw, registry);
      throw new Error('Expected appearance validation to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(AppearancePackageValidationError);
      const validationError = error as AppearancePackageValidationError;
      expect(validationError.groups).toHaveLength(2);
      expect(validationError.groups.map(group => group.surfaceId)).toEqual([
        'toolbar-mode',
        'floating-mini-chat',
      ]);
      expect(validationError.message).toContain('5 issues');
      expect(validationError.message).toContain('\ncomponent toolbar-mode:\n');
    }
  });

  it('rejects unknown fields instead of silently carrying CSS or selectors', () => {
    const raw = validPackage() as unknown as Record<string, unknown>;
    raw.css = '.btn { display: none }';
    const components = raw.components as Record<string, { parts: Record<string, Record<string, unknown>> }>;
    components['gallery-layout'].parts.root.selector = '.private-class';
    const result = appearancePackageValidator.validate(raw, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'UNKNOWN_FIELD', path: 'css' }),
      expect.objectContaining({ code: 'UNKNOWN_FIELD', path: 'components.gallery-layout.parts.root.selector' }),
    ]));
  });

  it('accepts structured advanced style values and rejects behavior-changing CSS escape hatches', () => {
    const structured = validPackage();
    structured.components = {
      'content-canvas': {
        parts: {
          root: {
            base: {
              display: 'grid',
              gridTemplateColumns: {
                kind: 'gridTracks',
                tracks: [{ kind: 'fr', value: 1 }, { kind: 'px', value: 20 }],
              },
              position: 'relative',
              aspectRatio: { kind: 'ratio', width: 4, height: 3 },
              objectFit: 'contain',
              filter: [{ kind: 'brightness', value: { kind: 'number', value: 1.1 } }],
            },
          },
        },
      },
    };
    expect(appearancePackageValidator.validate(structured, registry).valid).toBe(true);

    const escaped = validPackage() as unknown as Record<string, unknown>;
    escaped.components = {
      'content-canvas': {
        parts: {
          root: {
            base: {
              display: 'none',
              pointerEvents: 'none',
              gridTemplateColumns: '1fr 1fr',
            },
          },
        },
      },
    };
    const result = appearancePackageValidator.validate(escaped, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'INVALID_ENUM_VALUE', path: 'components.content-canvas.parts.root.base.display' }),
      expect.objectContaining({ code: 'UNSUPPORTED_STYLE_PROPERTY', path: 'components.content-canvas.parts.root.base.pointerEvents' }),
      expect.objectContaining({ code: 'INVALID_GRID_TEMPLATE', path: 'components.content-canvas.parts.root.base.gridTemplateColumns' }),
    ]));
  });

  it('accepts aligned structured background layers', () => {
    const pkg = validPackage();
    pkg.assets = {
      backdrop: { kind: 'image', mimeType: 'image/webp', source: { kind: 'package', path: 'assets/backdrop.webp' } },
      ornament: { kind: 'image', mimeType: 'image/png', source: { kind: 'package', path: 'assets/ornament.png' } },
    };
    pkg.components!['gallery-layout'].parts.root.base = {
      backgroundImages: [
        { kind: 'asset', assetId: 'ornament' },
        { kind: 'asset', assetId: 'backdrop' },
      ],
      backgroundSizes: [
        { kind: 'backgroundSize', width: { kind: 'px', value: 96 }, height: { kind: 'lengthKeyword', value: 'auto' } },
        'cover',
      ],
      backgroundPositions: [
        { kind: 'backgroundPosition', x: { kind: 'percent', value: 100 }, y: { kind: 'percent', value: 100 } },
        'center',
      ],
      backgroundRepeats: ['no-repeat', 'no-repeat'],
      backgroundBlendModes: ['normal', 'soft-light'],
    };

    const result = appearancePackageValidator.validate(pkg, registry);
    expect(result.valid).toBe(true);
    expect(result.errors).toEqual([]);
  });

  it('accepts a declared preview asset and rejects missing preview references', () => {
    const pkg = validPackage();
    pkg.assets = {
      preview: { kind: 'image', mimeType: 'image/webp', source: { kind: 'package', path: 'assets/preview.webp' } },
    };
    pkg.preview = { kind: 'asset', assetId: 'preview' };
    expect(appearancePackageValidator.validate(pkg, registry).valid).toBe(true);

    pkg.preview = { kind: 'asset', assetId: 'missing' };
    expect(appearancePackageValidator.validate(pkg, registry).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'UNKNOWN_ASSET_REFERENCE', path: 'preview.assetId' }),
    ]));
  });

  it('accepts host-managed video background media with an image poster', () => {
    const pkg = validPackage();
    pkg.requiredCapabilities = ['assets.v1', 'background-media.v1'];
    pkg.assets = {
      motion: { kind: 'video', mimeType: 'video/webm', source: { kind: 'package', path: 'assets/background.webm' } },
      poster: { kind: 'image', mimeType: 'image/webp', source: { kind: 'package', path: 'assets/poster.webp' } },
    };
    pkg.backgroundMedia = {
      kind: 'video',
      assetId: 'motion',
      posterAssetId: 'poster',
      fit: 'cover',
      position: 'center',
    };

    expect(appearancePackageValidator.validate(pkg, registry)).toMatchObject({ valid: true, errors: [] });
  });

  it('keeps video assets out of previews and CSS background declarations', () => {
    const pkg = validPackage();
    pkg.assets = {
      motion: { kind: 'video', mimeType: 'video/mp4', source: { kind: 'package', path: 'assets/background.mp4' } },
    };
    pkg.preview = { kind: 'asset', assetId: 'motion' };
    pkg.components!['gallery-layout'].parts.root.base = {
      backgroundImage: { kind: 'asset', assetId: 'motion' },
    };

    expect(appearancePackageValidator.validate(pkg, registry).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'PREVIEW_ASSET_NOT_IMAGE', path: 'preview.assetId' }),
      expect.objectContaining({ code: 'BACKGROUND_ASSET_NOT_IMAGE' }),
      expect.objectContaining({ code: 'UNUSED_VIDEO_ASSET', path: 'assets.motion' }),
    ]));
  });

  it('requires the background capability and rejects unrelated video assets', () => {
    const pkg = validPackage();
    pkg.assets = {
      motion: { kind: 'video', mimeType: 'video/webm', source: { kind: 'package', path: 'assets/background.webm' } },
      unused: { kind: 'video', mimeType: 'video/mp4', source: { kind: 'package', path: 'assets/unused.mp4' } },
      poster: { kind: 'image', mimeType: 'image/webp', source: { kind: 'package', path: 'assets/poster.webp' } },
    };
    pkg.backgroundMedia = { kind: 'video', assetId: 'motion', posterAssetId: 'poster' };

    expect(appearancePackageValidator.validate(pkg, registry).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'MISSING_BACKGROUND_MEDIA_CAPABILITY', path: 'requiredCapabilities' }),
      expect.objectContaining({ code: 'UNUSED_VIDEO_ASSET', path: 'assets.unused' }),
    ]));
  });

  it('rejects conflicting or misaligned background layer declarations', () => {
    const pkg = validPackage() as unknown as Record<string, unknown>;
    const components = pkg.components as Record<string, { parts: Record<string, { base: Record<string, unknown> }> }>;
    components['gallery-layout'].parts.root.base = {
      backgroundImage: { kind: 'asset', assetId: 'backdrop' },
      backgroundImages: [
        { kind: 'asset', assetId: 'backdrop' },
        { kind: 'asset', assetId: 'ornament' },
      ],
      backgroundSizes: ['cover'],
      backgroundBlendModes: ['normal', 'unsupported'],
    };

    const result = appearancePackageValidator.validate(pkg, registry);
    expect(result.errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'CONFLICTING_BACKGROUND_FIELDS' }),
      expect.objectContaining({ code: 'BACKGROUND_LAYER_COUNT_MISMATCH' }),
      expect.objectContaining({ code: 'INVALID_BACKGROUND_BLEND_MODES' }),
      expect.objectContaining({ code: 'UNKNOWN_ASSET_REFERENCE' }),
    ]));
  });

  it('validates composable material definitions and rejects the retired singular field', () => {
    const pkg = validPackage() as unknown as Record<string, unknown>;
    pkg.materials = {
      surface: {
        visualRole: 'control',
        style: { backgroundColor: { kind: 'hex', value: '#202020' } },
      },
    };
    const components = pkg.components as Record<string, { parts: Record<string, Record<string, unknown>> }>;
    components['gallery-layout'].parts.root.materials = ['surface'];
    expect(appearancePackageValidator.validate(pkg, registry).valid).toBe(true);

    delete components['gallery-layout'].parts.root.materials;
    components['gallery-layout'].parts.root.material = 'surface';
    expect(appearancePackageValidator.validate(pkg, registry).errors).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'UNKNOWN_FIELD', path: 'components.gallery-layout.parts.root.material' }),
    ]));
  });

  it('warns when continuous regions are framed without an explicit intent', () => {
    const pkg = validPackage();
    pkg.components = {
      'content-canvas': {
        parts: {
          main: { base: { borderRadius: { kind: 'px', value: 8 } } },
        },
      },
    };

    const warned = appearancePackageValidator.validate(pkg, registry);
    expect(warned.valid).toBe(true);
    expect(warned.warnings).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'CONTINUOUS_SURFACE_FRAMED' }),
    ]));

    pkg.components['content-canvas'].parts.main.decorationIntent = 'framed';
    const intentional = appearancePackageValidator.validate(pkg, registry);
    expect(intentional.warnings).not.toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'CONTINUOUS_SURFACE_FRAMED' }),
    ]));

    pkg.components['content-canvas'].parts.main = {
      states: { active: { boxShadow: [{ kind: 'ref', path: 'globals.shadows.surface' }] } },
    };
    pkg.globals = {
      shadows: {
        surface: {
          kind: 'shadow',
          x: { kind: 'zero' },
          y: { kind: 'px', value: 4 },
          blur: { kind: 'px', value: 12 },
          color: { kind: 'rgb', r: 0, g: 0, b: 0, a: 0.2 },
        },
      },
    };
    expect(appearancePackageValidator.validate(pkg, registry).warnings).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'CONTINUOUS_SURFACE_FRAMED', path: 'components.content-canvas.parts.main.states.active' }),
    ]));
  });

  it('warns when override is used as a package-wide default', () => {
    const pkg = validPackage();
    pkg.components = {
      'content-canvas': {
        parts: {
          root: { cascade: 'override', base: { color: { kind: 'hex', value: '#ffffff' } } },
          main: { cascade: 'override', base: { color: { kind: 'hex', value: '#ffffff' } } },
          editor: { cascade: 'override', base: { color: { kind: 'hex', value: '#ffffff' } } },
          empty: { cascade: 'normal', base: { color: { kind: 'hex', value: '#ffffff' } } },
        },
      },
    };

    const result = appearancePackageValidator.validate(pkg, registry);
    expect(result.valid).toBe(true);
    expect(result.warnings).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'EXCESSIVE_OVERRIDE_USAGE' }),
    ]));
  });
});
