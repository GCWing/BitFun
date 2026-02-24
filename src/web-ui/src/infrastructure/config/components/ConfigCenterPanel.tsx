import React, { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Select } from '@/component-library';
import AIModelConfig from './AIModelConfig';
import AgenticModeConfig from './AgenticModeConfig';
import AIFeaturesConfig from './AIFeaturesConfig';
import AIRulesConfig from './AIRulesConfig';
import SubAgentConfig from './SubAgentConfig';
import SkillsConfig from './SkillsConfig';
import SkillMarketConfig from './SkillMarketConfig';
import MCPConfig from './MCPConfig';
import IntegrationsConfig from './IntegrationsConfig';
import AgenticToolsConfig from './AgenticToolsConfig';
import AIMemoryConfig from './AIMemoryConfig';
import LspConfig from './LspConfig';
import DebugConfig from './DebugConfig';
import LoggingConfig from './LoggingConfig';
import TerminalConfig from './TerminalConfig';
import EditorConfig from './EditorConfig';
import { ThemeConfig } from './ThemeConfig';
import PromptTemplateConfig from './PromptTemplateConfig';
import ModeConfig from './ModeConfig';
import './ConfigCenter.scss';

 

export interface ConfigCenterPanelProps {
  initialTab?: 'models' | 'ai-rules' | 'agents' | 'mcp' | 'agentic-tools' | 'logging' | 'skill-market';
}

type ConfigTab =
  | 'models'
  | 'super-agent'
  | 'ai-features'
  | 'modes'
  | 'ai-rules'
  | 'agents'
  | 'skills'
  | 'skill-market'
  | 'integrations'
  | 'mcp'
  | 'agentic-tools'
  | 'ai-memory'
  | 'lsp'
  | 'debug'
  | 'logging'
  | 'terminal'
  | 'editor'
  | 'theme'
  | 'prompt-templates';

interface TabCategory {
  name: string;
  tabs: Array<{
    id: ConfigTab;
    label: string;
    badge?: React.ReactNode;
  }>;
}

