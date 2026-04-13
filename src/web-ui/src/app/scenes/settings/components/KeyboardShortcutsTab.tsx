/**
 * KeyboardShortcutsTab — settings page for viewing and remapping keyboard shortcuts.
 *
 * Features:
 * - Grouped by scope with i18n labels
 * - Search filter by action name or key combination
 * - Click-to-record new key binding (capture mode)
 * - Real-time conflict detection (highlighted in red)
 * - Apply saves to configManager at path 'app.keybindings'
 * - Reset button restores all defaults
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Button, Search, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { ConfigPageLayout, ConfigPageHeader, ConfigPageContent, ConfigPageSection } from '@/infrastructure/config/components/common';
import {
  shortcutManager,
  parseStoredKeybindings,
  buildStoredKeybindings,
  type ShortcutRegistration,
  type KeybindingOverrides,
} from '@/infrastructure/services/ShortcutManager';
import { configManager } from '@/infrastructure/config';
import type { ShortcutConfig, ShortcutScope } from '@/shared/types/shortcut';
import {
  ALL_SHORTCUTS,
  SCOPE_ORDER,
  SCOPE_LABEL_KEYS,
  getShortcutDescriptionI18nKey,
  NON_USER_CUSTOMIZABLE_SHORTCUT_IDS,
} from '@/shared/constants/shortcuts';
import { createLogger } from '@/shared/utils/logger';
import './KeyboardShortcutsTab.scss';

const log = createLogger('KeyboardShortcutsTab');

const SCOPE_DISPLAY_ORDER: ShortcutScope[] = SCOPE_ORDER;

/** SceneBar Alt+1–3: merged into one row in settings; not listed individually */
const MERGED_SCENE_FOCUS_IDS = new Set(['scene.focus1', 'scene.focus2', 'scene.focus3']);

/** Canvas tabs Mod+1–9: merged into one row in settings; not listed individually */
const MERGED_TAB_SWITCH_IDS = new Set([
  'tab.switch1',
  'tab.switch2',
  'tab.switch3',
  'tab.switch4',
  'tab.switch5',
  'tab.switch6',
  'tab.switch7',
  'tab.switch8',
  'tab.switchLast',
]);

const MERGED_SCENE_RECORD_ID = '__merged_scene__';
const MERGED_TAB_RECORD_ID = '__merged_tab__';

const SCENE_FOCUS_ORDER = ['scene.focus1', 'scene.focus2', 'scene.focus3'] as const;
const TAB_SWITCH_ORDER = [
  'tab.switch1',
  'tab.switch2',
  'tab.switch3',
  'tab.switch4',
  'tab.switch5',
  'tab.switch6',
  'tab.switch7',
  'tab.switch8',
  'tab.switchLast',
] as const;

/**
 * Merge live ShortcutManager registrations with the static catalog so the settings UI
 * lists every known shortcut even when no runtime useShortcut has registered yet.
 */
/** Key cap label: bare space is invisible in UI; other single chars stay upper-case. */
function formatShortcutKeyCap(key: string, t: (key: string) => string): string {
  if (key === ' ') return t('keyboard.keyLabels.space');
  if (key.length === 1) return key.toUpperCase();
  return key;
}

function mergeCatalogWithLive(live: ShortcutRegistration[]): ShortcutRegistration[] {
  const liveById = new Map(live.map((r) => [r.id, r]));
  const out: ShortcutRegistration[] = [];
  for (const def of ALL_SHORTCUTS) {
    const hit = liveById.get(def.id);
    if (hit) {
      out.push(hit);
      liveById.delete(def.id);
    } else {
      out.push({
        id: def.id,
        config: shortcutManager.getEffectiveConfig(def.id, def.config),
        callback: () => {},
        priority: 0,
        description: def.descriptionKey,
      });
    }
  }
  for (const r of liveById.values()) {
    out.push(r);
  }
  out.sort((a, b) => {
    const scopeOrder: Record<ShortcutScope, number> = { app: 0, chat: 1, canvas: 2, filetree: 3 };
    const sa = scopeOrder[a.config.scope ?? 'app'];
    const sb = scopeOrder[b.config.scope ?? 'app'];
    if (sa !== sb) return sa - sb;
    return b.priority - a.priority;
  });
  return out;
}

