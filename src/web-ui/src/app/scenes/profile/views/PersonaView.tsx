import React, {
  useCallback, useEffect, useMemo, useRef, useState,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import {
  Bot, ChevronRight, Pencil, X,
  ListChecks, RotateCcw,
} from 'lucide-react';
import { Input, Select, type SelectOption } from '@/component-library';
import { AIRulesAPI, RuleLevel, type AIRule } from '@/infrastructure/api/service-api/AIRulesAPI';
import { getAllMemories, toggleMemory, type AIMemory } from '@/infrastructure/api/aiMemoryApi';
import { promptTemplateService } from '@/infrastructure/services/PromptTemplateService';
import type { PromptTemplate } from '@/shared/types/prompt-template';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type {
  ModeConfigItem, SkillInfo, AIModelConfig,
  DefaultModelsConfig, AIExperienceConfig,
} from '@/infrastructure/config/types';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import type { ConfigTab } from '@/app/scenes/settings/settingsConfig';
import { quickActions } from '@/shared/services/ide-control';
import { PersonaRadar } from './PersonaRadar';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './PersonaView.scss';

const log = createLogger('PersonaView');

function navToSettings(tab: ConfigTab) {
  useSettingsStore.getState().setActiveTab(tab);
  quickActions.openSettings();
}

interface ToolInfo { name: string; description: string; is_readonly: boolean; }

const C = 'bp';
const IDENTITY_KEY = 'bf_agent_identity';
const DEFAULT_NAME = 'BitFun Agent';
const CHIP_LIMIT   = 12;

// Structural slot keys only — labels/descs are resolved via i18n at render time
const MODEL_SLOT_KEYS = ['primary', 'fast', 'compression', 'image', 'voice', 'retrieval'] as const;
type ModelSlotKey = typeof MODEL_SLOT_KEYS[number];

// Preset option IDs per slot (no translated labels here)
const SLOT_PRESET_IDS: Record<ModelSlotKey, { id: string }[]> = {
  primary:     [{ id: 'primary' }],
  fast:        [{ id: 'fast' }],
  compression: [{ id: 'fast' }],
  image:       [],
  voice:       [],
  retrieval:   [],
};

interface ToggleChipProps {
  label: string;
  enabled: boolean;
  onToggle: () => void;
  accentColor?: string;
  tooltip?: string;
  loading?: boolean;
}
const ToggleChip: React.FC<ToggleChipProps> = ({
  label, enabled, onToggle, accentColor, tooltip, loading,
}) => (
  <button
    type="button"
    className={`${C}-chip ${enabled ? 'is-on' : 'is-off'} ${loading ? 'is-loading' : ''}`}
    onClick={onToggle}
    title={tooltip ?? label}
    disabled={loading}
    style={accentColor ? { '--chip-accent': accentColor } as React.CSSProperties : undefined}
  >
    <span className={`${C}-chip__label`}>{label}</span>
  </button>
);

interface ModelPillProps {
  slotKey: ModelSlotKey;
  slotLabel: string;
  slotDesc: string;
  currentId: string;
  models: AIModelConfig[];
  defaultModels: DefaultModelsConfig | null;
  onChange: (id: string) => void;
}
const ModelPill: React.FC<ModelPillProps> = ({
  slotKey, slotLabel, slotDesc, currentId, models, defaultModels, onChange,
}) => {
  const { t } = useTranslation('scenes/profile');

  const presetDefs = SLOT_PRESET_IDS[slotKey];
  const isPreset = presetDefs.some(p => p.id === currentId);
  const isConfigured = currentId !== '';

  // Translate preset option labels
  const presetLabelFor = useCallback((id: string) => {
    if (id === 'primary') return t('slotDefault.primary');
    if (id === 'fast')    return t('slotDefault.fast');
    return id;
  }, [t]);

  // Placeholder when slot has no explicit assignment
  const defaultLabel = useMemo(() => {
    if (slotKey === 'primary')     return t('slotDefault.primary');
    if (slotKey === 'fast')        return t('slotDefault.fast');
    if (slotKey === 'compression') return t('slotDefault.fast');
    return t('slotDefault.unconfigured');
  }, [slotKey, t]);

  const options = useMemo<SelectOption[]>(() => {
    const presetOptions: SelectOption[] = presetDefs.map(p => ({
      value: `preset:${p.id}`,
      label: presetLabelFor(p.id),
      group: t('modelGroups.presets'),
    }));
    const modelOptions: SelectOption[] = models
      .filter(m => m.enabled && !!m.id)
      .map(m => ({
        value: `model:${m.id}`,
        label: m.name,
        group: t('modelGroups.models'),
      }));
    return [...presetOptions, ...modelOptions];
  }, [presetDefs, models, presetLabelFor, t]);

  const selectedValue = !currentId
    ? ''
    : isPreset
      ? `preset:${currentId}`
      : `model:${currentId}`;

  const handleSelect = useCallback((value: string | number | (string | number)[]) => {
    if (Array.isArray(value)) return;
    const raw = String(value);
    if (raw.startsWith('preset:')) {
      onChange(raw.replace('preset:', ''));
      return;
    }
    if (raw.startsWith('model:')) {
      onChange(raw.replace('model:', ''));
    }
  }, [onChange]);

  return (
    <div className={`${C}-model-cell`}>
      <div className={`${C}-model-cell__meta`}>
        <span className={`${C}-model-cell__label`}>{slotLabel}</span>
        <span className={`${C}-model-cell__desc`}>{slotDesc}</span>
      </div>
      <Select
        className={`${C}-model-select ${!isConfigured ? 'is-empty' : ''}`}
        size="small"
        options={options}
        value={selectedValue}
        onChange={handleSelect}
        placeholder={defaultLabel}
      />
    </div>
  );
};

const PersonaView: React.FC<{ workspacePath: string }> = () => {
  const { t } = useTranslation('scenes/profile');

  // Initialize identity from localStorage immediately (lazy initializer avoids flash)
  const [identity, setIdentity] = useState<{ name: string; desc: string }>(() => {
    try {
      const s = localStorage.getItem(IDENTITY_KEY);
      if (s) return JSON.parse(s) as { name: string; desc: string };
    } catch { /* ignore */ }
    return { name: DEFAULT_NAME, desc: '' };
  });
  const [editingField, setEditingField] = useState<'name' | 'desc' | null>(null);
  const [editValue, setEditValue] = useState('');
  const nameInputRef = useRef<HTMLInputElement>(null);
  const descInputRef = useRef<HTMLInputElement>(null);

  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig | null>(null);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});
  const [rules, setRules] = useState<AIRule[]>([]);
  const [memories, setMemories] = useState<AIMemory[]>([]);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [agenticConfig, setAgenticConfig] = useState<ModeConfigItem | null>(null);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [templates, setTemplates] = useState<PromptTemplate[]>([]);
  const [aiExp, setAiExp] = useState<Partial<AIExperienceConfig>>({
    enable_visual_mode: false,
    enable_session_title_generation: true,
    enable_welcome_panel_ai_analysis: true,
  });

  // loading maps (optimistic toggle)
  const [rulesLoading,   setRulesLoading]   = useState<Record<string, boolean>>({});
  const [memoriesLoading, setMemoriesLoading] = useState<Record<string, boolean>>({});
  const [toolsLoading,   setToolsLoading]   = useState<Record<string, boolean>>({});
  const [skillsLoading,  setSkillsLoading]  = useState<Record<string, boolean>>({});

  const [rulesExpanded,    setRulesExpanded]    = useState(false);
  const [memoriesExpanded, setMemoriesExpanded] = useState(false);
  const [skillsExpanded,   setSkillsExpanded]   = useState(false);

  const [radarOpen,    setRadarOpen]    = useState(false);
  const [radarClosing, setRadarClosing] = useState(false);
  const closingTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // section refs for radar-click navigation
  const rulesRef     = useRef<HTMLDivElement>(null);
  const memoryRef    = useRef<HTMLDivElement>(null);
  const toolsRef     = useRef<HTMLDivElement>(null);
  const skillsRef    = useRef<HTMLDivElement>(null);
  const templatesRef = useRef<HTMLDivElement>(null);
  const interactionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    (async () => {
      try {
        const [u, p, m] = await Promise.all([
          AIRulesAPI.getRules(RuleLevel.User),
          AIRulesAPI.getRules(RuleLevel.Project),
          getAllMemories(),
        ]);
        setRules([...u, ...p]);
        setMemories(m);
      } catch (e) { log.error('rules/memory', e); }
    })();
  }, []);

  useEffect(() => {
    const init = async () => {
      try { await promptTemplateService.initialize(); } finally {
        setTemplates(promptTemplateService.getAllTemplates());
      }
    };
    init();
    return promptTemplateService.subscribe(() => setTemplates(promptTemplateService.getAllTemplates()));
  }, []);

  const loadCaps = useCallback(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const [tools, mcps, sks, modeConf, allModels, defModels, funcModels, exp] = await Promise.all([
        invoke<ToolInfo[]>('get_all_tools_info').catch(() => [] as ToolInfo[]),
        MCPAPI.getServers().catch(() => [] as MCPServerInfo[]),
        configAPI.getSkillConfigs().catch(() => [] as SkillInfo[]),
        configAPI.getModeConfig('agentic').catch(() => null as ModeConfigItem | null),
        (configManager.getConfig<AIModelConfig[]>('ai.models') as Promise<AIModelConfig[]>).catch(() => [] as AIModelConfig[]),
        (configManager.getConfig<DefaultModelsConfig>('ai.default_models') as Promise<DefaultModelsConfig | null>).catch(() => null),
        (configManager.getConfig<Record<string, string>>('ai.func_agent_models') as Promise<Record<string, string>>).catch(() => ({} as Record<string, string>)),
        configAPI.getConfig('app.ai_experience').catch(() => null) as Promise<AIExperienceConfig | null>,
      ]);
      setAvailableTools(tools);
      setMcpServers(mcps);
      setSkills(sks);
      setAgenticConfig(modeConf);
      setModels(allModels ?? []);
      setDefaultModels(defModels);
      setFuncAgentModels(funcModels ?? {});
      if (exp) setAiExp(exp);
    } catch (e) { log.error('capabilities', e); }
  }, []);
  useEffect(() => { loadCaps(); }, [loadCaps]);

  const startEdit = (field: 'name' | 'desc') => {
    setEditingField(field);
    setEditValue(field === 'name' ? identity.name : (identity.desc || t('defaultDesc')));
    setTimeout(() => (field === 'name' ? nameInputRef : descInputRef).current?.focus(), 10);
  };
  const commitEdit = useCallback(() => {
    if (!editingField) return;
    const fallback = editingField === 'name' ? DEFAULT_NAME : t('defaultDesc');
    const updated = { ...identity, [editingField === 'name' ? 'name' : 'desc']: editValue.trim() || fallback };
    setIdentity(updated);
    localStorage.setItem(IDENTITY_KEY, JSON.stringify(updated));
    setEditingField(null);
  }, [editingField, editValue, identity, t]);
  const onEditKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') commitEdit();
    if (e.key === 'Escape') setEditingField(null);
  };

  const openRadar  = useCallback(() => setRadarOpen(true), []);
  const closeRadar = useCallback(() => {
    setRadarClosing(true);
    closingTimer.current = setTimeout(() => { setRadarOpen(false); setRadarClosing(false); }, 220);
  }, []);
  useEffect(() => {
    if (!radarOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') closeRadar(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [radarOpen, closeRadar]);
  useEffect(() => () => { if (closingTimer.current) clearTimeout(closingTimer.current); }, []);

  const handleRadarDimClick = useCallback((label: string) => {
    const map: Record<string, React.RefObject<HTMLDivElement | HTMLElement>> = {
      [t('radar.dims.rigor')]:       rulesRef,
      [t('radar.dims.memory')]:      memoryRef,
      [t('radar.dims.autonomy')]:    toolsRef,
      [t('radar.dims.adaptability')]: skillsRef,
      [t('radar.dims.creativity')]:  templatesRef,
      [t('radar.dims.expression')]:  interactionRef,
    };
    const target = map[label];
    if (target?.current) {
      target.current.scrollIntoView({ behavior: 'smooth', block: 'start' });
      target.current.classList.add('is-pulse');
      setTimeout(() => target.current?.classList.remove('is-pulse'), 900);
    }
    if (radarOpen) closeRadar();
  }, [radarOpen, closeRadar, t]);

  const handleModelChange = useCallback(async (key: string, id: string) => {
    try {
      const cur = await (configManager.getConfig<Record<string, string>>('ai.func_agent_models') as Promise<Record<string, string> | null>).catch(() => null) ?? {};
      const upd = { ...cur, [key]: id };
      await configManager.setConfig('ai.func_agent_models', upd);
      setFuncAgentModels(upd);
      notificationService.success(t('notifications.modelUpdated'), { duration: 1500 });
    } catch (e) { log.error('model update', e); notificationService.error(t('notifications.updateFailed')); }
  }, [t]);

  const toggleRule = useCallback(async (rule: AIRule) => {
    const key = `${rule.level}-${rule.name}`;
    const newEnabled = !rule.enabled;
    setRulesLoading(p => ({ ...p, [key]: true }));
    setRules(p => p.map(r => r.name === rule.name && r.level === rule.level ? { ...r, enabled: newEnabled } : r));
    try {
      await AIRulesAPI.updateRule(
        rule.level === RuleLevel.User ? RuleLevel.User : RuleLevel.Project,
        rule.name, { enabled: newEnabled },
      );
    } catch (e) {
      log.error('rule toggle', e);
      setRules(p => p.map(r => r.name === rule.name && r.level === rule.level ? { ...r, enabled: rule.enabled } : r));
      notificationService.error(t('notifications.toggleFailed'));
    } finally { setRulesLoading(p => { const n = { ...p }; delete n[key]; return n; }); }
  }, [t]);

  const toggleMem = useCallback(async (mem: AIMemory) => {
    setMemoriesLoading(p => ({ ...p, [mem.id]: true }));
    setMemories(p => p.map(m => m.id === mem.id ? { ...m, enabled: !m.enabled } : m));
    try { await toggleMemory(mem.id); }
    catch (e) {
      log.error('memory toggle', e);
      setMemories(p => p.map(m => m.id === mem.id ? { ...m, enabled: mem.enabled } : m));
      notificationService.error(t('notifications.toggleFailed'));
    } finally { setMemoriesLoading(p => { const n = { ...p }; delete n[mem.id]; return n; }); }
  }, [t]);

  const toggleTool = useCallback(async (name: string) => {
    if (!agenticConfig) return;
    setToolsLoading(p => ({ ...p, [name]: true }));
    const tools = agenticConfig.available_tools ?? [];
    const newTools = tools.includes(name) ? tools.filter(t => t !== name) : [...tools, name];
    const newCfg = { ...agenticConfig, available_tools: newTools };
    setAgenticConfig(newCfg);
    try {
      await configAPI.setModeConfig('agentic', newCfg);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('tool toggle', e);
      setAgenticConfig(agenticConfig);
      notificationService.error(t('notifications.toggleFailed'));
    } finally { setToolsLoading(p => { const n = { ...p }; delete n[name]; return n; }); }
  }, [agenticConfig, t]);

  const selectAllTools = useCallback(async () => {
    if (!agenticConfig) return;
    const c = { ...agenticConfig, available_tools: availableTools.map(t => t.name) };
    setAgenticConfig(c);
    try { await configAPI.setModeConfig('agentic', c); } catch { setAgenticConfig(agenticConfig); }
  }, [agenticConfig, availableTools]);

  const clearAllTools = useCallback(async () => {
    if (!agenticConfig) return;
    const c = { ...agenticConfig, available_tools: [] };
    setAgenticConfig(c);
    try { await configAPI.setModeConfig('agentic', c); } catch { setAgenticConfig(agenticConfig); }
  }, [agenticConfig]);

  const resetTools = useCallback(async () => {
    if (!window.confirm(t('notifications.resetConfirm'))) return;
    try { await configAPI.resetModeConfig('agentic'); await loadCaps(); notificationService.success(t('notifications.resetSuccess')); }
    catch { notificationService.error(t('notifications.resetFailed')); }
  }, [loadCaps, t]);

  const toggleSkill = useCallback(async (sk: SkillInfo) => {
    const newEnabled = !sk.enabled;
    setSkillsLoading(p => ({ ...p, [sk.name]: true }));
    setSkills(p => p.map(s => s.name === sk.name ? { ...s, enabled: newEnabled } : s));
    try { await configAPI.setSkillEnabled(sk.name, newEnabled); }
    catch (e) {
      log.error('skill toggle', e);
      setSkills(p => p.map(s => s.name === sk.name ? { ...s, enabled: sk.enabled } : s));
      notificationService.error(t('notifications.toggleFailed'));
    } finally { setSkillsLoading(p => { const n = { ...p }; delete n[sk.name]; return n; }); }
  }, [t]);

  const togglePref = useCallback(async (key: keyof AIExperienceConfig) => {
    const cur = aiExp[key] as boolean;
    setAiExp(p => ({ ...p, [key]: !cur }));
    try { await configAPI.setConfig(`app.ai_experience.${key}`, !cur); }
    catch { setAiExp(p => ({ ...p, [key]: cur })); }
  }, [aiExp]);

  const sortRules = useMemo(() =>
    [...rules].sort((a, b) => a.enabled === b.enabled ? 0 : a.enabled ? -1 : 1), [rules]);
  const sortMem = useMemo(() =>
    [...memories].sort((a, b) => a.enabled !== b.enabled ? (a.enabled ? -1 : 1) : b.importance - a.importance),
    [memories]);
  const sortTools = useMemo(() => {
    const en = agenticConfig?.available_tools ?? [];
    return [...availableTools].sort((a, b) => {
      const ao = en.includes(a.name), bo = en.includes(b.name);
      return ao !== bo ? (ao ? -1 : 1) : a.name.localeCompare(b.name);
    });
  }, [availableTools, agenticConfig]);
  const sortSkills = useMemo(() =>
    [...skills].sort((a, b) => a.enabled !== b.enabled ? (a.enabled ? -1 : 1) : a.name.localeCompare(b.name)),
    [skills]);
  const sortTemplates = useMemo(() =>
    [...templates].sort((a, b) => a.isFavorite !== b.isFavorite ? (a.isFavorite ? -1 : 1) : b.usageCount - a.usageCount),
    [templates]);

  const enabledRules = useMemo(() => rules.filter(r => r.enabled).length, [rules]);
  const userRules    = useMemo(() => rules.filter(r => r.level === RuleLevel.User).length, [rules]);
  const projRules    = useMemo(() => rules.filter(r => r.level === RuleLevel.Project).length, [rules]);
  const enabledMems  = useMemo(() => memories.filter(m => m.enabled).length, [memories]);
  const enabledTools = useMemo(() => agenticConfig?.available_tools?.length ?? 0, [agenticConfig]);
  const enabledSkls  = useMemo(() => skills.filter(s => s.enabled).length, [skills]);
  const healthyMcp   = useMemo(() => mcpServers.filter(s => s.status === 'Healthy' || s.status === 'Connected').length, [mcpServers]);

  const skillEn  = useMemo(() => skills.filter(s => s.enabled), [skills]);
  const memEn    = useMemo(() => memories.filter(m => m.enabled).length, [memories]);
  const rulesEn  = useMemo(() => rules.filter(r => r.enabled), [rules]);
  const avgImp   = useMemo(() => memEn > 0 ? memories.filter(m => m.enabled).reduce((s, m) => s + m.importance, 0) / memEn : 0, [memories, memEn]);
  const favCount = useMemo(() => templates.filter(t => t.isFavorite).length, [templates]);
  const radarDims = useMemo(() => [
    { label: t('radar.dims.creativity'),   value: Math.min(10, templates.length * 0.6 + skillEn.length * 0.5) },
    { label: t('radar.dims.rigor'),        value: Math.min(10, rulesEn.length * 1.5) },
    { label: t('radar.dims.autonomy'),     value: agenticConfig?.enabled
      ? Math.min(10, 4 + (agenticConfig.available_tools?.length ?? 0) * 0.25 + mcpServers.length * 0.5)
      : Math.min(10, enabledTools * 0.3 + healthyMcp * 0.8) },
    { label: t('radar.dims.memory'),       value: Math.min(10, memEn * 0.7 + avgImp * 0.3) },
    { label: t('radar.dims.expression'),   value: Math.min(10, templates.length * 0.5 + favCount * 1.2) },
    { label: t('radar.dims.adaptability'), value: Math.min(10, skillEn.length * 1.2 + mcpServers.length * 0.8) },
  ], [templates, skillEn, rulesEn, agenticConfig, mcpServers, enabledTools, healthyMcp, memEn, avgImp, favCount, t]);

  // model slot current IDs (with fallbacks)
  const slotIds: Record<ModelSlotKey, string> = useMemo(() => ({
    primary:     funcAgentModels['primary']     ?? 'primary',
    fast:        funcAgentModels['fast']        ?? 'fast',
    compression: funcAgentModels['compression'] ?? 'fast',
    image:       funcAgentModels['image']       ?? '',
    voice:       funcAgentModels['voice']       ?? '',
    retrieval:   funcAgentModels['retrieval']   ?? '',
  }), [funcAgentModels]);

  // Tool KPI text
  const toolKpi = useMemo(() => {
    if (mcpServers.length > 0) {
      return t('kpi.toolStatsMcp', {
        enabled: enabledTools,
        total: availableTools.length,
        mcpHealthy: healthyMcp,
        mcpTotal: mcpServers.length,
      });
    }
    return t('kpi.toolStats', { enabled: enabledTools, total: availableTools.length });
  }, [t, enabledTools, availableTools.length, healthyMcp, mcpServers.length]);

  // Preference items — computed inside render to use t()
  const prefItems = useMemo(() => [
    {
      key: 'enable_visual_mode' as keyof AIExperienceConfig,
      label: t('prefs.visualMode'),
      desc:  t('prefs.visualModeDesc'),
    },
    {
      key: 'enable_session_title_generation' as keyof AIExperienceConfig,
      label: t('prefs.sessionTitle'),
      desc:  t('prefs.sessionTitleDesc'),
    },
    {
      key: 'enable_welcome_panel_ai_analysis' as keyof AIExperienceConfig,
      label: t('prefs.welcomeAnalysis'),
      desc:  t('prefs.welcomeAnalysisDesc'),
    },
  ], [t]);

  return (
    <div className={C}>

      <header className={`${C}-hero`}>
        <div className={`${C}-hero__left`}>
          <div className={`${C}-hero__avatar`}>
            <Bot size={56} strokeWidth={1.3} />
          </div>

          <div className={`${C}-hero__info`}>
            <div className={`${C}-hero__name-row`}>
              {editingField === 'name' ? (
                <Input
                  ref={nameInputRef}
                  className={`${C}-hero__name-input`}
                  value={editValue}
                  onChange={e => setEditValue(e.target.value)}
                  onBlur={commitEdit}
                  onKeyDown={onEditKey}
                  inputSize="small"
                />
              ) : (
                <h1
                  className={`${C}-hero__name`}
                  onClick={() => startEdit('name')}
                  title={t('hero.editNameTitle')}
                >
                  {identity.name}
                  <Pencil size={11} className={`${C}-hero__name-edit`} strokeWidth={1.6} />
                </h1>
              )}
              <span className={`${C}-hero__badge`}>Super Agent</span>
            </div>

            {editingField === 'desc' ? (
              <Input
                ref={descInputRef}
                className={`${C}-hero__desc-input`}
                value={editValue}
                onChange={e => setEditValue(e.target.value)}
                onBlur={commitEdit}
                onKeyDown={onEditKey}
                placeholder={t('hero.descPlaceholder')}
                inputSize="small"
              />
            ) : (
              <p
                className={`${C}-hero__desc`}
                onClick={() => startEdit('desc')}
                title={t('hero.editDescTitle')}
              >
                {identity.desc || t('defaultDesc')}
              </p>
            )}
          </div>
        </div>

        <div className={`${C}-hero__radar`} title={t('hero.radarTitle')}>
          <PersonaRadar dims={radarDims} size={140} onDimClick={handleRadarDimClick} onChartClick={openRadar} />
        </div>
      </header>

      <section className={`${C}-section`}>
        <h2 className={`${C}-section__title`}>{t('sections.brain')}</h2>

        <div className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.model')}</span>
            <button type="button" className={`${C}-link`} onClick={() => navToSettings('models')}>
              {t('actions.globalManage')} <ChevronRight size={11} />
            </button>
          </div>
          <div className={`${C}-model-grid`}>
            {MODEL_SLOT_KEYS.map(key => (
              <ModelPill
                key={key}
                slotKey={key}
                slotLabel={t(`modelSlots.${key}.label`)}
                slotDesc={t(`modelSlots.${key}.desc`)}
                currentId={slotIds[key]}
                models={models}
                defaultModels={defaultModels}
                onChange={id => handleModelChange(key, id)}
              />
            ))}
          </div>
        </div>

        <div ref={rulesRef} className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.rules')}</span>
            <span className={`${C}-card__kpi`}>
              {t('kpi.rules', { user: userRules, project: projRules, enabled: enabledRules })}
            </span>
            <button type="button" className={`${C}-link`} onClick={() => navToSettings('ai-context')}>
              {t('actions.manage')} <ChevronRight size={11} />
            </button>
          </div>
          <div className={`${C}-chip-row`}>
            {(rules.length > CHIP_LIMIT && !rulesExpanded ? sortRules.slice(0, CHIP_LIMIT) : sortRules).map(r => (
              <ToggleChip
                key={`${r.level}-${r.name}`}
                label={r.name}
                enabled={r.enabled}
                onToggle={() => toggleRule(r)}
                accentColor="#60a5fa"
                loading={rulesLoading[`${r.level}-${r.name}`]}
              />
            ))}
            {sortRules.length === 0 && <span className={`${C}-empty-hint`}>{t('empty.rules')}</span>}
            {rules.length > CHIP_LIMIT && (
              <button type="button" className={`${C}-chip ${C}-chip--more`} onClick={() => setRulesExpanded(v => !v)}>
                {rulesExpanded ? t('actions.collapse') : `+${rules.length - CHIP_LIMIT}`}
              </button>
            )}
          </div>
        </div>

        <div ref={memoryRef} className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.memory')}</span>
            <span className={`${C}-card__kpi`}>{t('kpi.memory', { count: enabledMems })}</span>
            <button type="button" className={`${C}-link`} onClick={() => navToSettings('ai-context')}>
              {t('actions.manage')} <ChevronRight size={11} />
            </button>
          </div>
          <div className={`${C}-chip-row`}>
            {(memories.length > CHIP_LIMIT && !memoriesExpanded ? sortMem.slice(0, CHIP_LIMIT) : sortMem).map(m => (
              <ToggleChip
                key={m.id}
                label={m.title}
                enabled={m.enabled}
                onToggle={() => toggleMem(m)}
                accentColor="#c9944d"
                loading={memoriesLoading[m.id]}
                tooltip={m.title}
              />
            ))}
            {sortMem.length === 0 && <span className={`${C}-empty-hint`}>{t('empty.memory')}</span>}
            {memories.length > CHIP_LIMIT && (
              <button type="button" className={`${C}-chip ${C}-chip--more`} onClick={() => setMemoriesExpanded(v => !v)}>
                {memoriesExpanded ? t('actions.collapse') : `+${memories.length - CHIP_LIMIT}`}
              </button>
            )}
          </div>
        </div>
      </section>

      <section className={`${C}-section`}>
        <h2 className={`${C}-section__title`}>{t('sections.capabilities')}</h2>

        <div ref={toolsRef} className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.toolsMcp')}</span>
            <span className={`${C}-card__kpi`}>{toolKpi}</span>
            <div className={`${C}-card__actions`}>
              <button type="button" className={`${C}-icon-btn`} onClick={selectAllTools} title={t('actions.selectAll')}>
                <ListChecks size={13} strokeWidth={1.8} />
              </button>
              <button type="button" className={`${C}-icon-btn`} onClick={clearAllTools} title={t('actions.clearAll')}>
                <X size={13} strokeWidth={1.8} />
              </button>
              <button type="button" className={`${C}-icon-btn`} onClick={resetTools} title={t('actions.reset')}>
                <RotateCcw size={13} strokeWidth={1.8} />
              </button>
            </div>
          </div>
          <div className={`${C}-chip-row`}>
            {sortTools.map(tool => (
              <ToggleChip
                key={tool.name}
                label={tool.name}
                enabled={agenticConfig?.available_tools?.includes(tool.name) ?? false}
                onToggle={() => toggleTool(tool.name)}
                accentColor="#6eb88c"
                loading={toolsLoading[tool.name]}
                tooltip={tool.description || tool.name}
              />
            ))}
            {availableTools.length === 0 && <span className={`${C}-empty-hint`}>{t('empty.tools')}</span>}
          </div>

          {mcpServers.length > 0 && (
            <div className={`${C}-mcp-row`}>
              <span className={`${C}-mcp-row__label`}>MCP</span>
              {mcpServers.map(srv => {
                const ok = srv.status === 'Healthy' || srv.status === 'Connected';
                return (
                  <span key={srv.id} className={`${C}-mcp-tag ${ok ? 'is-ok' : 'is-err'}`}>
                    <span className={`${C}-mcp-tag__dot`} />
                    {srv.name}
                  </span>
                );
              })}
              <button type="button" className={`${C}-link`} onClick={() => navToSettings('mcp')}>
                {t('actions.manage')} <ChevronRight size={11} />
              </button>
            </div>
          )}
        </div>

        <div ref={skillsRef} className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.skills')}</span>
            <span className={`${C}-card__kpi`}>{t('kpi.skills', { count: enabledSkls })}</span>
            <button type="button" className={`${C}-link`} onClick={() => navToSettings('skills')}>
              {t('actions.manage')} <ChevronRight size={11} />
            </button>
          </div>
          <div className={`${C}-chip-row`}>
            {(skills.length > CHIP_LIMIT && !skillsExpanded ? sortSkills.slice(0, CHIP_LIMIT) : sortSkills).map(sk => (
              <ToggleChip
                key={sk.name}
                label={sk.name}
                enabled={sk.enabled}
                onToggle={() => toggleSkill(sk)}
                accentColor="#8b5cf6"
                loading={skillsLoading[sk.name]}
                tooltip={sk.description}
              />
            ))}
            {sortSkills.length === 0 && <span className={`${C}-empty-hint`}>{t('empty.skills')}</span>}
            {skills.length > CHIP_LIMIT && (
              <button type="button" className={`${C}-chip ${C}-chip--more`} onClick={() => setSkillsExpanded(v => !v)}>
                {skillsExpanded ? t('actions.collapse') : `+${skills.length - CHIP_LIMIT}`}
              </button>
            )}
          </div>
        </div>
      </section>

      <section className={`${C}-section`} ref={interactionRef as React.RefObject<HTMLElement>}>
        <h2 className={`${C}-section__title`}>{t('sections.interaction')}</h2>

        <div ref={templatesRef} className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.templates')}</span>
            <span className={`${C}-card__kpi`}>{t('kpi.templateCount', { count: templates.length })}</span>
            <button type="button" className={`${C}-link`} onClick={() => navToSettings('prompt-templates')}>
              {t('actions.manage')} <ChevronRight size={11} />
            </button>
          </div>
          <div className={`${C}-chip-row`}>
            {sortTemplates.slice(0, 14).map(tmpl => (
              <span key={tmpl.id} className={`${C}-tpl-chip ${tmpl.isFavorite ? 'is-fav' : ''}`}>
                {tmpl.isFavorite && '★ '}{tmpl.name}
              </span>
            ))}
            {templates.length === 0 && <span className={`${C}-empty-hint`}>{t('empty.templates')}</span>}
          </div>
        </div>

        <div className={`${C}-card`}>
          <div className={`${C}-card__head`}>
            <span className={`${C}-card__label`}>{t('cards.preferences')}</span>
          </div>
          <div className={`${C}-chip-row`}>
            {prefItems.map(({ key, label, desc }) => (
              <ToggleChip
                key={key}
                label={label}
                enabled={!!aiExp[key]}
                onToggle={() => togglePref(key)}
                tooltip={`${label}：${desc}`}
                accentColor="#7096c4"
              />
            ))}
          </div>
        </div>
      </section>

      {radarOpen && createPortal(
        <div
          className={`${C}-modal${radarClosing ? ' is-closing' : ''}`}
          onClick={closeRadar}
        >
          <div className={`${C}-modal__box`} onClick={e => e.stopPropagation()}>
            <div className={`${C}-modal__head`}>
              <div>
                <p className={`${C}-modal__title`}>{t('radar.title')}</p>
                <p className={`${C}-modal__sub`}>{t('radar.subtitle')}</p>
              </div>
              <button className={`${C}-modal__close`} onClick={closeRadar}>
                <X size={15} strokeWidth={1.8} />
              </button>
            </div>
            <div className={`${C}-modal__radar`}>
              <PersonaRadar dims={radarDims} size={280} onDimClick={handleRadarDimClick} />
            </div>
            <div className={`${C}-modal__dims`}>
              {radarDims.map(d => (
                <div key={d.label} className={`${C}-modal__dim`}>
                  <span className={`${C}-modal__dim-label`}>{d.label}</span>
                  <div className={`${C}-modal__dim-track`}>
                    <div className={`${C}-modal__dim-fill`} style={{ width: `${d.value * 10}%` }} />
                  </div>
                  <span className={`${C}-modal__dim-val`}>{d.value.toFixed(1)}</span>
                </div>
              ))}
            </div>
          </div>
        </div>,
        document.body,
      )}
    </div>
  );
};

export default PersonaView;