const ConfigCenterPanel: React.FC<ConfigCenterPanelProps> = ({ 
  initialTab = 'models' 
}) => {
  const { t } = useTranslation('settings');
  const [activeTab, setActiveTab] = useState<ConfigTab>(initialTab);
  const [expandedCategories, setExpandedCategories] = useState<Set<number>>(
    new Set([0, 1, 2, 3, 4, 5]) 
  );

  const toggleCategory = (index: number) => {
    setExpandedCategories(prev => {
      const newSet = new Set(prev);
      if (newSet.has(index)) {
        newSet.delete(index);
      } else {
        newSet.add(index);
      }
      return newSet;
    });
  };

  const categories: TabCategory[] = useMemo(() => [
    {
      name: t('configCenter.categories.general'),
      tabs: [
        {
          id: 'theme' as ConfigTab,
          label: t('configCenter.tabs.theme')
        },
        {
          id: 'models' as ConfigTab,
          label: t('configCenter.tabs.models')
        }
      ]
    },
    {
      name: t('configCenter.categories.agent'),
      tabs: [
        {
          id: 'super-agent' as ConfigTab,
          label: t('configCenter.tabs.superAgent')
        },
        {
          id: 'modes' as ConfigTab,
          label: t('configCenter.tabs.modes')
        },
        {
          id: 'agents' as ConfigTab,
          label: t('configCenter.tabs.agents')
        },
        {
          id: 'ai-features' as ConfigTab,
          label: t('configCenter.tabs.aiFeatures')
        },
        {
          id: 'agentic-tools' as ConfigTab,
          label: t('configCenter.tabs.agenticTools')
        }
      ]
    },
    {
      name: t('configCenter.categories.context'),
      tabs: [
        {
          id: 'ai-rules' as ConfigTab,
          label: t('configCenter.tabs.aiRules')
        },
        {
          id: 'ai-memory' as ConfigTab,
          label: t('configCenter.tabs.aiMemory')
        },
        {
          id: 'prompt-templates' as ConfigTab,
          label: t('configCenter.tabs.promptTemplates')
        }
      ]
    },
    {
      name: t('configCenter.categories.extensions'),
      tabs: [
        {
          id: 'skills' as ConfigTab,
          label: t('configCenter.tabs.skills')
        },
        {
          id: 'skill-market' as ConfigTab,
          label: t('configCenter.tabs.skillMarket')
        },
        {
          id: 'integrations' as ConfigTab,
          label: t('configCenter.tabs.integrations')
        },
        {
          id: 'mcp' as ConfigTab,
          label: t('configCenter.tabs.mcp')
        }
      ]
    },
    {
      name: t('configCenter.categories.devkit'),
      tabs: [
        {
          id: 'editor' as ConfigTab,
          label: t('configCenter.tabs.editor')
        },
        {
          id: 'lsp' as ConfigTab,
          label: t('configCenter.tabs.lsp')
        },
        {
          id: 'debug' as ConfigTab,
          label: t('configCenter.tabs.debug')
        },
        {
          id: 'terminal' as ConfigTab,
          label: t('configCenter.tabs.terminal')
        },
        {
          id: 'logging' as ConfigTab,
          label: t('configCenter.tabs.logging')
        }
      ]
    }
  ], [t]);

  
  const selectOptions = useMemo(() => {
    const options: Array<{ value: string; label: string; group?: string }> = [];
    categories.forEach(category => {
      category.tabs.forEach(tab => {
        options.push({
          value: tab.id,
          label: tab.label,
          group: category.name
        });
      });
    });
    return options;
  }, [categories]);

  const renderTabContent = () => {
    switch (activeTab) {
      case 'theme':
        return <ThemeConfig />;
      case 'models':
        return <AIModelConfig />;
      case 'super-agent':
        return <AgenticModeConfig />;
      case 'modes':
        return <ModeConfig />;
      case 'ai-features':
        return <AIFeaturesConfig />;
      case 'agentic-tools':
        return <AgenticToolsConfig />;
      case 'ai-rules':
        return <AIRulesConfig />;
      case 'ai-memory':
        return <AIMemoryConfig />;
      case 'prompt-templates':
        return <PromptTemplateConfig />;
      case 'skills':
        return <SkillsConfig />;
      case 'skill-market':
        return <SkillMarketConfig />;
      case 'agents':
        return <SubAgentConfig />;
      case 'mcp':
        return <MCPConfig />;
      case 'integrations':
        return <IntegrationsConfig />;
      case 'lsp':
        return <LspConfig />;
      case 'debug':
        return <DebugConfig />;
      case 'logging':
        return <LoggingConfig />;
      case 'terminal':
        return <TerminalConfig />;
      case 'editor':
        return <EditorConfig />;
      default:
        return null;
    }
  };

  return (
    <div className="config-center-panel-wrapper">
      
      <div className="config-compact-selector">
        <Select
          className="config-compact-selector__select"
          value={activeTab}
          onChange={(value) => setActiveTab(value as ConfigTab)}
          options={selectOptions}
          placeholder={t('configCenter.selectConfig')}
          size="small"
        />
      </div>
      
      <div className="config-center-body">
        
        <div className="config-tabs">
          {categories.map((category, categoryIndex) => {
            const isExpanded = expandedCategories.has(categoryIndex);
            return (
              <div key={categoryIndex} className="tab-category">
                <div 
                  className="tab-category-title"
                  onClick={() => toggleCategory(categoryIndex)}
                >
                  {category.name}
                </div>
                {isExpanded ? (
                  <div className="tab-category-items">
                    {category.tabs.map(tab => (
                      <button
                        key={tab.id}
                        className={`tab-button ${activeTab === tab.id ? 'active' : ''}`}
                        onClick={() => setActiveTab(tab.id)}
                      >
                        <div className="tab-label">
                          {tab.label}
                          {tab.badge && <span className="tab-badge">{tab.badge}</span>}
                        </div>
                      </button>
                    ))}
                  </div>
                ) : (
                  <div 
                    className="tab-category-collapsed"
                    onClick={() => toggleCategory(categoryIndex)}
                  >······</div>
                )}
              </div>
            );
          })}
        </div>

        
        <div className="config-content">
          {renderTabContent()}
        </div>
      </div>
    </div>
  );
};

export default ConfigCenterPanel;