function captureDigit(e: KeyboardEvent): string | null {
  const code = e.code ?? '';
  const d = /^Digit([1-9])$/i.exec(code);
  if (d) return d[1];
  const n = /^Numpad([1-9])$/i.exec(code);
  if (n) return n[1];
  return null;
}

function modSignature(cfg: ShortcutConfig): string {
  const isMac = typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');
  const primary = isMac ? Boolean(cfg.meta || cfg.ctrl) : Boolean(cfg.ctrl);
  return [primary ? '1' : '0', cfg.shift ? '1' : '0', cfg.alt ? '1' : '0'].join('');
}

function getEffectiveConfig(reg: ShortcutRegistration, pending?: PendingChange): ShortcutConfig {
  if (!pending) return reg.config;
  return {
    ...reg.config,
    key: pending.key,
    ctrl: pending.ctrl,
    shift: pending.shift,
    alt: pending.alt,
  };
}

function sceneGroupUniform(
  registrations: ShortcutRegistration[],
  pending: Record<string, PendingChange>
): boolean {
  const cfgs = SCENE_FOCUS_ORDER.map((id) => {
    const reg = registrations.find((r) => r.id === id);
    if (!reg) return null;
    return getEffectiveConfig(reg, pending[id]);
  });
  if (cfgs.some((c) => !c)) return false;
  const m0 = modSignature(cfgs[0]!);
  return (
    m0 === modSignature(cfgs[1]!) &&
    m0 === modSignature(cfgs[2]!) &&
    cfgs[0]!.key === '1' &&
    cfgs[1]!.key === '2' &&
    cfgs[2]!.key === '3'
  );
}

function tabGroupUniform(
  registrations: ShortcutRegistration[],
  pending: Record<string, PendingChange>
): boolean {
  const cfgs = TAB_SWITCH_ORDER.map((id) => {
    const reg = registrations.find((r) => r.id === id);
    if (!reg) return null;
    return getEffectiveConfig(reg, pending[id]);
  });
  if (cfgs.some((c) => !c)) return false;
  const m0 = modSignature(cfgs[0]!);
  for (let i = 0; i < 9; i++) {
    if (modSignature(cfgs[i]!) !== m0) return false;
    if (cfgs[i]!.key !== String(i + 1)) return false;
  }
  return true;
}

function formatModifierPrefix(cfg: ShortcutConfig): string {
  const isMac = typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');
  const primary = isMac ? Boolean(cfg.meta || cfg.ctrl) : Boolean(cfg.ctrl);
  const parts: string[] = [];
  if (primary) parts.push(isMac ? '⌘' : 'Ctrl');
  if (cfg.shift) parts.push(isMac ? '⇧' : 'Shift');
  if (cfg.alt) parts.push(isMac ? '⌥' : 'Alt');
  return parts.join(isMac ? '' : '+');
}

function formatMergedRangeLabel(cfg: ShortcutConfig, range: '1–3' | '1–9'): string {
  const prefix = formatModifierPrefix(cfg);
  const isMac = typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');
  if (!prefix) return range;
  return isMac ? `${prefix}${range}` : `${prefix}+${range}`;
}

function firstConflictInGroup(
  ids: readonly string[],
  registrations: ShortcutRegistration[],
  pendingChanges: Record<string, PendingChange>
): ShortcutRegistration | null {
  for (const id of ids) {
    const reg = registrations.find((r) => r.id === id);
    if (!reg) continue;
    const eff = getEffectiveConfig(reg, pendingChanges[id]);
    const conflicts = shortcutManager.checkConflicts(
      {
        key: eff.key,
        ctrl: !!eff.ctrl,
        shift: !!eff.shift,
        alt: !!eff.alt,
        meta: eff.meta,
        scope: eff.scope ?? 'app',
      },
      id,
      [...ids].filter((x) => x !== id)
    );
    if (conflicts[0]) return conflicts[0];
  }
  return null;
}

