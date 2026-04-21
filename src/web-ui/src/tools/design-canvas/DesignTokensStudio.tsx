import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Apple, ArrowDown, Check, Monitor, Orbit, Palette, Terminal } from 'lucide-react';
import { Button, Input } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import { designTokensAPI } from './designTokensAPI';
import { useDesignTokensStore, type DesignTokenProposal } from './store/designTokensStore';
import { canonicalScopeKey, pickString, resolveTokens } from './tokensSchema';
import './DesignTokensStudio.scss';

interface Props {
  artifactId?: string;
  scopePath?: string;
}

const pick = pickString;

/** Token values may be bare numbers (e.g. "12") — ensure they carry a CSS unit. */
function withUnit(val: string | undefined, fallback: string): string {
  if (!val) return fallback;
  return /^\d+(\.\d+)?$/.test(val.trim()) ? `${val}px` : val;
}

function withMs(val: string | undefined, fallback: string): string {
  if (!val) return fallback;
  if (/^\d+(\.\d+)?$/.test(val.trim())) return `${val}ms`;
  return val;
}

function entries(obj: unknown): Array<[string, string]> {
  if (!obj || typeof obj !== 'object') return [];
  return Object.entries(obj as Record<string, unknown>)
    .filter(([, v]) => typeof v === 'string' || typeof v === 'number')
    .map(([k, v]) => [k, String(v)] as [string, string]);
}

function cloneProposal(proposal: DesignTokenProposal): DesignTokenProposal {
  if (typeof structuredClone === 'function') {
    return structuredClone(proposal);
  }
  return JSON.parse(JSON.stringify(proposal)) as DesignTokenProposal;
}

function updateRecordValue(
  proposal: DesignTokenProposal,
  section: keyof DesignTokenProposal,
  key: string,
  value: string,
  nestedKey?: string
): DesignTokenProposal {
  const next = cloneProposal(proposal);
  const sectionValue = ((next[section] as Record<string, unknown>) || {}) as Record<string, unknown>;
  if (nestedKey) {
    const nested = { ...((sectionValue[key] as Record<string, unknown>) || {}) };
    nested[nestedKey] = value;
    sectionValue[key] = nested;
  } else {
    sectionValue[key] = value;
  }
  (next as any)[section] = sectionValue;
  return next;
}

function isHexColor(value: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(value.trim());
}

interface EditableValueProps {
  value: string;
  title?: string;
  className?: string;
  onChange: (value: string) => void;
}

const EditableValue: React.FC<EditableValueProps> = ({ value, title, className = '', onChange }) => {
  const [editing, setEditing] = useState(false);
  const [localValue, setLocalValue] = useState(value);

  useEffect(() => {
    if (!editing) {
      setLocalValue(value);
    }
  }, [editing, value]);

  const commitLocal = () => {
    onChange(localValue);
    setEditing(false);
  };

  if (editing) {
    return (
      <Input
        className={`design-tokens-studio__inline-input ${className}`}
        value={localValue}
        autoFocus
        onChange={(event) => setLocalValue(event.currentTarget.value)}
        onBlur={commitLocal}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commitLocal();
          } else if (event.key === 'Escape') {
            setLocalValue(value);
            setEditing(false);
          }
        }}
      />
    );
  }

  return (
    <button
      type="button"
      className={`design-tokens-studio__editable-value ${className}`}
      title={title || value}
      onClick={() => setEditing(true)}
    >
      {value}
    </button>
  );
};

interface ColorSwatchButtonProps {
  name: string;
  value: string;
  proposal: DesignTokenProposal;
  onChange: (proposal: DesignTokenProposal) => void;
}

