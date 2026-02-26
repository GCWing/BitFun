/**
 * SettingsScene — content-only renderer for the Settings scene.
 *
 * The left-side navigation lives in SettingsNav (rendered by NavPanel via
 * nav-registry). This component only renders the active config content panel
 * driven by settingsStore.activeTab.
 */

import React, { lazy, Suspense } from 'react';
import { useSettingsStore } from './settingsStore';
import './SettingsScene.scss';

const AIModelConfig        = lazy(() => import('../../../infrastructure/config/components/AIModelConfig'));
const AIFeaturesConfig     = lazy(() => import('../../../infrastructure/config/components/AIFeaturesConfig'));
const AIRulesMemoryConfig  = lazy(() => import('../../../infrastructure/config/components/AIRulesMemoryConfig'));
const AgentsConfig         = lazy(() => import('../../../infrastructure/config/components/AgentsConfig'));
const SkillsConfig         = lazy(() => import('../../../infrastructure/config/components/SkillsConfig'));
const McpToolsConfig       = lazy(() => import('../../../infrastructure/config/components/McpToolsConfig'));
const LspConfig            = lazy(() => import('../../../infrastructure/config/components/LspConfig'));
const DebugConfig          = lazy(() => import('../../../infrastructure/config/components/DebugConfig'));
const LoggingConfig        = lazy(() => import('../../../infrastructure/config/components/LoggingConfig'));
const TerminalConfig       = lazy(() => import('../../../infrastructure/config/components/TerminalConfig'));
const EditorConfig         = lazy(() => import('../../../infrastructure/config/components/EditorConfig'));
const ThemeConfigComponent = lazy(() => import('../../../infrastructure/config/components/ThemeConfig').then(m => ({ default: m.ThemeConfig })));
const PromptTemplateConfig = lazy(() => import('../../../infrastructure/config/components/PromptTemplateConfig'));

const SettingsScene: React.FC = () => {
  const activeTab = useSettingsStore(s => s.activeTab);

  let Content: React.LazyExoticComponent<React.ComponentType> | null = null;

  switch (activeTab) {
    case 'theme':            Content = ThemeConfigComponent; break;
    case 'models':           Content = AIModelConfig;        break;
    case 'ai-features':      Content = AIFeaturesConfig;     break;
    case 'ai-context':       Content = AIRulesMemoryConfig; break;
    case 'prompt-templates': Content = PromptTemplateConfig; break;
    case 'skills':           Content = SkillsConfig;         break;
    case 'mcp-tools':        Content = McpToolsConfig;      break;
    case 'agents':           Content = AgentsConfig;         break;
    case 'lsp':              Content = LspConfig;            break;
    case 'debug':            Content = DebugConfig;          break;
    case 'logging':          Content = LoggingConfig;        break;
    case 'terminal':         Content = TerminalConfig;       break;
    case 'editor':           Content = EditorConfig;         break;
  }

  return (
    <div className="bitfun-settings-scene">
      <Suspense fallback={<div className="bitfun-settings-scene__loading" />}>
        {Content && <Content />}
      </Suspense>
    </div>
  );
};

export default SettingsScene;