interface PendingChange {
  id: string;
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

/** Human-readable label for settings list (always via settings namespace + catalog). */
function shortcutDisplayName(
  reg: ShortcutRegistration,
  t: (key: string) => string
): string {
  const i18nKey =
    getShortcutDescriptionI18nKey(reg.id) ??
    (reg.description?.startsWith('keyboard.') ? reg.description : undefined);
  if (i18nKey) {
    const text = t(i18nKey);
    if (text && text !== i18nKey) return text;
  }
  if (reg.description && !reg.description.startsWith('keyboard.')) {
    return reg.description;
  }
  return reg.id.replace(/[._]/g, ' ');
}

const KeyboardShortcutsTab: React.FC = () => {
  const { t } = useI18n('settings');

  const [registrations, setRegistrations] = useState<ShortcutRegistration[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [pendingChanges, setPendingChanges] = useState<Record<string, PendingChange>>({});
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const recordingRef = useRef<string | null>(null);
  recordingRef.current = recordingId;

  // Live registrations from ShortcutManager (effects may register after first paint).
  const refreshRegistrations = useCallback(() => {
    setRegistrations(shortcutManager.getAllRegistrations());
  }, []);

  useEffect(() => {
    refreshRegistrations();
    return shortcutManager.subscribeRegistrationChanges(refreshRegistrations);
  }, [refreshRegistrations]);

  /** Full list for the UI: catalog + live, so every scope shows even before hooks register. */
  const displayRegistrations = useMemo(() => mergeCatalogWithLive(registrations), [registrations]);

  // Key capture during recording mode
  useEffect(() => {
    if (!recordingId) return;

    const handleCapture = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        setRecordingId(null);
        return;
      }

      const isMac = navigator.platform.toUpperCase().includes('MAC');
      const ctrl = isMac ? e.metaKey : e.ctrlKey;

      if (recordingId === MERGED_SCENE_RECORD_ID) {
        if (!captureDigit(e)) return;
        const next: Record<string, PendingChange> = {};
        for (let i = 0; i < SCENE_FOCUS_ORDER.length; i++) {
          const id = SCENE_FOCUS_ORDER[i];
          next[id] = {
            id,
            key: String(i + 1),
            ctrl,
            shift: e.shiftKey,
            alt: e.altKey,
          };
        }
        setPendingChanges((prev) => ({ ...prev, ...next }));
        setRecordingId(null);
        return;
      }

      if (recordingId === MERGED_TAB_RECORD_ID) {
        if (!captureDigit(e)) return;
        const next: Record<string, PendingChange> = {};
        for (let i = 0; i < TAB_SWITCH_ORDER.length; i++) {
          const id = TAB_SWITCH_ORDER[i];
          next[id] = {
            id,
            key: String(i + 1),
            ctrl,
            shift: e.shiftKey,
            alt: e.altKey,
          };
        }
        setPendingChanges((prev) => ({ ...prev, ...next }));
        setRecordingId(null);
        return;
      }

      // Skip modifier-only presses
      if (['Control', 'Shift', 'Alt', 'Meta', 'OS'].includes(e.key)) return;

      setPendingChanges((prev) => ({
        ...prev,
        [recordingId]: {
          id: recordingId,
          key: e.key,
          ctrl,
          shift: e.shiftKey,
          alt: e.altKey,
        },
      }));
      setRecordingId(null);
    };

    window.addEventListener('keydown', handleCapture, true);
    return () => window.removeEventListener('keydown', handleCapture, true);
  }, [recordingId]);

  // Detect conflicts for a given pending change
  const detectConflict = useCallback(
    (
      change: PendingChange,
      originalScope: ShortcutScope,
      excludeIds?: string[]
    ): ShortcutRegistration | null => {
      const conflicts = shortcutManager.checkConflicts(
        { key: change.key, ctrl: change.ctrl, shift: change.shift, alt: change.alt, scope: originalScope },
        change.id,
        excludeIds
      );
      return conflicts[0] ?? null;
    },
    []
  );