const ColorSwatchButton: React.FC<ColorSwatchButtonProps> = ({ name, value, proposal, onChange }) => (
  <label className="design-tokens-studio__swatch-color design-tokens-studio__swatch-color--editable" style={{ background: String(value) }}>
    {isHexColor(value) && (
      <input
        type="color"
        value={value}
        aria-label={name}
        onChange={(event) => onChange(updateRecordValue(proposal, 'colors', name, event.currentTarget.value))}
      />
    )}
  </label>
);

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
  const [activePreviewMode, setActivePreviewMode] = useState<'native' | 'inverse'>('native');

  const selected = useMemo<DesignTokenProposal | undefined>(
    () => proposals.find((p) => p.id === selectedId) || proposals[0],
    [proposals, selectedId]
  );
  const [draft, setDraft] = useState<DesignTokenProposal | null>(selected ? cloneProposal(selected) : null);
  const [savingDraft, setSavingDraft] = useState(false);
  const [draftError, setDraftError] = useState<string | null>(null);
  const draftProposal = draft || selected;
  const isDirty = Boolean(selected && draft && JSON.stringify(selected) !== JSON.stringify(draft));

  useEffect(() => {
    setDraft(selected ? cloneProposal(selected) : null);
    setDraftError(null);
  }, [selected?.id, selected]);

  const commit = async () => {
    if (!draftProposal) return;
    try {
      await designTokensAPI.commit(draftProposal.id, artifactId, workspacePath);
      notificationService.success(t('designCanvas.studio.commitOk'));
    } catch (err: any) {
      notificationService.error(
        t('designCanvas.studio.commitFail', { message: String(err?.message || err) })
      );
    }
  };

  const saveDraft = async () => {
    if (!draft || !selected || !isDirty) return;
    setSavingDraft(true);
    setDraftError(null);
    try {
      await designTokensAPI.updateProposal(draft, artifactId, workspacePath);
      notificationService.success(t('designCanvas.studio.saveOk'));
    } catch (err: any) {
      const message = String(err?.message || err);
      setDraftError(message);
      notificationService.error(t('designCanvas.studio.saveFail', { message }));
    } finally {
      setSavingDraft(false);
    }
  };

  const resetDraft = () => {
    setDraft(selected ? cloneProposal(selected) : null);
    setDraftError(null);
  };

  if (!draftProposal) {
    return (
      <div className="design-tokens-studio design-tokens-studio--empty">
        <Palette size={28} />
        <div>{t('designCanvas.studio.empty')}</div>
      </div>
    );
  }

  const colors = (draftProposal.colors || {}) as Record<string, string>;
  const typography = (draftProposal.typography as Record<string, any>) || {};
  const scale = (typography.scale as Record<string, any>) || {};
  const weight = (typography.weight as Record<string, any>) || {};
  const radius = (draftProposal.radius as Record<string, any>) || {};
  const shadow = (draftProposal.shadow as Record<string, any>) || {};
  const spacing = (draftProposal.spacing as Record<string, any>) || {};
  const motion = (draftProposal.motion as Record<string, any>) || {};

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

  /** Canonical type ramp + xs/xl spacing — same as Proposal card / tokens CSS pipeline. */
  const resolvedCanonical = useMemo(() => resolveTokens(draftProposal), [draftProposal]);

  /** When a section has no authored tokens, show these schema defaults so the block is not visually empty. */
  const defaultTypeRamp = useMemo(
    () =>
      [
        { role: 'display' as const, size: resolvedCanonical['--dt-font-display'] },
        { role: 'headline' as const, size: resolvedCanonical['--dt-font-headline'] },
        { role: 'title' as const, size: resolvedCanonical['--dt-font-title'] },
        { role: 'body' as const, size: resolvedCanonical['--dt-font-body'] },
        { role: 'caption' as const, size: resolvedCanonical['--dt-font-caption'] },
      ],
    [resolvedCanonical]
  );

  const defaultRadiusTiles = useMemo(
    () =>
      [
        ['sm', resolvedCanonical['--dt-radius-sm']] as const,
        ['md', resolvedCanonical['--dt-radius-md']] as const,
        ['lg', resolvedCanonical['--dt-radius-lg']] as const,
        ['full', resolvedCanonical['--dt-radius-full']] as const,
      ],
    [resolvedCanonical]
  );

  const defaultShadowTiles = useMemo(
    () =>
      [
        ['sm', resolvedCanonical['--dt-shadow-sm']] as const,
        ['md', resolvedCanonical['--dt-shadow-md']] as const,
        ['lg', resolvedCanonical['--dt-shadow-lg']] as const,
      ],
    [resolvedCanonical]
  );

  const defaultSpacingRows = useMemo(
    () =>
      [
        ['xs', resolvedCanonical['--dt-space-xs']] as const,
        ['sm', resolvedCanonical['--dt-space-sm']] as const,
        ['md', resolvedCanonical['--dt-space-md']] as const,
        ['lg', resolvedCanonical['--dt-space-lg']] as const,
        ['xl', resolvedCanonical['--dt-space-xl']] as const,
      ],
    [resolvedCanonical]
  );

  const sharedVars = {
    '--dt-primary': resolve('primary', 'accent', 'brand') || '#161616',
    '--dt-primary-hover': resolve('primaryHover', 'accentHover') || resolve('primary', 'accent') || '#161616',
    '--dt-accent': resolve('accent', 'primary', 'brand') || '#161616',
    '--dt-success': resolve('success') || '#2F7A4D',
    '--dt-warning': resolve('warning') || resolve('accent', 'primary') || '#B7791F',
    '--dt-danger': resolve('danger', 'error') || '#B34343',
    '--dt-font': family,
    '--dt-font-mono': monoFamily,
    '--dt-font-display': resolvedCanonical['--dt-font-display'],
    '--dt-font-headline': resolvedCanonical['--dt-font-headline'],
    '--dt-font-title': resolvedCanonical['--dt-font-title'],
    '--dt-font-body': resolvedCanonical['--dt-font-body'],
    '--dt-font-caption': resolvedCanonical['--dt-font-caption'],
    '--dt-radius-sm': withUnit(pick(radius, 'sm', 'xs'), '4px'),
    '--dt-radius-md': withUnit(pick(radius, 'md', 'base'), '8px'),
    '--dt-radius-lg': withUnit(pick(radius, 'lg'), '14px'),
    '--dt-radius-full': withUnit(pick(radius, 'full', 'pill'), '999px'),
    '--dt-shadow-sm': pick(shadow, 'sm') || '0 1px 2px rgba(0,0,0,0.06)',
    '--dt-shadow-md': pick(shadow, 'md', 'base') || '0 2px 8px rgba(0,0,0,0.08)',
    '--dt-shadow-lg': pick(shadow, 'lg') || '0 10px 28px rgba(0,0,0,0.14)',
    '--dt-duration': withMs(pick(motion.duration as any, 'normal', 'base'), '200ms'),
    '--dt-ease': (motion.ease as string) || 'cubic-bezier(0.4, 0, 0.2, 1)',
    '--dt-space-xs': resolvedCanonical['--dt-space-xs'],
    '--dt-space-sm': pick(spacing, 'sm', 'xs') || '8px',
    '--dt-space-md': pick(spacing, 'md', 'base') || '16px',
    '--dt-space-lg': pick(spacing, 'lg') || '24px',
    '--dt-space-xl': resolvedCanonical['--dt-space-xl'],
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
  const previewSurfaces = [
    { mode: 'native' as const, label: nativeLabel, vars: nativeVars },
    { mode: 'inverse' as const, label: inverseLabel, vars: inverseVars },
  ];
  const activePreviewSurface = previewSurfaces.find((surface) => surface.mode === activePreviewMode) || previewSurfaces[0];

  return (
    <div className="design-tokens-studio">
      <main className="design-tokens-studio__main">
        <header className="design-tokens-studio__header">
          <div className="design-tokens-studio__heading">
            <span className="design-tokens-studio__eyebrow">{t('designCanvas.studio.eyebrow')}</span>
            <h2>
              <EditableValue
                value={draftProposal.name}
                onChange={(value) =>
                  setDraft((current) => current ? { ...current, name: value } : current)
                }
              />
            </h2>
            <p>
              <EditableValue
                value={draftProposal.mood}
                onChange={(value) =>
                  setDraft((current) => current ? { ...current, mood: value } : current)
                }
              />
            </p>
          </div>
          <div className="design-tokens-studio__header-actions">
            {draftError && <span className="design-tokens-studio__save-error">{draftError}</span>}
            {isDirty && <span className="design-tokens-studio__dirty">{t('designCanvas.studio.unsaved')}</span>}
            <Button type="button" size="small" variant="ghost" disabled={!isDirty || savingDraft} onClick={resetDraft}>
                {t('designCanvas.studio.revert')}
            </Button>
            <Button
              type="button"
              size="small"
              variant="secondary"
              disabled={!isDirty || savingDraft}
              isLoading={savingDraft}
              onClick={saveDraft}
            >
              {t('designCanvas.studio.saveEdits')}
            </Button>
            <Button type="button" size="small" variant="primary" className="design-tokens-studio__commit" onClick={commit}>
              <Check size={14} />
              {document?.committed_id === draftProposal.id ? t('designCanvas.studio.recommit') : t('designCanvas.studio.adopt')}
            </Button>
          </div>
        </header>

        <section className="design-tokens-studio__scheme-header">
          <div className="design-tokens-studio__section-head">
            <h3>{t('designCanvas.studio.sidebarTitle')}</h3>
            <span>{t('designCanvas.studio.proposalCount', { count: proposals.length })}</span>
          </div>
          <div
            className="design-tokens-studio__scheme-tabs"
            role="tablist"
            aria-label={t('designCanvas.studio.sidebarTitle')}
          >
            {proposals.map((proposal) => {
              const isCommitted = document?.committed_id === proposal.id;
              const isActive = proposal.id === draftProposal.id;
              const classes = [
                'design-tokens-studio__proposal',
                isActive ? 'is-active' : '',
                isCommitted ? 'is-committed' : '',
              ].filter(Boolean).join(' ');
              return (
                <button
                  key={proposal.id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
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
          </div>
        </section>

        <section className="design-tokens-studio__section">
          <div className="design-tokens-studio__section-head">
            <h3>{t('designCanvas.studio.palette')}</h3>
            <span>{t('designCanvas.studio.tokenCount', { count: colorEntries.length })}</span>
          </div>
          <div className="design-tokens-studio__swatches">
            {colorEntries.map(([name, value]) => (
              <div key={name} className="design-tokens-studio__swatch">
                <ColorSwatchButton
                  name={name}
                  value={value}
                  proposal={draftProposal}
                  onChange={setDraft}
                />
                <div className="design-tokens-studio__swatch-meta">
                  <code>{name}</code>
                  <EditableValue
                    value={String(value)}
                    onChange={(next) => setDraft(updateRecordValue(draftProposal, 'colors', name, next))}
                  />
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
              {scaleEntries.length === 0 ? (
                <div className="design-tokens-studio__empty-fill">
                  <p className="design-tokens-studio__empty-fill-caption">
                    {t('designCanvas.studio.emptySectionSchemaDefaults')}
                  </p>
                  {defaultTypeRamp.map(({ role, size }) => (
                    <div
                      key={role}
                      className="design-tokens-studio__type-row design-tokens-studio__type-row--placeholder"
                    >
                      <div className="design-tokens-studio__type-sample" style={{ fontSize: size }}>
                        {t('designCanvas.studio.typeSample')}
                      </div>
                      <div className="design-tokens-studio__type-meta">
                        <code>{t(`designCanvas.studio.typeRole.${role}`)}</code>
                        <span className="design-tokens-studio__empty-fill-value">{size}</span>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                scaleEntries.map(([name, value]) => (
                  <div key={name} className="design-tokens-studio__type-row">
                    <div className="design-tokens-studio__type-sample" style={{ fontSize: value }}>
                      {t('designCanvas.studio.typeSample')}
                    </div>
                    <div className="design-tokens-studio__type-meta">
                      <code>{name}</code>
                      <EditableValue
                        value={value}
                        onChange={(next) => setDraft(updateRecordValue(draftProposal, 'typography', 'scale', next, name))}
                      />
                    </div>
                  </div>
                ))
              )}
              {entries(weight).length > 0 && (
                <div className="design-tokens-studio__weights">
                  {entries(weight).map(([name, value]) => (
                    <span key={name} style={{ fontWeight: value as any }}>
                      {name} /
                      <EditableValue
                        value={value}
                        onChange={(next) => setDraft(updateRecordValue(draftProposal, 'typography', 'weight', next, name))}
                      />
                    </span>
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
            {radiusEntries.length === 0 ? (
              <div className="design-tokens-studio__empty-fill">
                <p className="design-tokens-studio__empty-fill-caption">
                  {t('designCanvas.studio.emptySectionSchemaDefaults')}
                </p>
                <div className="design-tokens-studio__tiles">
                  {defaultRadiusTiles.map(([name, value]) => (
                    <div key={name} className="design-tokens-studio__tile design-tokens-studio__tile--placeholder">
                      <div
                        className="design-tokens-studio__tile-swatch"
                        style={{ borderRadius: value as string }}
                      />
                      <code>{name}</code>
                      <span className="design-tokens-studio__empty-fill-value">{value}</span>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="design-tokens-studio__tiles">
                {radiusEntries.map(([name, value]) => (
                  <div key={name} className="design-tokens-studio__tile">
                    <div
                      className="design-tokens-studio__tile-swatch"
                      style={{ borderRadius: value as string }}
                    />
                    <code>{name}</code>
                    <EditableValue
                      value={value}
                      onChange={(next) => setDraft(updateRecordValue(draftProposal, 'radius', name, next))}
                    />
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.shadow')}</h3>
              <span>{t('designCanvas.studio.tokenCount', { count: shadowEntries.length })}</span>
            </div>
            {shadowEntries.length === 0 ? (
              <div className="design-tokens-studio__empty-fill">
                <p className="design-tokens-studio__empty-fill-caption">
                  {t('designCanvas.studio.emptySectionSchemaDefaults')}
                </p>
                <div className="design-tokens-studio__tiles">
                  {defaultShadowTiles.map(([name, value]) => (
                    <div key={name} className="design-tokens-studio__tile design-tokens-studio__tile--placeholder">
                      <div
                        className="design-tokens-studio__tile-swatch design-tokens-studio__tile-swatch--shadow"
                        style={{ boxShadow: value as string }}
                      />
                      <code>{name}</code>
                      <span className="design-tokens-studio__empty-fill-value design-tokens-studio__empty-fill-value--shadow">
                        {value}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <div className="design-tokens-studio__tiles">
                {shadowEntries.map(([name, value]) => (
                  <div key={name} className="design-tokens-studio__tile">
                    <div
                      className="design-tokens-studio__tile-swatch design-tokens-studio__tile-swatch--shadow"
                      style={{ boxShadow: value as string }}
                    />
                    <code>{name}</code>
                    <EditableValue
                      className="design-tokens-studio__tile-mono"
                      value={String(value)}
                      title={String(value)}
                      onChange={(next) => setDraft(updateRecordValue(draftProposal, 'shadow', name, next))}
                    />
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="design-tokens-studio__section">
            <div className="design-tokens-studio__section-head">
              <h3>{t('designCanvas.studio.spacing')}</h3>
              <span>{t('designCanvas.studio.tokenCount', { count: spacingEntries.length })}</span>
            </div>
            <div className="design-tokens-studio__spacing">
              {spacingEntries.length === 0 ? (
                <div className="design-tokens-studio__empty-fill">
                  <p className="design-tokens-studio__empty-fill-caption">
                    {t('designCanvas.studio.emptySectionSchemaDefaults')}
                  </p>
                  {defaultSpacingRows.map(([name, value]) => {
                    const num = parseInt(String(value), 10);
                    const w = isNaN(num) ? 8 : Math.min(64, Math.max(2, num));
                    return (
                      <div
                        key={name}
                        className="design-tokens-studio__spacing-row design-tokens-studio__spacing-row--placeholder"
                      >
                        <span className="design-tokens-studio__spacing-bar" style={{ width: w }} />
                        <code>{name}</code>
                        <span className="design-tokens-studio__empty-fill-value">{value}</span>
                      </div>
                    );
                  })}
                </div>
              ) : (
                spacingEntries.map(([name, value]) => {
                  const num = parseInt(String(value), 10);
                  const w = isNaN(num) ? 8 : Math.min(64, Math.max(2, num));
                  return (
                    <div key={name} className="design-tokens-studio__spacing-row">
                      <span className="design-tokens-studio__spacing-bar" style={{ width: w }} />
                      <code>{name}</code>
                      <EditableValue
                        value={value}
                        onChange={(next) => setDraft(updateRecordValue(draftProposal, 'spacing', name, next))}
                      />
                    </div>
                  );
                })
              )}
            </div>
          </section>
        </div>

        <section className="design-tokens-studio__section">
          <div className="design-tokens-studio__section-head">
            <h3>{t('designCanvas.studio.componentPreview')}</h3>
            <div className="design-tokens-studio__preview-tabs" role="tablist" aria-label={t('designCanvas.studio.componentPreview')}>
              {previewSurfaces.map((surface) => (
                <button
                  key={surface.mode}
                  type="button"
                  role="tab"
                  aria-selected={surface.mode === activePreviewMode}
                  className={surface.mode === activePreviewMode ? 'is-active' : ''}
                  onClick={() => setActivePreviewMode(surface.mode)}
                >
                  {surface.label}
                </button>
              ))}
            </div>
          </div>
          <div className="design-tokens-studio__preview-pair">
            {[activePreviewSurface].map((surface) => (
              <div key={surface.mode} className={`design-tokens-studio__preview design-tokens-studio__preview--${surface.mode}`} style={surface.vars}>
                <div className="preview-landing">
                  <div className="preview-landing__nav">
                    <div className="preview-landing__brand">
                      <span className="preview-landing__mark" aria-hidden="true">
                        <Orbit size={14} strokeWidth={2} />
                      </span>
                      <span title="BitFun">BitFun</span>
                    </div>
                    <nav className="preview-landing__navlinks" aria-label={t('designCanvas.studio.previewBitfunNavAria')}>
                      <span>{t('designCanvas.studio.previewBitfunNavAgents')}</span>
                      <span>{t('designCanvas.studio.previewBitfunNavWorkflows')}</span>
                      <span>{t('designCanvas.studio.previewBitfunNavDocs')}</span>
                    </nav>
                    <button className="preview-btn preview-btn--primary preview-landing__cta" type="button">
                      {t('designCanvas.studio.previewBitfunNavCta')}
                    </button>
                  </div>

                  <section className="preview-landing__hero">
                    <div className="preview-landing__hero-copy">
                      <h4>
                        <em>{draftProposal.name}</em>
                        {t('designCanvas.studio.previewBitfunHeadingSuffix')}
                      </h4>
                      <p>
                        {surface.mode === 'native'
                          ? t('designCanvas.studio.previewBitfunNativeDesc')
                          : t('designCanvas.studio.previewBitfunStressDesc')}
                      </p>
                      <div className="preview-landing__actions">
                        <button className="preview-btn preview-btn--primary" type="button">
                          {t('designCanvas.studio.previewBitfunCtaPrimary')}
                        </button>
                        <button className="preview-btn preview-btn--ghost" type="button">
                          {t('designCanvas.studio.previewBitfunCtaSecondary')}
                        </button>
                      </div>
                    </div>

                    <div className="preview-landing__stage" aria-hidden="true">
                      <div className="preview-landing__orbit">
                        <span className="preview-landing__orbit-ring preview-landing__orbit-ring--outer" />
                        <span className="preview-landing__orbit-ring preview-landing__orbit-ring--mid" />
                        <span className="preview-landing__orbit-ring preview-landing__orbit-ring--inner" />
                        <span className="preview-landing__orbit-core">
                          <Orbit size={20} strokeWidth={1.75} />
                        </span>
                        <span className="preview-landing__orbit-node preview-landing__orbit-node--a" />
                        <span className="preview-landing__orbit-node preview-landing__orbit-node--b" />
                        <span className="preview-landing__orbit-node preview-landing__orbit-node--c" />
                      </div>
                    </div>
                  </section>

                  <div className="preview-landing__proof">
                    <span>
                      <strong>12+</strong>
                      {t('designCanvas.studio.previewBitfunProofOne')}
                    </span>
                    <span>
                      <strong>OSS</strong>
                      {t('designCanvas.studio.previewBitfunProofTwo')}
                    </span>
                    <span>
                      <strong>0-Cloud</strong>
                      {t('designCanvas.studio.previewBitfunProofThree')}
                    </span>
                  </div>
                </div>

                <div className="preview-download">
                  <header className="preview-download__head">
                    <div className="preview-download__brand">
                      <span className="preview-app__mark" aria-hidden="true">
                        <Orbit size={14} strokeWidth={2} />
                      </span>
                      <span>{t('designCanvas.studio.previewBitfunDownloadBrand')}</span>
                    </div>
                    <span className="preview-download__version">v1.8.0 · 2026-04-12</span>
                  </header>

                  <section className="preview-download__hero">
                    <h4>{t('designCanvas.studio.previewBitfunDownloadTitle')}</h4>
                    <p>{t('designCanvas.studio.previewBitfunDownloadDesc')}</p>
                  </section>

                  <section className="preview-download__platforms" aria-label={t('designCanvas.studio.previewBitfunDownloadPlatformsAria')}>
                    <article className="preview-platform is-primary">
                      <span className="preview-platform__icon" aria-hidden="true">
                        <Apple size={16} strokeWidth={1.9} />
                      </span>
                      <div className="preview-platform__meta">
                        <strong>macOS</strong>
                        <span>Apple Silicon · Intel</span>
                      </div>
                      <span className="preview-platform__size">14.2 MB · .dmg</span>
                      <button className="preview-btn preview-btn--primary preview-platform__cta" type="button">
                        <ArrowDown size={12} strokeWidth={2.2} />
                        {t('designCanvas.studio.previewBitfunDownloadCta')}
                      </button>
                    </article>

                    <article className="preview-platform">
                      <span className="preview-platform__icon" aria-hidden="true">
                        <Monitor size={16} strokeWidth={1.9} />
                      </span>
                      <div className="preview-platform__meta">
                        <strong>Windows</strong>
                        <span>Windows 10+</span>
                      </div>
                      <span className="preview-platform__size">12.8 MB · .msi</span>
                      <button className="preview-btn preview-btn--ghost preview-platform__cta" type="button">
                        <ArrowDown size={12} strokeWidth={2.2} />
                        {t('designCanvas.studio.previewBitfunDownloadCta')}
                      </button>
                    </article>

                    <article className="preview-platform">
                      <span className="preview-platform__icon" aria-hidden="true">
                        <Terminal size={16} strokeWidth={1.9} />
                      </span>
                      <div className="preview-platform__meta">
                        <strong>Linux</strong>
                        <span>.AppImage · .deb · .rpm</span>
                      </div>
                      <span className="preview-platform__size">15.1 MB</span>
                      <button className="preview-btn preview-btn--ghost preview-platform__cta" type="button">
                        <ArrowDown size={12} strokeWidth={2.2} />
                        {t('designCanvas.studio.previewBitfunDownloadCta')}
                      </button>
                    </article>
                  </section>

                  <footer className="preview-download__meta">
                    <span>
                      <Check size={11} strokeWidth={2.4} />
                      {t('designCanvas.studio.previewBitfunDownloadMetaSigned')}
                    </span>
                    <span>
                      <Check size={11} strokeWidth={2.4} />
                      {t('designCanvas.studio.previewBitfunDownloadMetaChecksum')}
                    </span>
                    <span>
                      <Check size={11} strokeWidth={2.4} />
                      {t('designCanvas.studio.previewBitfunDownloadMetaOss')}
                    </span>
                  </footer>
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
