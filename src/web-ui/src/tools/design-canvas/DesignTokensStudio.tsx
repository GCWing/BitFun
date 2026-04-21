import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Check, Palette } from 'lucide-react';
import { Button } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import { designTokensAPI } from './designTokensAPI';
import { useDesignTokensStore, type DesignTokenProposal } from './store/designTokensStore';
import { canonicalScopeKey, pickString } from './tokensSchema';
import './DesignTokensStudio.scss';

interface Props {
  artifactId?: string;
  scopePath?: string;
}

const pick = pickString;

function entries(obj: unknown): Array<[string, string]> {
  if (!obj || typeof obj !== 'object') return [];
  return Object.entries(obj as Record<string, unknown>)
    .filter(([, v]) => typeof v === 'string' || typeof v === 'number')
    .map(([k, v]) => [k, String(v)] as [string, string]);
}

export const DesignTokensStudio: React.FC<Props> = ({ artifactId, scopePath }) => {
  const { t } = useTranslation('flow-chat');
  const { workspacePath } = useCurrentWorkspace();
  // Canonical scope key: prefer an explicit path from the card,
  // otherwise derive workspace/artifact key so we never silently fall
  // through to `Object.values(docs)[0]`.
  const scopeKey = useMemo(
    () => canonicalScopeKey({ explicitPath: scopePath, workspacePath, artifactId }),
    [scopePath, workspacePath, artifactId]
  );
  const document = useDesignTokensStore((s) => s.byScope[scopeKey]);
  const proposals = document?.proposals || [];
  const [selectedId, setSelectedId] = useState<string>(document?.committed_id || proposals[0]?.id || '');
  const [switchOn, setSwitchOn] = useState(true);

  const selected = useMemo<DesignTokenProposal | undefined>(
    () => proposals.find((p) => p.id === selectedId) || proposals[0],
    [proposals, selectedId]
  );

  const commit = async () => {
    if (!selected) return;
    try {
      await designTokensAPI.commit(selected.id, artifactId, workspacePath);
      notificationService.success(t('designCanvas.studio.commitOk'));
    } catch (err: any) {
      notificationService.error(
        t('designCanvas.studio.commitFail', { message: String(err?.message || err) })
      );
    }
  };

  if (!selected) {
    return (
      <div className="design-tokens-studio design-tokens-studio--empty">
        <Palette size={28} />
        <div>{t('designCanvas.studio.empty')}</div>
      </div>
    );
  }

  const colors = (selected.colors || {}) as Record<string, string>;
  const typography = (selected.typography as Record<string, any>) || {};
  const scale = (typography.scale as Record<string, any>) || {};
  const weight = (typography.weight as Record<string, any>) || {};
  const radius = (selected.radius as Record<string, any>) || {};
  const shadow = (selected.shadow as Record<string, any>) || {};
  const spacing = (selected.spacing as Record<string, any>) || {};
  const motion = (selected.motion as Record<string, any>) || {};

  const colorEntries = entries(colors);
  const radiusEntries = entries(radius);
  const shadowEntries = entries(shadow);
  const spacingEntries = entries(spacing);
  const scaleEntries = entries(scale);

  const family = pick(typography, 'fontFamily', 'family') || 'Inter, system-ui, sans-serif';
  const monoFamily = pick(typography, 'fontFamilyMono', 'familyMono') || 'ui-monospace, SFMono-Regular, monospace';

  const resolve = (...keys: string[]) => pick(colors, ...keys);

  // Parse a color string's luminance (rough, RGB only) so we can auto-detect
  // whether the committed palette is light-on-dark or dark-on-light, and
  // synthesize a matching companion surface when we need to preview on the
  // opposite lightness.
  const parseLuminance = (input?: string): number | undefined => {
    if (!input) return undefined;
    const v = input.trim();
    const hex = v.startsWith('#') ? v.slice(1) : '';
    const fromHex = (h: string) => {
      const norm =
        h.length === 3 || h.length === 4
          ? h.slice(0, 3).split('').map((c) => c + c).join('')
          : h.slice(0, 6);
      if (!/^[0-9a-fA-F]{6}$/.test(norm)) return undefined;
      const r = parseInt(norm.slice(0, 2), 16) / 255;
      const g = parseInt(norm.slice(2, 4), 16) / 255;
      const b = parseInt(norm.slice(4, 6), 16) / 255;
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    if (hex) return fromHex(hex);
    const rgb = v.match(/rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i);
    if (rgb) {
      const r = parseInt(rgb[1], 10) / 255;
      const g = parseInt(rgb[2], 10) / 255;
      const b = parseInt(rgb[3], 10) / 255;
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    }
    return undefined;
  };

  const realBackground = resolve('background', 'bg');
  const realSurface = resolve('surface', 'surfaceElevated') || realBackground;
  const realText = resolve('text');
  const realTextMuted = resolve('textMuted', 'textSecondary');
  const realBorder = resolve('border');
  const paletteLuminance = parseLuminance(realBackground) ?? 0.5;
  const paletteIsLight = paletteLuminance > 0.5;

  const sharedVars = {
    '--dt-primary': resolve('primary', 'accent', 'brand') || '#161616',
    '--dt-primary-hover': resolve('primaryHover', 'accentHover') || resolve('primary', 'accent') || '#161616',
    '--dt-accent': resolve('accent', 'primary', 'brand') || '#161616',
    '--dt-success': resolve('success') || '#2F7A4D',
    '--dt-danger': resolve('danger', 'error') || '#B34343',
    '--dt-font': family,
    '--dt-font-mono': monoFamily,
    '--dt-radius-sm': pick(radius, 'sm', 'xs') || '4px',
    '--dt-radius-md': pick(radius, 'md', 'base') || '8px',
    '--dt-radius-lg': pick(radius, 'lg') || '14px',
    '--dt-radius-full': pick(radius, 'full', 'pill') || '999px',
    '--dt-shadow-sm': pick(shadow, 'sm') || '0 1px 2px rgba(0,0,0,0.06)',
    '--dt-shadow-md': pick(shadow, 'md', 'base') || '0 2px 8px rgba(0,0,0,0.08)',
    '--dt-shadow-lg': pick(shadow, 'lg') || '0 10px 28px rgba(0,0,0,0.14)',
    '--dt-duration': pick(motion.duration as any, 'normal', 'base') || '200ms',
    '--dt-ease': (motion.ease as string) || 'cubic-bezier(0.4, 0, 0.2, 1)',
    '--dt-space-sm': pick(spacing, 'sm', 'xs') || '8px',
    '--dt-space-md': pick(spacing, 'md', 'base') || '16px',
    '--dt-space-lg': pick(spacing, 'lg') || '24px',
  } as React.CSSProperties;

  // Primary surface: render the committed palette *exactly as authored*.
  const nativeVars = {
    ...sharedVars,
    '--dt-bg': realBackground || (paletteIsLight ? '#F7F7F5' : '#0C0D10'),
    '--dt-surface': realSurface || realBackground || (paletteIsLight ? '#FFFFFF' : '#14161A'),
    '--dt-surface-elevated': realSurface || realBackground || (paletteIsLight ? '#FFFFFF' : '#1A1C21'),
    '--dt-text': realText || (paletteIsLight ? '#0C0D10' : '#F5F7FB'),
    '--dt-text-muted': realTextMuted || (paletteIsLight ? 'rgba(12,13,16,0.55)' : 'rgba(245,247,251,0.64)'),
    '--dt-border': realBorder || (paletteIsLight ? 'rgba(12,13,16,0.09)' : 'rgba(255,255,255,0.09)'),
  } as React.CSSProperties;

  // Companion surface: swap to the opposite lightness using neutral defaults
  // so reviewers can stress-test readability without pretending the palette
  // defines two separate systems.
  const inverseVars = {
    ...sharedVars,
    '--dt-bg': paletteIsLight ? '#0C0D10' : '#F7F7F5',
    '--dt-surface': paletteIsLight ? '#14161A' : '#FFFFFF',
    '--dt-surface-elevated': paletteIsLight ? '#1A1C21' : '#FFFFFF',
    '--dt-text': paletteIsLight ? '#F5F7FB' : '#0C0D10',
    '--dt-text-muted': paletteIsLight ? 'rgba(245,247,251,0.64)' : 'rgba(12,13,16,0.55)',
    '--dt-border': paletteIsLight ? 'rgba(255,255,255,0.09)' : 'rgba(12,13,16,0.09)',
  } as React.CSSProperties;

  const nativeLabel = paletteIsLight
    ? t('designCanvas.studio.nativeLight')
    : t('designCanvas.studio.nativeDark');
  const inverseLabel = paletteIsLight
    ? t('designCanvas.studio.stressDark')
    : t('designCanvas.studio.stressLight');

  return (
    <div className="design-tokens-studio">
      <aside className="design-tokens-studio__sidebar">
        <div className="design-tokens-studio__sidebar-title">{t('designCanvas.studio.sidebarTitle')}</div>
        {proposals.map((proposal) => {
          const isCommitted = document?.committed_id === proposal.id;
          const classes = [
            'design-tokens-studio__proposal',
            proposal.id === selected.id ? 'is-active' : '',
            isCommitted ? 'is-committed' : '',
          ].filter(Boolean).join(' ');
          return (
            <button
              key={proposal.id}
              type="button"
              className={classes}
              onClick={() => setSelectedId(proposal.id)}
            >
              <div className="design-tokens-studio__proposal-swatches">
                {Object.values((proposal.colors as Record<string, string>) || {}).slice(0, 4).map((c, i) => (
                  <span key={i} style={{ background: String(c) }} />
                ))}
              </div>
              <div className="design-tokens-studio__proposal-meta">
                <div className="design-tokens-studio__proposal-title">{proposal.name}</div>
                <div className="design-tokens-studio__proposal-mood">{proposal.mood}</div>
              </div>
              {isCommitted && (
                <span className="design-tokens-studio__proposal-badge" title={t('designCanvas.studio.adoptedBadge')}>
                  <Check size={10} />
                </span>
              )}
            </button>
          );
        })}
      </aside>

      <main className="design-tokens-studio__main">
        <header className="design-tokens-studio__header">
          <div className="design-tokens-studio__heading">
            <span className="design-tokens-studio__eyebrow">{t('designCanvas.studio.eyebrow')}</span>
            <h2>{selected.name}</h2>
            <p>{selected.mood}</p>
          </div>
          <Button type="button" size="small" variant="primary" className="design-tokens-studio__commit" onClick={commit}>
            <Check size={14} />
            {document?.committed_id === selected.id ? t('designCanvas.studio.recommit') : t('designCanvas.studio.adopt')}
          </Button>
        </header>

        <section className="design-tokens-studio__section">
          <div className="design-tokens-studio__section-head">
            <h3>{t('designCanvas.studio.palette')}</h3>
            <span>{t('designCanvas.studio.tokenCount', { count: colorEntries.length })}</span>
          </div>
          <div className="design-tokens-studio__swatches">
            {colorEntries.map(([name, value]) => (
              <div key={name} className="design-tokens-studio__swatch">
                <div className="design-tokens-studio__swatch-color" style={{ background: String(value) }} />
                <div className="design-tokens-studio__swatch-meta">
                  <code>{name}</code>
                  <span>{String(value)}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        <div className="design-tokens-studio__grid">
          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.typography')}</h3>
              <span style={{ fontFamily: family }}>{family.split(',')[0].replace(/['"]/g, '')}</span>
            </div>
            <div className="design-tokens-studio__type" style={{ fontFamily: family }}>
              {scaleEntries.length === 0 && (
                <div className="design-tokens-studio__hint">{t('designCanvas.studio.noTypeScale')}</div>
              )}
              {scaleEntries.map(([name, value]) => (
                <div key={name} className="design-tokens-studio__type-row">
                  <div className="design-tokens-studio__type-sample" style={{ fontSize: value }}>
                    {t('designCanvas.studio.typeSample')}
                  </div>
                  <div className="design-tokens-studio__type-meta">
                    <code>{name}</code>
                    <span>{value}</span>
                  </div>
                </div>
              ))}
              {entries(weight).length > 0 && (
                <div className="design-tokens-studio__weights">
                  {entries(weight).map(([name, value]) => (
                    <span key={name} style={{ fontWeight: value as any }}>{name} · {value}</span>
                  ))}
                </div>
              )}
            </div>
          </section>

          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.radius')}</h3>
              <span>{t('designCanvas.studio.tokenCount', { count: radiusEntries.length })}</span>
            </div>
            <div className="design-tokens-studio__tiles">
              {radiusEntries.map(([name, value]) => (
                <div key={name} className="design-tokens-studio__tile">
                  <div
                    className="design-tokens-studio__tile-swatch"
                    style={{ borderRadius: value as string }}
                  />
                  <code>{name}</code>
                  <span>{value}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.shadow')}</h3>
              <span>{t('designCanvas.studio.tokenCount', { count: shadowEntries.length })}</span>
            </div>
            <div className="design-tokens-studio__tiles">
              {shadowEntries.map(([name, value]) => (
                <div key={name} className="design-tokens-studio__tile">
                  <div
                    className="design-tokens-studio__tile-swatch design-tokens-studio__tile-swatch--shadow"
                    style={{ boxShadow: value as string }}
                  />
                  <code>{name}</code>
                  <span className="design-tokens-studio__tile-mono" title={String(value)}>{String(value)}</span>
                </div>
              ))}
            </div>
          </section>

          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.spacing')}</h3>
              <span>{t('designCanvas.studio.tokenCount', { count: spacingEntries.length })}</span>
            </div>
            <div className="design-tokens-studio__spacing">
              {spacingEntries.length === 0 && (
                <div className="design-tokens-studio__hint">{t('designCanvas.studio.noSpacingScale')}</div>
              )}
              {spacingEntries.map(([name, value]) => {
                const num = parseInt(String(value), 10);
                const w = isNaN(num) ? 8 : Math.min(64, Math.max(2, num));
                return (
                  <div key={name} className="design-tokens-studio__spacing-row">
                    <span className="design-tokens-studio__spacing-bar" style={{ width: w }} />
                    <code>{name}</code>
                    <span>{value}</span>
                  </div>
                );
              })}
            </div>
          </section>
        </div>

        <section className="design-tokens-studio__section">
          <div className="design-tokens-studio__section-head">
            <h3>{t('designCanvas.studio.componentPreview')}</h3>
            <span>{t('designCanvas.studio.previewPairCaption')}</span>
          </div>
          <div className="design-tokens-studio__preview-pair">
            {([
              { mode: 'native' as const, label: nativeLabel, vars: nativeVars },
              { mode: 'inverse' as const, label: inverseLabel, vars: inverseVars },
            ]).map((surface) => (
              <div key={surface.mode} className={`design-tokens-studio__preview design-tokens-studio__preview--${surface.mode}`} style={surface.vars}>
                <div className="design-tokens-studio__preview-label">{surface.label}</div>
                <div className="preview-grid">
                  <div className="preview-card preview-card--primary">
                    <div className="preview-chip">{selected.mood}</div>
                    <h4>{selected.name}</h4>
                    <p>
                      {surface.mode === 'native'
                        ? t('designCanvas.studio.previewNativeDesc')
                        : t('designCanvas.studio.previewStressDesc')}
                    </p>

                    <div className="preview-row">
                      <button className="preview-btn preview-btn--primary" type="button">
                        {t('designCanvas.studio.btnPrimary')}
                      </button>
                      <button className="preview-btn preview-btn--secondary" type="button">
                        {t('designCanvas.studio.btnSecondary')}
                      </button>
                      <button className="preview-btn preview-btn--ghost" type="button">
                        {t('designCanvas.studio.btnGhost')}
                      </button>
                    </div>

                    <div className="preview-row preview-row--split">
                      <div className="preview-field">
                        <label>{t('designCanvas.studio.fieldEmail')}</label>
                        <div className="preview-input">
                          <input type="email" placeholder="name@studio.com" defaultValue="" />
                        </div>
                      </div>
                      <div className="preview-field preview-field--inline">
                        <label>{t('designCanvas.studio.fieldNotify')}</label>
                        <button
                          type="button"
                          className={`preview-switch${switchOn ? ' is-on' : ''}`}
                          onClick={() => setSwitchOn((v) => !v)}
                          aria-pressed={switchOn}
                        >
                          <span className="preview-switch__thumb" />
                        </button>
                      </div>
                    </div>

                    <div className="preview-chips">
                      <span className="preview-pill">{t('designCanvas.studio.pillAccent')}</span>
                      <span className="preview-pill preview-pill--muted">{t('designCanvas.studio.pillMuted')}</span>
                      <span className="preview-pill preview-pill--outline">{t('designCanvas.studio.pillOutline')}</span>
                    </div>
                  </div>

                  <div className="preview-card preview-card--stats">
                    <div className="preview-stat">
                      <span className="preview-stat__label">{t('designCanvas.studio.statRadius')}</span>
                      <strong>{pick(radius, 'md', 'base') || '—'}</strong>
                    </div>
                    <div className="preview-stat">
                      <span className="preview-stat__label">{t('designCanvas.studio.statShadow')}</span>
                      <div className="preview-stat__shadow" />
                    </div>
                    <div className="preview-stat">
                      <span className="preview-stat__label">{t('designCanvas.studio.statMotion')}</span>
                      <strong>{pick(motion.duration as any, 'normal', 'base') || '—'}</strong>
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </section>
      </main>
    </div>
  );
};

export default DesignTokensStudio;