  // Apply all pending changes
  const handleApply = useCallback(async () => {
    setSaving(true);
    setSaveError(null);
    try {
      // 1. Read existing stored overrides so previous sessions' changes are not lost
      let existingOverrides: KeybindingOverrides = {};
      try {
        const raw = await configManager.getConfig('app.keybindings');
        existingOverrides = parseStoredKeybindings(raw);
      } catch {
        // First use — no stored overrides yet
      }

      // 2. Build overrides map from this session's pending changes
      const pendingOverrides: KeybindingOverrides = {};
      for (const [id, change] of Object.entries(pendingChanges)) {
        pendingOverrides[id] = {
          key: change.key,
          ...(change.ctrl  ? { ctrl:  true } : {}),
          ...(change.shift ? { shift: true } : {}),
          ...(change.alt   ? { alt:   true } : {}),
        };
      }

      // 3. Merge: pending changes override existing ones
      const merged: KeybindingOverrides = { ...existingOverrides, ...pendingOverrides };

      // 4. Prune overrides whose shortcut ID no longer exists in the catalog,
      //    preventing unbounded storage growth as shortcuts are added/removed.
      const knownIds = new Set(ALL_SHORTCUTS.map((d) => d.id));
      for (const id of Object.keys(merged)) {
        if (!knownIds.has(id) || NON_USER_CUSTOMIZABLE_SHORTCUT_IDS.has(id)) delete merged[id];
      }

      // 5. Persist with versioned format + sync in-memory state
      await configManager.setConfig('app.keybindings', buildStoredKeybindings(merged));
      shortcutManager.loadUserOverrides(merged);
      setPendingChanges({});
      refreshRegistrations();
    } catch (err) {
      log.error('Failed to save keybindings', err);
      setSaveError(t('keyboard.saveError'));
    } finally {
      setSaving(false);
    }
  }, [pendingChanges, refreshRegistrations, t]);

  // Reset all to defaults
  const handleReset = useCallback(async () => {
    setSaving(true);
    setSaveError(null);
    try {
      await configManager.setConfig('app.keybindings', buildStoredKeybindings({}));
      shortcutManager.loadUserOverrides({});
      setPendingChanges({});
      refreshRegistrations();
    } catch (err) {
      log.error('Failed to reset keybindings', err);
      setSaveError(t('keyboard.resetError'));
    } finally {
      setSaving(false);
    }
  }, [refreshRegistrations, t]);

  // Format a key combination from a registration or pending change
  const formatKey = (reg: ShortcutRegistration, pending?: PendingChange): string => {
    const isMac = navigator.platform.toUpperCase().includes('MAC');
    const cfg: ShortcutConfig = pending
      ? { ...reg.config, key: pending.key, ctrl: pending.ctrl, shift: pending.shift, alt: pending.alt }
      : reg.config;
    const parts: string[] = [];
    const primaryMod = isMac ? cfg.ctrl || cfg.meta : cfg.ctrl;
    if (primaryMod) parts.push(isMac ? '⌘' : 'Ctrl');
    if (cfg.shift) parts.push(isMac ? '⇧' : 'Shift');
    if (cfg.alt) parts.push(isMac ? '⌥' : 'Alt');
    parts.push(formatShortcutKeyCap(cfg.key, t));
    return parts.join(isMac ? '' : '+');
  };

  // Filter by search query (match translated action name or key combo)
  const filteredByScope = (scope: ShortcutScope): ShortcutRegistration[] => {
    const q = searchQuery.toLowerCase();
    return displayRegistrations.filter((r) => {
      if ((r.config.scope ?? 'app') !== scope) return false;
      if (!q) return true;
      const name = shortcutDisplayName(r, t).toLowerCase();
      const descMatch = name.includes(q) || r.id.toLowerCase().includes(q);
      const keyMatch = formatKey(r).toLowerCase().includes(q);
      return descMatch || keyMatch;
    });
  };

  /** Whether the merged SceneBar row matches the current search query */
  const mergedSceneRowVisible = useMemo(() => {
    const hasRegs = displayRegistrations.some((r) => MERGED_SCENE_FOCUS_IDS.has(r.id));
    if (!hasRegs) return false;
    const q = searchQuery.trim().toLowerCase();
    if (!q) return true;
    const hay = [
      t('keyboard.shortcuts.scene.focusMerged'),
      t('keyboard.shortcuts.scene.focusMergedHint'),
      t('keyboard.mergedNonUniform'),
    ]
      .join(' ')
      .toLowerCase();
    return hay.includes(q);
  }, [displayRegistrations, searchQuery, t]);

