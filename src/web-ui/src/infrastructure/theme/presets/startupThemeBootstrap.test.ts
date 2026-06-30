import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { builtinThemes, DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID } from './index';
import { createStartupThemeBootstrapManifest } from './startupThemeBootstrap';
import { createThemePromptSnapshotManifest } from './themePromptSnapshots';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const generatedManifestPath = path.resolve(
  __dirname,
  '../../../../../apps/desktop/src/generated/startup_theme_bootstrap.json',
);
const generatedThemePromptSnapshotPath = path.resolve(
  __dirname,
  '../../../../../crates/assembly/core/src/agentic/tools/implementations/generated/theme_prompt_snapshots.json',
);

describe('startup theme bootstrap manifest', () => {
  it('projects only the desktop first-paint theme fields from TS builtin themes', () => {
    const manifest = createStartupThemeBootstrapManifest(builtinThemes);

    expect(manifest).toMatchObject({
      version: 1,
      defaultLightThemeId: DEFAULT_LIGHT_THEME_ID,
      defaultDarkThemeId: DEFAULT_DARK_THEME_ID,
    });
    expect(manifest.themes).toHaveLength(builtinThemes.length);
    expect(new Set(manifest.themes.map(theme => theme.id)).size).toBe(builtinThemes.length);
    expect(manifest.themes.map(theme => theme.id)).toEqual(
      expect.arrayContaining([DEFAULT_LIGHT_THEME_ID, DEFAULT_DARK_THEME_ID])
    );

    const light = manifest.themes.find(theme => theme.id === 'bitfun-light');
    const sourceLight = builtinThemes.find(theme => theme.id === 'bitfun-light');

    expect(light).toEqual({
      id: sourceLight?.id,
      bgPrimary: sourceLight?.colors.background.primary,
      bgSecondary: sourceLight?.colors.background.secondary,
      bgScene: sourceLight?.colors.background.scene,
      isLight: true,
      textPrimary: sourceLight?.colors.text.primary,
      textMuted: sourceLight?.colors.text.muted,
      accentColor: sourceLight?.colors.accent[500],
    });
  });

  it('keeps the committed desktop manifest synchronized with TS builtin themes', () => {
    const generated = JSON.parse(fs.readFileSync(generatedManifestPath, 'utf8'));

    expect(generated).toEqual(createStartupThemeBootstrapManifest(builtinThemes));
  });
});

describe('theme prompt snapshot manifest', () => {
  it('projects generative UI prompt fields from TS builtin themes', () => {
    const manifest = createThemePromptSnapshotManifest(builtinThemes);

    expect(manifest).toMatchObject({
      version: 1,
      defaultLightThemeId: DEFAULT_LIGHT_THEME_ID,
      defaultDarkThemeId: DEFAULT_DARK_THEME_ID,
    });
    expect(manifest.themes).toHaveLength(builtinThemes.length);

    const dark = manifest.themes.find(theme => theme.id === 'bitfun-dark');
    const sourceDark = builtinThemes.find(theme => theme.id === 'bitfun-dark');

    expect(dark).toEqual({
      id: sourceDark?.id,
      themeType: sourceDark?.type,
      bgPrimary: sourceDark?.colors.background.primary,
      bgSecondary: sourceDark?.colors.background.secondary,
      bgScene: sourceDark?.colors.background.scene,
      textPrimary: sourceDark?.colors.text.primary,
      textMuted: sourceDark?.colors.text.muted,
      accent500: sourceDark?.colors.accent[500],
      accent600: sourceDark?.colors.accent[600],
      borderBase: sourceDark?.colors.border.base,
      elementBase: sourceDark?.colors.element.base,
      radiusBase: sourceDark?.effects.radius.base,
      spacing4: sourceDark?.effects.spacing[4],
      shadowBase: sourceDark?.effects.shadow.base,
      styleNotes: sourceDark?.description,
    });
  });

  it('keeps the committed generative UI manifest synchronized with TS builtin themes', () => {
    const generated = JSON.parse(fs.readFileSync(generatedThemePromptSnapshotPath, 'utf8'));

    expect(generated).toEqual(createThemePromptSnapshotManifest(builtinThemes));
  });
});