  /** Whether the merged canvas-tab row matches the current search query */
  const mergedTabSwitchRowVisible = useMemo(() => {
    const hasRegs = displayRegistrations.some((r) => MERGED_TAB_SWITCH_IDS.has(r.id));
    if (!hasRegs) return false;
    const q = searchQuery.trim().toLowerCase();
    if (!q) return true;
    const hay = [
      t('keyboard.shortcuts.tab.switchMerged'),
      t('keyboard.shortcuts.tab.switchMergedHint'),
      t('keyboard.mergedNonUniform'),
    ]
      .join(' ')
      .toLowerCase();
    return hay.includes(q);
  }, [displayRegistrations, searchQuery, t]);

  const mergedSceneKeyLabel = useMemo(() => {
    const firstId = SCENE_FOCUS_ORDER[0];
    const reg = displayRegistrations.find((r) => r.id === firstId);
    if (!reg) return t('keyboard.mergedNonUniform');
    if (!sceneGroupUniform(displayRegistrations, pendingChanges)) {
      return t('keyboard.mergedNonUniform');
    }
    const cfg = getEffectiveConfig(reg, pendingChanges[firstId]);
    return formatMergedRangeLabel(cfg, '1–3');
  }, [displayRegistrations, pendingChanges, t]);

  const mergedTabKeyLabel = useMemo(() => {
    const firstId = TAB_SWITCH_ORDER[0];
    const reg = displayRegistrations.find((r) => r.id === firstId);
    if (!reg) return t('keyboard.mergedNonUniform');
    if (!tabGroupUniform(displayRegistrations, pendingChanges)) {
      return t('keyboard.mergedNonUniform');
    }
    const cfg = getEffectiveConfig(reg, pendingChanges[firstId]);
    return formatMergedRangeLabel(cfg, '1–9');
  }, [displayRegistrations, pendingChanges, t]);

  const mergedSceneConflict = useMemo(
    () => firstConflictInGroup(SCENE_FOCUS_ORDER, displayRegistrations, pendingChanges),
    [displayRegistrations, pendingChanges]
  );

  const mergedTabConflict = useMemo(
    () => firstConflictInGroup(TAB_SWITCH_ORDER, displayRegistrations, pendingChanges),
    [displayRegistrations, pendingChanges]
  );

  const mergedScenePending = useMemo(
    () => SCENE_FOCUS_ORDER.some((id) => pendingChanges[id] !== undefined),
    [pendingChanges]
  );

  const mergedTabPending = useMemo(
    () => TAB_SWITCH_ORDER.some((id) => pendingChanges[id] !== undefined),
    [pendingChanges]
  );

  const appShortcutsWithoutMerged = filteredByScope('app').filter(
    (r) => !MERGED_SCENE_FOCUS_IDS.has(r.id)
  );
  const canvasShortcutsWithoutMerged = filteredByScope('canvas').filter(
    (r) => !MERGED_TAB_SWITCH_IDS.has(r.id)
  );
  const hasAnyVisibleShortcut =
    mergedTabSwitchRowVisible ||
    mergedSceneRowVisible ||
    appShortcutsWithoutMerged.length > 0 ||
    canvasShortcutsWithoutMerged.length > 0 ||
    filteredByScope('chat').length > 0 ||
    filteredByScope('filetree').length > 0;

  const hasPendingChanges = Object.keys(pendingChanges).length > 0;

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        className="kb-shortcuts-page-header"
        title={t('keyboard.title')}
        subtitle={t('keyboard.description')}
      />
      <ConfigPageContent>
        {/* Search + actions bar */}
        <div className="kb-shortcuts__toolbar">
          <Search
            className="kb-shortcuts__search"
            size="small"
            value={searchQuery}
            onChange={setSearchQuery}
            placeholder={t('keyboard.search')}
            inputAriaLabel={t('keyboard.search')}
            enterToSearch={false}
            clearable
          />
          <div className="kb-shortcuts__actions">
            {hasPendingChanges && (
              <Button
                variant="primary"
              size="small"
              onClick={handleApply}
              disabled={saving}
            >
              {saving ? t('keyboard.saving') : t('keyboard.apply')}
            </Button>
          )}
            <Button
              variant="secondary"
              size="small"
              onClick={handleReset}
              disabled={saving}
            >
              {t('keyboard.reset')}
            </Button>
          </div>
        </div>

        {saveError && (
          <div className="kb-shortcuts__error">{saveError}</div>
        )}

        {/* Shortcuts grouped by scope */}
        {SCOPE_DISPLAY_ORDER.map((scope) => {
          const rawItems = filteredByScope(scope);
          const showMergedTab = scope === 'canvas' && mergedTabSwitchRowVisible;
          const showMergedScene = scope === 'app' && mergedSceneRowVisible;
          const items =
            scope === 'app'
              ? rawItems.filter((r) => !MERGED_SCENE_FOCUS_IDS.has(r.id))
              : scope === 'canvas'
              ? rawItems.filter((r) => !MERGED_TAB_SWITCH_IDS.has(r.id))
              : rawItems;
          if (items.length === 0 && !showMergedScene && !showMergedTab) return null;

          return (
            <ConfigPageSection
              key={scope}
              title={t(SCOPE_LABEL_KEYS[scope])}
            >
              <div className="kb-shortcuts__list">
                {showMergedTab && (
                  <div
                    className={[
                      'kb-shortcuts__item kb-shortcuts__item--merged',
                      recordingId === MERGED_TAB_RECORD_ID ? 'kb-shortcuts__item--recording' : '',
                      mergedTabConflict ? 'kb-shortcuts__item--conflict' : '',
                      mergedTabPending ? 'kb-shortcuts__item--modified' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    key="tab-switch-merged"
                  >
                    <div className="kb-shortcuts__item-label">
                      <span className="kb-shortcuts__item-name">{t('keyboard.shortcuts.tab.switchMerged')}</span>
                      <span className="kb-shortcuts__item-hint">{t('keyboard.shortcuts.tab.switchMergedHint')}</span>
                      {mergedTabConflict && (
                        <span className="kb-shortcuts__item-conflict-hint">
                          {t('keyboard.conflict')}: {shortcutDisplayName(mergedTabConflict, t)}
                        </span>
                      )}
                    </div>
                    <div className="kb-shortcuts__item-key">
                      <Tooltip content={t('keyboard.clickToRecord')} placement="top">
                        <button
                          type="button"
                          className={[
                            'kb-shortcuts__keybadge',
                            recordingId === MERGED_TAB_RECORD_ID ? 'kb-shortcuts__keybadge--recording' : '',
                            mergedTabConflict ? 'kb-shortcuts__keybadge--conflict' : '',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                          onClick={() =>
                            setRecordingId(recordingId === MERGED_TAB_RECORD_ID ? null : MERGED_TAB_RECORD_ID)
                          }
                        >
                          {recordingId === MERGED_TAB_RECORD_ID ? t('keyboard.recording') : mergedTabKeyLabel}
                        </button>
                      </Tooltip>
                      {mergedTabPending && recordingId !== MERGED_TAB_RECORD_ID && (
                        <Tooltip content={t('keyboard.revertChange')} placement="top">
                          <button
                            type="button"
                            className="kb-shortcuts__revert-btn"
                            onClick={() => {
                              setPendingChanges((prev) => {
                                const next = { ...prev };
                                for (const id of TAB_SWITCH_ORDER) delete next[id];
                                return next;
                              });
                            }}
                          >
                            ↩
                          </button>
                        </Tooltip>
                      )}
                    </div>
                  </div>
                )}
                {showMergedScene && (
                  <div
                    className={[
                      'kb-shortcuts__item kb-shortcuts__item--merged',
                      recordingId === MERGED_SCENE_RECORD_ID ? 'kb-shortcuts__item--recording' : '',
                      mergedSceneConflict ? 'kb-shortcuts__item--conflict' : '',
                      mergedScenePending ? 'kb-shortcuts__item--modified' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    key="scene-focus-merged"
                  >
                    <div className="kb-shortcuts__item-label">
                      <span className="kb-shortcuts__item-name">{t('keyboard.shortcuts.scene.focusMerged')}</span>
                      <span className="kb-shortcuts__item-hint">{t('keyboard.shortcuts.scene.focusMergedHint')}</span>
                      {mergedSceneConflict && (
                        <span className="kb-shortcuts__item-conflict-hint">
                          {t('keyboard.conflict')}: {shortcutDisplayName(mergedSceneConflict, t)}
                        </span>
                      )}
                    </div>
                    <div className="kb-shortcuts__item-key">
                      <Tooltip content={t('keyboard.clickToRecord')} placement="top">
                        <button
                          type="button"
                          className={[
                            'kb-shortcuts__keybadge',
                            recordingId === MERGED_SCENE_RECORD_ID ? 'kb-shortcuts__keybadge--recording' : '',
                            mergedSceneConflict ? 'kb-shortcuts__keybadge--conflict' : '',
                          ]
                            .filter(Boolean)
                            .join(' ')}
                          onClick={() =>
                            setRecordingId(recordingId === MERGED_SCENE_RECORD_ID ? null : MERGED_SCENE_RECORD_ID)
                          }
                        >
                          {recordingId === MERGED_SCENE_RECORD_ID ? t('keyboard.recording') : mergedSceneKeyLabel}
                        </button>
                      </Tooltip>
                      {mergedScenePending && recordingId !== MERGED_SCENE_RECORD_ID && (
                        <Tooltip content={t('keyboard.revertChange')} placement="top">
                          <button
                            type="button"
                            className="kb-shortcuts__revert-btn"
                            onClick={() => {
                              setPendingChanges((prev) => {
                                const next = { ...prev };
                                for (const id of SCENE_FOCUS_ORDER) delete next[id];
                                return next;
                              });
                            }}
                          >
                            ↩
                          </button>
                        </Tooltip>
                      )}
                    </div>
                  </div>
                )}
                {items.map((reg) => {
                  const pending = pendingChanges[reg.id];
                  const isRecording = recordingId === reg.id;
                  const fixed = NON_USER_CUSTOMIZABLE_SHORTCUT_IDS.has(reg.id);
                  const conflict =
                    !fixed && pending ? detectConflict(pending, reg.config.scope ?? 'app') : null;

                  return (
                    <div
                      key={reg.id}
                      className={[
                        'kb-shortcuts__item',
                        !fixed && isRecording ? 'kb-shortcuts__item--recording' : '',
                        conflict ? 'kb-shortcuts__item--conflict' : '',
                        pending && !fixed ? 'kb-shortcuts__item--modified' : '',
                      ].filter(Boolean).join(' ')}
                    >
                      <div className="kb-shortcuts__item-label">
                        <span className="kb-shortcuts__item-name">
                          {shortcutDisplayName(reg, t)}
                        </span>
                        {conflict && (
                          <span className="kb-shortcuts__item-conflict-hint">
                            {t('keyboard.conflict')}: {shortcutDisplayName(conflict, t)}
                          </span>
                        )}
                      </div>
                      <div className="kb-shortcuts__item-key">
                        {fixed ? (
                          <Tooltip content={t('keyboard.fixedBinding')} placement="top">
                            <span
                              className={['kb-shortcuts__keybadge', 'kb-shortcuts__keybadge--readonly'].join(' ')}
                            >
                              {formatKey(reg)}
                            </span>
                          </Tooltip>
                        ) : (
                          <>
                            <Tooltip content={t('keyboard.clickToRecord')} placement="top">
                              <button
                                type="button"
                                className={[
                                  'kb-shortcuts__keybadge',
                                  isRecording ? 'kb-shortcuts__keybadge--recording' : '',
                                  conflict ? 'kb-shortcuts__keybadge--conflict' : '',
                                ].filter(Boolean).join(' ')}
                                onClick={() => setRecordingId(isRecording ? null : reg.id)}
                              >
                                {isRecording
                                  ? t('keyboard.recording')
                                  : formatKey(reg, pending)}
                              </button>
                            </Tooltip>
                            {pending && !isRecording && (
                              <Tooltip content={t('keyboard.revertChange')} placement="top">
                                <button
                                  type="button"
                                  className="kb-shortcuts__revert-btn"
                                  onClick={() => {
                                    setPendingChanges((prev) => {
                                      const next = { ...prev };
                                      delete next[reg.id];
                                      return next;
                                    });
                                  }}
                                >
                                  ↩
                                </button>
                              </Tooltip>
                            )}
                          </>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </ConfigPageSection>
          );
        })}

        {displayRegistrations.length > 0 && !hasAnyVisibleShortcut && (
          <div className="kb-shortcuts__empty">
            {t('keyboard.noResults')}
          </div>
        )}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default KeyboardShortcutsTab;
