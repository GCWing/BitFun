/* Component registry */
import React from 'react';
import type { ComponentCategory } from '../types';
import { Button } from '@components/Button';
import { IconButton } from '@components/IconButton';
import { WindowControls } from '@components/WindowControls';
import { Input } from '@components/Input';
import { Search } from '@components/Search';
import { Select } from '@components/Select';
import { Checkbox } from '@components/Checkbox';
import { Switch } from '@components/Switch';
import { Textarea } from '@components/Textarea';
import { Modal } from '@components/Modal';
import { CubeLoading } from '@components/CubeLoading';
import { Alert } from '@components/Alert';
import { Tooltip } from '@components/Tooltip';
import { Tabs, TabPane } from '@components/Tabs';
import { Tag } from '@components/Tag';
import { Avatar, AvatarGroup } from '@components/Avatar';
import { Empty } from '@components/Empty';
import { Markdown } from '@components/Markdown';
import { CodeEditor } from '@components/CodeEditor';
import { StreamText } from '@components/StreamText';
import { TodoWriteDisplay } from '@/flow_chat/tool-cards/TodoWriteDisplay';
import { TaskToolDisplay } from '@/flow_chat/tool-cards/TaskToolDisplay';
import { WebSearchCard as RealWebSearchCard } from '@/flow_chat/tool-cards/WebSearchCard';
import { ReadFileDisplay } from '@/flow_chat/tool-cards/ReadFileDisplay';
import { GrepSearchDisplay } from '@/flow_chat/tool-cards/GrepSearchDisplay';
import { GlobSearchDisplay } from '@/flow_chat/tool-cards/GlobSearchDisplay';
import { FileOperationToolCard } from '@/flow_chat/tool-cards/FileOperationToolCard';
import { LSDisplay } from '@/flow_chat/tool-cards/LSDisplay';
import { MCPToolDisplay } from '@/flow_chat/tool-cards/MCPToolDisplay';
import { ContextCompressionDisplay } from '@/flow_chat/tool-cards/ContextCompressionDisplay';
import { SkillDisplay } from '@/flow_chat/tool-cards/SkillDisplay';
import { AskUserQuestionCard } from '@/flow_chat/tool-cards/AskUserQuestionCard';
import { CreatePlanDisplay } from '@/flow_chat/tool-cards/CreatePlanDisplay';
import { InitLiveAppDisplay } from '@/flow_chat/tool-cards/InitLiveAppToolDisplay';
import type { FlowToolItem, FlowThinkingItem } from '@/flow_chat/types/flow-chat';
import { TOOL_CARD_CONFIGS } from '@/flow_chat/tool-cards';
import { ModelThinkingDisplay } from '@/flow_chat/tool-cards/ModelThinkingDisplay';
import { ReproductionStepsBlock } from '@components/Markdown/ReproductionStepsBlock';

function createMockToolItem(
  toolName: string,
  input: any,
  result?: any,
  status: 'pending' | 'preparing' | 'streaming' | 'running' | 'completed' | 'error' = 'completed'
): FlowToolItem {
  const config = TOOL_CARD_CONFIGS[toolName];
  return {
    id: `mock-${toolName}-${Date.now()}`,
    type: 'tool',
    status,
    timestamp: Date.now(),
    toolName,
    toolCall: {
      id: `call-${toolName}`,
      input
    },
    toolResult: result ? {
      result,
      success: status === 'completed',
      error: status === 'error' ? '执行失败' : undefined
    } : undefined,
    config: config || {
      toolName,
      displayName: toolName,
      icon: '🔧',
      requiresConfirmation: false,
      resultDisplayType: 'summary',
      description: '',
      displayMode: 'compact',
      primaryColor: '#6b7280'
    }
  } as FlowToolItem;
}


export const componentRegistry: ComponentCategory[] = [
  {
    id: 'basic',
    name: '基础组件',
    description: '常用的基础UI组件',
    layoutType: 'grid-4',
    components: [
      {
        id: 'button-primary',
        name: 'Button - Primary',
        description: '主要按钮',
        category: 'basic',
        component: () => <Button variant="primary">Primary Button</Button>,
      },
      {
        id: 'button-secondary',
        name: 'Button - Secondary',
        description: '次要按钮',
        category: 'basic',
        component: () => <Button variant="secondary">Secondary Button</Button>,
      },
      {
        id: 'button-ghost',
        name: 'Button - Ghost',
        description: '幽灵按钮',
        category: 'basic',
        component: () => <Button variant="ghost">Ghost Button</Button>,
      },
      {
        id: 'button-sizes',
        name: 'Button - Sizes',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
            <Button size="small">Small</Button>
            <Button size="medium">Medium</Button>
            <Button size="large">Large</Button>
          </div>
        ),
      },
      {
        id: 'tag-demo',
        name: 'Tag - 演示',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
              <Tag color="blue">Blue</Tag>
              <Tag color="green">Green</Tag>
              <Tag color="red">Red</Tag>
              <Tag color="yellow">Yellow</Tag>
              <Tag color="purple">Purple</Tag>
              <Tag color="gray">Gray</Tag>
            </div>
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
              <Tag size="small">Small</Tag>
              <Tag size="medium">Medium</Tag>
              <Tag size="large">Large</Tag>
            </div>
            <div style={{ display: 'flex', gap: '8px' }}>
              <Tag color="blue" rounded>Rounded</Tag>
              <Tag color="green" closable onClose={() => alert('Closed!')}>Closable</Tag>
            </div>
          </div>
        ),
      },
      {
        id: 'icon-button-variants',
        name: 'IconButton - 变体',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center', flexWrap: 'wrap' }}>
            <IconButton variant="default" aria-label="Search">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="2"/>
                <path d="M11 11L15 15" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </IconButton>
            <IconButton variant="primary" aria-label="Star">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M8 2L10 6L14 6.5L11 9.5L12 14L8 11.5L4 14L5 9.5L2 6.5L6 6L8 2Z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
              </svg>
            </IconButton>
            <IconButton variant="ghost" aria-label="Settings">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="2"/>
                <path d="M8 1V3M8 13V15M15 8H13M3 8H1M13.5 2.5L12 4M4 12L2.5 13.5M13.5 13.5L12 12M4 4L2.5 2.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </IconButton>
            <IconButton variant="danger" aria-label="Delete">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M3 4H13M5 4V3C5 2.5 5.5 2 6 2H10C10.5 2 11 2.5 11 3V4M6 7V12M10 7V12M4 4L5 14H11L12 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </IconButton>
            <IconButton variant="success" aria-label="Check">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M3 8L6 11L13 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </IconButton>
            <IconButton variant="warning" aria-label="Warning">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M8 2L14 14H2L8 2Z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
                <path d="M8 6V9M8 11V11.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </IconButton>
          </div>
        ),
      },
      {
        id: 'icon-button-sizes',
        name: 'IconButton - 尺寸',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
            <IconButton size="small" variant="primary" aria-label="Small">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path d="M8 2L10 6L14 6.5L11 9.5L12 14L8 11.5L4 14L5 9.5L2 6.5L6 6L8 2Z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
              </svg>
            </IconButton>
            <IconButton size="medium" variant="primary" aria-label="Medium">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M8 2L10 6L14 6.5L11 9.5L12 14L8 11.5L4 14L5 9.5L2 6.5L6 6L8 2Z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
              </svg>
            </IconButton>
            <IconButton size="large" variant="primary" aria-label="Large">
              <svg width="20" height="20" viewBox="0 0 16 16" fill="none">
                <path d="M8 2L10 6L14 6.5L11 9.5L12 14L8 11.5L4 14L5 9.5L2 6.5L6 6L8 2Z" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
              </svg>
            </IconButton>
          </div>
        ),
      },
      {
        id: 'icon-button-shapes',
        name: 'IconButton - 形状',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
            <IconButton shape="square" variant="primary" aria-label="Square">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <rect x="3" y="3" width="10" height="10" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </IconButton>
            <IconButton shape="circle" variant="primary" aria-label="Circle">
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <circle cx="8" cy="8" r="5" stroke="currentColor" strokeWidth="2"/>
              </svg>
            </IconButton>
          </div>
        ),
      },
      {
        id: 'window-controls-demo',
        name: 'WindowControls - 窗口控件',
        description: 'Demo',
        category: 'basic',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
            <div>
              <WindowControls
                onMinimize={() => {}}
                onMaximize={() => {}}
                onClose={() => {}}
              />
            </div>
            <div>
              <WindowControls
                showMinimize={false}
                onMaximize={() => {}}
                onClose={() => {}}
              />
            </div>
            <div>
              <WindowControls
                showMaximize={false}
                onMinimize={() => {}}
                onClose={() => {}}
              />
            </div>
          </div>
        ),
      },
    ],
  },
  {
    id: 'feedback',
    name: '反馈组件',
    description: 'Demo',
    layoutType: 'demo',
    components: [
      {
        id: 'cube-loading-variants',
        name: 'CubeLoading - 所有变体',
        description: '3x3x3 立方体加载动画展示',
        category: 'feedback',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '32px', padding: '20px' }}>
            {}
            <div>
              <div style={{ fontSize: '12px', color: '#666', marginBottom: '16px', fontWeight: 500 }}>尺寸</div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '48px', alignItems: 'flex-end' }}>
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '12px' }}>
                  <CubeLoading size="small" />
                  <span style={{ fontSize: '12px', color: '#999' }}>Small</span>
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '12px' }}>
                  <CubeLoading size="medium" />
                  <span style={{ fontSize: '12px', color: '#999' }}>Medium</span>
                </div>
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '12px' }}>
                  <CubeLoading size="large" />
                  <span style={{ fontSize: '12px', color: '#999' }}>Large</span>
                </div>
              </div>
            </div>
            {}
            <div>
              <div style={{ fontSize: '12px', color: '#666', marginBottom: '16px', fontWeight: 500 }}>With text</div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '48px', alignItems: 'flex-start' }}>
                <CubeLoading text="加载中.." />
                <CubeLoading size="large" text="加载中.." />
              </div>
            </div>
          </div>
        ),
      },
      {
        id: 'modal-basic',
        name: 'Modal - Basic',
        description: '基础弹窗',
        category: 'feedback',
        component: () => {
          const [isOpen, setIsOpen] = React.useState(false);
          return (
            <>
              <Button onClick={() => setIsOpen(true)}>打开弹窗</Button>
              <Modal
                isOpen={isOpen}
                onClose={() => setIsOpen(false)}
                title="基础弹窗"
              >
                <div style={{ padding: '16px' }}>
                  <p>Modal body content</p>
                </div>
              </Modal>
            </>
          );
        },
      },
      {
        id: 'alert-demo',
        name: 'Alert - 四种类型',
        description: 'Demo',
        category: 'feedback',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <Alert type="success" title="Success" message="Operation completed" closable />
            <Alert type="error" title="Error" message="Something went wrong" closable />
            <Alert type="warning" message="Warning message" />
            <Alert type="info" message="Info message" showIcon />
          </div>
        ),
      },
      {
        id: 'stream-text-demo',
        name: 'StreamText - 流式文本演示',
        description: 'AI 流式文本打字机效果',
        category: 'feedback',
        component: () => {
          const [key, setKey] = React.useState(0);

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
              <div style={{
                fontSize: '15px',
                lineHeight: '1.8',
                minHeight: '120px',
                padding: '20px',
                background: 'rgba(255, 255, 255, 0.02)',
                borderRadius: '8px',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                maxWidth: '700px'
              }}>
                <StreamText
                  key={key}
                  text="Streaming AI demo text."
                  effect="smooth"
                  speed={30}
                  showCursor={true}
                />
              </div>
              <Button
                size="small"
                variant="secondary"
                onClick={() => setKey(prev => prev + 1)}
              >
                重新播放
              </Button>
            </div>
          );
        },
      },
    ],
  },
  {
    id: 'form',
    name: '表单组件',
    description: '输入类表单组件',
    layoutType: 'grid-2',
    components: [
      {
        id: 'input-demo',
name: 'Input - Demo',
        description: 'Demo',
        category: 'form',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', maxWidth: '400px' }}>
            <Input placeholder="Enter text" />
            <Input label="Label" placeholder="Placeholder" />
            <Input
              label="邮箱"
              type="email"
              placeholder="example@email.com"
              prefix="@"
            />
            <Input
              label="Password"
              type="password"
              placeholder="Enter password"
              error
              errorMessage="Error message"
            />
            <Input variant="filled" placeholder="Filled variant" />
            <Input variant="outlined" placeholder="Outlined variant" />
          </div>
        ),
      },
      {
        id: 'search-demo',
name: 'Search - Demo',
        description: 'Demo',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState('');
          const [loading, setLoading] = React.useState(false);
          const [searchOptions, setSearchOptions] = React.useState({
            caseSensitive: false,
            useRegex: false,
          });

          const handleSearch = (val: string) => {
            setLoading(true);
            setTimeout(() => {
              setLoading(false);
            }, 1500);
          };

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '20px', maxWidth: '500px' }}>
              <Search
                placeholder="搜索关键词.."
                onChange={(val) => setValue(val)}
              />
              <Search
                placeholder="Search"
                showSearchButton
                onSearch={handleSearch}
                loading={loading}
              />
              <Search
                placeholder="With suffix"
                suffixContent={
                  <div style={{ display: 'flex', gap: '4px' }}>
                    <button
                      style={{
                        padding: '4px 6px',
                        background: searchOptions.caseSensitive ? 'rgba(96, 165, 250, 0.2)' : 'transparent',
                        border: '1px solid rgba(255, 255, 255, 0.1)',
                        borderRadius: '4px',
                        color: searchOptions.caseSensitive ? '#60a5fa' : '#a0a0a0',
                        cursor: 'pointer',
                        fontSize: '12px',
                      }}
                      onClick={() => setSearchOptions(prev => ({ ...prev, caseSensitive: !prev.caseSensitive }))}
                      title="Option"
                    >
                      Aa
                    </button>
                    <button
                      style={{
                        padding: '4px 6px',
                        background: searchOptions.useRegex ? 'rgba(96, 165, 250, 0.2)' : 'transparent',
                        border: '1px solid rgba(255, 255, 255, 0.1)',
                        borderRadius: '4px',
                        color: searchOptions.useRegex ? '#60a5fa' : '#a0a0a0',
                        cursor: 'pointer',
                        fontSize: '12px',
                      }}
                      onClick={() => setSearchOptions(prev => ({ ...prev, useRegex: !prev.useRegex }))}
                      title="Option"
                    >
                      .*
                    </button>
                  </div>
                }
              />
              <Search
                placeholder="Search..."
                expandOnFocus
              />
              <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
                <Search size="small" placeholder="Search" />
                <Search size="medium" placeholder="Search" />
                <Search size="large" placeholder="Search" />
              </div>
              <Search
                placeholder="Disabled"
                disabled
              />
              <Search
                placeholder="Error"
                error
                errorMessage="Error message"
              />
            </div>
          );
        },
      },
      {
        id: 'select-basic',
        name: 'Select - 基础选择',
        description: '基础单选和多选示例',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState<string | number>('');
          const [multiValue, setMultiValue] = React.useState<(string | number)[]>([]);

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '20px', maxWidth: '400px' }}>
              <Select
                label="Select"
                options={[
                  { label: 'Option 1', value: '1' },
                  { label: 'Option 2', value: '2' },
                  { label: 'Option 3', value: '3' },
                  { label: 'Option 4', value: '4', disabled: true },
                ]}
                placeholder="Select..."
                value={value}
                onChange={(v) => setValue(v as string | number)}
                clearable
              />

              <Select
                label="Multiple"
                multiple
                showSelectAll
                options={[
                  { label: 'React', value: 'react' },
                  { label: 'Vue', value: 'vue' },
                  { label: 'Angular', value: 'angular' },
                  { label: 'Svelte', value: 'svelte' },
                  { label: 'Solid', value: 'solid' },
                ]}
                placeholder="选择技术"
                value={multiValue}
                onChange={(v) => setMultiValue(v as (string | number)[])}
                clearable
              />

              <div style={{ display: 'flex', gap: '12px', flexDirection: 'column' }}>
                <Select
                  size="small"
                  options={[
                    { label: 'Small', value: 's1' },
                    { label: 'Option 2', value: 's2' },
                  ]}
                  placeholder="Small size"
                />
                <Select
                  size="large"
                  options={[
                    { label: 'Large', value: 'l1' },
                    { label: 'Option 2', value: 'l2' },
                  ]}
                  placeholder="Large size"
                />
              </div>
            </div>
          );
        },
      },
      {
        id: 'select-searchable',
        name: 'Select - Demo',
        description: '可搜索的选择器示例',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState<string | number>('');

          const countries = [
            { label: 'CN', value: 'cn', description: 'China' },
            { label: 'US', value: 'us', description: 'United States' },
            { label: 'JP', value: 'jp', description: 'Japan' },
            { label: 'UK', value: 'uk', description: 'United Kingdom' },
            { label: 'FR', value: 'fr', description: 'France' },
            { label: 'DE', value: 'de', description: 'Germany' },
            { label: 'CA', value: 'ca', description: 'Canada' },
            { label: 'AU', value: 'au', description: 'Australia' },
            { label: 'KR', value: 'kr', description: 'Korea' },
            { label: 'SG', value: 'sg', description: 'Singapore' },
          ];

          return (
            <div style={{ maxWidth: '400px' }}>
              <Select
                label="Country"
                searchable
                searchPlaceholder="Search..."
                options={countries}
                placeholder="Select..."
                value={value}
                onChange={(v) => setValue(v as string | number)}
                clearable
              />
            </div>
          );
        },
      },
      {
        id: 'select-grouped',
        name: 'Select - 分组选择',
        description: '带分组的选择器',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState<string | number>('');

          const options = [
            { label: 'React', value: 'react', group: 'Frontend' },
            { label: 'Vue', value: 'vue', group: 'Frontend' },
            { label: 'Angular', value: 'angular', group: 'Frontend' },
            { label: 'Node.js', value: 'nodejs', group: 'Backend' },
            { label: 'Deno', value: 'deno', group: 'Backend' },
            { label: 'Express', value: 'express', group: 'Backend' },
            { label: 'PostgreSQL', value: 'postgresql', group: 'Database' },
            { label: 'MongoDB', value: 'mongodb', group: 'Database' },
            { label: 'Redis', value: 'redis', group: 'Database' },
          ];

          return (
            <div style={{ maxWidth: '400px' }}>
              <Select
                label="选择框架"
                searchable
                options={options}
                placeholder="选择..."
                value={value}
                onChange={(v) => setValue(v as string | number)}
                clearable
              />
            </div>
          );
        },
      },
      {
        id: 'select-with-icons',
        name: 'Select - Demo',
        description: '带图标的选择器',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState<string | number>('');

          const options = [
            {
              label: 'TypeScript',
              value: 'ts',
              description: 'TypeScript language',
              icon: <span style={{ fontSize: '18px' }}>TS</span>
            },
            {
              label: 'JavaScript',
              value: 'js',
              description: 'JavaScript language',
              icon: <span style={{ fontSize: '18px' }}>JS</span>
            },
            {
              label: 'Python',
              value: 'py',
              description: 'Python language',
              icon: <span style={{ fontSize: '18px' }}>PY</span>
            },
            {
              label: 'Rust',
              value: 'rs',
              description: 'Rust language',
              icon: <span style={{ fontSize: '18px' }}>RS</span>
            },
            {
              label: 'Go',
              value: 'go',
              description: 'Go language',
              icon: <span style={{ fontSize: '18px' }}>GO</span>
            },
          ];

          return (
            <div style={{ maxWidth: '400px' }}>
              <Select
                label="Language"
                searchable
                options={options}
                placeholder="Select..."
                value={value}
                onChange={(v) => setValue(v as string | number)}
                clearable
              />
            </div>
          );
        },
      },
      {
        id: 'select-advanced',
        name: 'Select - Demo',
        description: '加载、错误和禁用状态',
        category: 'form',
        component: () => {
          const [value1, setValue1] = React.useState<string | number>('');
          const [value2, setValue2] = React.useState<string | number>('');

          const options = [
            { label: 'Option 1', value: '1' },
            { label: 'Option 2', value: '2' },
            { label: 'Option 3', value: '3' },
          ];

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '20px', maxWidth: '400px' }}>
              <Select
                label="Loading"
                loading
                options={options}
                placeholder="Loading..."
                value={value1}
                onChange={(v) => setValue1(v as string | number)}
              />

              <Select
                label="Error"
                error
                errorMessage="Error message"
                options={options}
                placeholder="Error"
                value={value2}
                onChange={(v) => setValue2(v as string | number)}
              />

              <Select
                label="Disabled"
                disabled
                options={options}
                placeholder="Placeholder"
              />
            </div>
          );
        },
      },
      {
        id: 'checkbox-demo',
        name: 'Checkbox - Demo',
        description: '复选框演示',
        category: 'form',
        component: () => {
          const [checked, setChecked] = React.useState(false);
          const [indeterminate, setIndeterminate] = React.useState(true);

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <Checkbox label="Option" />
              <Checkbox
                label="Option"
                description="Description"
                checked={checked}
                onChange={(e) => setChecked(e.target.checked)}
              />
              <Checkbox
                label="Indeterminate"
                indeterminate={indeterminate}
                onChange={() => setIndeterminate(false)}
              />
              <Checkbox label="Option" disabled />
              <Checkbox label="Option" error />
              <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
                <Checkbox size="small" label="Small" />
                <Checkbox size="medium" label="Medium" />
                <Checkbox size="large" label="Large" />
              </div>
            </div>
          );
        },
      },
      {
        id: 'switch-demo',
        name: 'Switch - Demo',
        description: 'Demo',
        category: 'form',
        component: () => {
          const [checked, setChecked] = React.useState(false);
          const [loading, setLoading] = React.useState(false);

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <Switch label="Option" />
              <Switch
                label="Option"
                description="Description"
                checked={checked}
                onChange={(e) => setChecked(e.target.checked)}
              />
              <Switch
                label="Loading"
                loading={loading}
                checked={loading}
                onChange={(e) => {
                  setLoading(true);
                  setTimeout(() => setLoading(false), 2000);
                }}
              />
              <Switch label="Option" disabled />
              <Switch
                checkedText="ON"
                uncheckedText="OFF"
                label="With labels"
              />
              <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
                <Switch size="small" />
                <Switch size="medium" />
                <Switch size="large" />
              </div>
            </div>
          );
        },
      },
      {
        id: 'textarea-demo',
        name: 'Textarea - Demo',
        description: 'Demo',
        category: 'form',
        component: () => {
          const [value, setValue] = React.useState('');

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', maxWidth: '500px' }}>
              <Textarea
                label="Label"
                placeholder="Placeholder..."
              />
              <Textarea
                label="字数限制"
                placeholder="最多100字符.."
                showCount
                maxLength={100}
                value={value}
                onChange={(e) => setValue(e.target.value)}
              />
              <Textarea
                label="自动调整高度"
                placeholder="内容会自动调整高度..."
                autoResize
              />
              <Textarea
                label="Error"
                error
                errorMessage="请输入有效内容"
                placeholder="输入内容.."
              />
              <Textarea
                variant="filled"
                placeholder="Filled variant"
              />
              <Textarea
                variant="outlined"
                placeholder="Outlined variant"
              />
            </div>
          );
        },
      },
    ],
  },
  {
    id: 'content',
    name: '内容组件',
    description: '展示内容和媒体的组件',
    layoutType: 'large-card',
    components: [
      {
        id: 'markdown-viewer',
        name: 'Markdown Demo',
description: 'Markdown with GFM support',
        category: 'content',
        component: () => (
          <Markdown
            content={`# Markdown 演示

这是一个**Markdown**渲染示例

## 功能

- 代码高亮
- GFM 支持
- 数学公式
- 表格支持

\`\`\`js
console.log('Hello, BitFun!');
\`\`\`

> 引用块示例`}
          />
        ),
      },
      {
        id: 'code-editor',
        name: 'CodeEditor',
        description: '基于 Monaco Editor 的代码编辑器',
        category: 'content',
        component: () => {
          const [code, setCode] = React.useState(`// TypeScript 示例
interface User {
  name: string;
  age: number;
  email?: string;
}

class Person implements User {
  constructor(
    public name: string,
    public age: number,
    public email?: string
  ) {}

  greet(): string {
    return \`Hello, I'm \${this.name}\`;
  }
}

const user = new Person("Alice", 25);
console.log(user.greet());`);

          return (
            <div style={{ width: '100%' }}>
              <CodeEditor
                value={code}
                language="typescript"
                height="350px"
                minimap={false}
                lineNumbers="on"
                onChange={(value) => setCode(value || '')}
              />
            </div>
          );
        },
      },
    ],
  },
  {
    id: 'navigation',
    name: '导航组件',
    description: 'Demo',
    layoutType: 'grid-2',
    components: [
      {
        id: 'tabs-demo',
        name: 'Tabs - Demo',
        description: 'Demo',
        category: 'navigation',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
            <Tabs type="line" defaultActiveKey="1">
              <TabPane tabKey="1" label="Tab 1">
                <div style={{ padding: '16px' }}>Line 类型 - 内容1</div>
              </TabPane>
              <TabPane tabKey="2" label="Tab 2">
                <div style={{ padding: '16px' }}>Line 类型 - 内容2</div>
              </TabPane>
              <TabPane tabKey="3" label="Tab 3">
                <div style={{ padding: '16px' }}>Line 类型 - 内容3</div>
              </TabPane>
            </Tabs>

            <Tabs type="card" defaultActiveKey="1">
              <TabPane tabKey="1" label="Card 1">
                <div style={{ padding: '16px' }}>Card 类型 - 内容1</div>
              </TabPane>
              <TabPane tabKey="2" label="Card 2">
                <div style={{ padding: '16px' }}>Card 类型 - 内容2</div>
              </TabPane>
            </Tabs>

            <Tabs type="pill" defaultActiveKey="1">
              <TabPane tabKey="1" label="Pill 1">
                <div style={{ padding: '16px' }}>Pill 类型 - 内容1</div>
              </TabPane>
              <TabPane tabKey="2" label="Pill 2">
                <div style={{ padding: '16px' }}>Pill 类型 - 内容2</div>
              </TabPane>
            </Tabs>
          </div>
        ),
      },
    ],
  },
  {
    id: 'advanced-feedback',
    name: '高级反馈',
    description: '高级反馈组件',
    layoutType: 'grid-3',
    components: [
      {
        id: 'tooltip-demo',
        name: 'Tooltip - 位置演示',
        description: '气泡提示',
        category: 'advanced-feedback',
        component: () => (
          <div style={{ display: 'flex', gap: '24px', flexWrap: 'wrap', justifyContent: 'center', padding: '40px' }}>
            <Tooltip content="上方提示" placement="top">
              <Button>Top</Button>
            </Tooltip>
            <Tooltip content="下方提示" placement="bottom">
              <Button>Bottom</Button>
            </Tooltip>
            <Tooltip content="左侧提示" placement="left">
              <Button>Left</Button>
            </Tooltip>
            <Tooltip content="右侧提示" placement="right">
              <Button>Right</Button>
            </Tooltip>
          </div>
        ),
      },
    ],
  },
  {
    id: 'flowchat-cards',
    name: 'FlowChat 卡片',
    description: '展示 FlowChat 工具调用卡片组件的预览',
    layoutType: 'column',
    components: [
      {
        id: 'read-file-card',
        name: 'ReadFile - 文件读取卡片',
        description: '读取文件的工具卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>读取文件 - 成功</h3>
            <ReadFileDisplay
              toolItem={createMockToolItem('Read',
                { target_file: 'src/App.tsx', offset: 1, limit: 50 },
                {
                  content: 'import React from "react";\n\nfunction App() {\n  return <div>Hello World</div>;\n}\n\nexport default App;',
                  lines_read: 7,
                  total_lines: 150,
                  file_size: 2048
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Read']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>读取文件 - 执行中</h3>
            <ReadFileDisplay
              toolItem={createMockToolItem('Read',
                { target_file: 'src/components/Header.tsx' },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['Read']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'file-operation-card',
        name: 'FileOperation - 文件操作卡片',
        description: '文件写入、编辑、删除操作的工具卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>写入文件</h3>
            <FileOperationToolCard
              toolItem={createMockToolItem('Write',
                {
                  file_path: 'src/newFile.ts',
                  contents: 'export const greeting = "Hello World";'
                },
                { success: true },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Write']}
              sessionId="preview-session"
              onConfirm={async () => alert('已确认')}
              onReject={async () => alert('已拒绝')}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>编辑文件</h3>
            <FileOperationToolCard
              toolItem={createMockToolItem('Edit',
                {
                  file_path: 'src/components/Header.tsx',
                  old_string: 'const title = "Old Title"',
                  new_string: 'const title = "New Title"'
                },
                { success: true },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Edit']}
              sessionId="preview-session"
              onConfirm={async () => alert('已确认')}
              onReject={async () => alert('已拒绝')}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>删除文件</h3>
            <FileOperationToolCard
              toolItem={createMockToolItem('Delete',
                { target_file: 'src/oldFile.ts' },
                { success: true },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Delete']}
              sessionId="preview-session"
              onConfirm={async () => alert('已删除')}
              onReject={async () => alert('已取消')}
            />
          </div>
        ),
      },
      {
        id: 'search-card',
        name: 'Search - 搜索卡片',
        description: '搜索工具结果展示',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Grep 搜索结果</h3>
            <GrepSearchDisplay
              toolItem={createMockToolItem('Grep',
                { pattern: 'function', path: 'src/' },
                {
                  matches: [
                    'src/app.ts:10:function main() {',
                    'src/utils.ts:5:function helper() {'
                  ],
                  total_matches: 2,
                  files_searched: 10
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Grep']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Grep - 多结果示例</h3>
            <GrepSearchDisplay
              toolItem={createMockToolItem('Grep',
                { pattern: 'import React', path: 'src/components' },
                {
                  matches: [
                    "src/components/App.tsx:1:import React from 'react';",
                    "src/components/Header.tsx:1:import React from 'react';",
                    "src/components/Button.tsx:1:import React from 'react';"
                  ],
                  total_matches: 3,
                  files_searched: 20
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Grep']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Glob 搜索结果</h3>
            <GlobSearchDisplay
              toolItem={createMockToolItem('Glob',
                { glob_pattern: '*.tsx' },
                {
                  files: ['App.tsx', 'Header.tsx', 'Footer.tsx', 'Button.tsx'],
                  total_count: 4
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Glob']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>LS 目录列表</h3>
            <LSDisplay
              toolItem={createMockToolItem('LS',
                { target_directory: 'src/components' },
                {
                  items: [
                    'App.tsx',
                    'Header.tsx',
                    'Footer.tsx',
                    'Button.tsx',
                    'Input.tsx'
                  ],
                  total_count: 5
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['LS']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'task-card',
        name: 'Task - AI任务卡片',
        description: 'AI 任务执行卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>AI 任务 - 执行中</h3>
            <TaskToolDisplay
              toolItem={createMockToolItem('Task',
                {
                  description: '分析代码库结构',
                  prompt: '分析当前项目的代码结构',
                  model_name: 'claude-3.5-sonnet',
                  subagent_type: 'code-analyzer'
                },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['Task']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>AI 任务 - 已完成</h3>
            <TaskToolDisplay
              toolItem={createMockToolItem('Task',
                {
                  description: '创建新功能',
                  prompt: '创建一个新的功能模块',
                  model_name: 'claude-3.5-sonnet',
                  subagent_type: 'architect'
                },
                {
                  status: 'completed',
                  result: `任务已成功完成

1. 搭建 React + TypeScript 环境
2. 配置 Zustand 状态管理
3. 添加 SCSS 与 BEM 样式体系
4. 实现核心组件

所有需求均已满足`,
                  duration_ms: 12500,
                  tool_uses: 8
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Task']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'todo-card',
        name: 'TodoWrite - Todo任务管理',
        description: 'Todo 任务状态卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Todo - 基础示例</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: '任务 A', status: 'completed' },
                    { id: '2', content: '任务 B', status: 'in_progress' },
                    { id: '3', content: '任务 C', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: '任务 A', status: 'completed' },
                    { id: '2', content: '任务 B', status: 'in_progress' },
                    { id: '3', content: '任务 C', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - 多任务示例</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'in_progress' },
                    { id: '3', content: '集成 API', status: 'in_progress' },
                    { id: '4', content: '任务 4', status: 'in_progress' },
                    { id: '5', content: '任务 5', status: 'pending' },
                    { id: '6', content: '任务 6', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'in_progress' },
                    { id: '3', content: '集成 API', status: 'in_progress' },
                    { id: '4', content: '任务 4', status: 'in_progress' },
                    { id: '5', content: '任务 5', status: 'pending' },
                    { id: '6', content: '任务 6', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo进度 - 进行中</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'completed' },
                    { id: '3', content: '任务 3', status: 'in_progress' },
                    { id: '4', content: '任务 4', status: 'pending' },
                    { id: '5', content: '任务 5', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'completed' },
                    { id: '3', content: '任务 3', status: 'in_progress' },
                    { id: '4', content: '任务 4', status: 'pending' },
                    { id: '5', content: '任务 5', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - 待处理</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: '事项', status: 'pending' },
                    { id: '2', content: '集成 API', status: 'pending' },
                    { id: '3', content: '任务 3', status: 'pending' },
                    { id: '4', content: '任务 4', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: '事项', status: 'pending' },
                    { id: '2', content: '集成 API', status: 'pending' },
                    { id: '3', content: '任务 3', status: 'pending' },
                    { id: '4', content: '任务 4', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - 已完成</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'completed' },
                    { id: '3', content: '任务 3', status: 'completed' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: '任务 1', status: 'completed' },
                    { id: '2', content: '任务 2', status: 'completed' },
                    { id: '3', content: '任务 3', status: 'completed' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'web-search-card',
        name: 'WebSearch - 搜索结果卡片',
        description: '网络搜索结果和URL展示',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>网页搜索 - 结果</h3>
            <RealWebSearchCard
              toolItem={createMockToolItem('WebSearch',
                { query: 'React Hooks 教程' },
                {
                  results: [
                    {
                      title: 'React Hooks 指南',
                      url: 'https://react.dev/hooks',
                      snippet: '学习 React Hooks 的基础用法与最佳实践...'
                    }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['WebSearch']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>多结果 - 网页搜索</h3>
            <RealWebSearchCard
              toolItem={createMockToolItem('WebSearch',
                { query: 'TypeScript best practices' },
                {
                  results: [
                    {
                      title: 'TypeScript Best Practices',
                      url: 'https://www.typescriptlang.org/docs/handbook/declaration-files/do-s-and-don-ts.html',
                      snippet: 'This guide covers the best practices for writing TypeScript code...'
                    },
                    {
                      title: 'TypeScript Deep Dive',
                      url: 'https://basarat.gitbook.io/typescript/',
                      snippet: 'A comprehensive guide to TypeScript...'
                    },
                    {
                      title: 'Clean Code with TypeScript',
                      url: 'https://github.com/labs42io/clean-code-typescript',
                      snippet: "Software engineering principles, from Robert C. Martin's book..."
                    }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['WebSearch']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'mcp-tool-card',
        name: 'MCP - MCP工具卡片',
        description: '展示MCP工具调用的卡片组件',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>MCP工具 - 文件列表</h3>
            <MCPToolDisplay
              toolItem={createMockToolItem('mcp__server__list_files',
                { directory: '/project/src' },
                {
                  content: [
                    {
                      type: 'text',
                      text: 'Found 5 files:\n- App.tsx\n- Header.tsx\n- Footer.tsx\n- Button.tsx\n- Input.tsx'
                    }
                  ]
                },
                'completed'
              )}
              config={{
                toolName: 'mcp__server__list_files',
                displayName: 'list_files',
                icon: '🔌',
                requiresConfirmation: false,
                resultDisplayType: 'detailed',
                description: 'MCP工具调用',
                displayMode: 'compact',
                primaryColor: '#8b5cf6'
              }}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>MCP - 执行中</h3>
            <MCPToolDisplay
              toolItem={createMockToolItem('mcp__server__fetch_data',
                { url: 'https://api.example.com/data' },
                undefined,
                'running'
              )}
              config={{
                toolName: 'mcp__server__fetch_data',
                displayName: 'fetch_data',
                icon: '🔌',
                requiresConfirmation: false,
                resultDisplayType: 'detailed',
                description: 'MCP工具调用',
                displayMode: 'compact',
                primaryColor: '#8b5cf6'
              }}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'context-compression-card',
        name: 'ContextCompression - 上下文压缩',
        description: '上下文压缩过程卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>上下文压缩 - 示例</h3>
            <ContextCompressionDisplay
              toolItem={createMockToolItem('ContextCompression',
                {
                  trigger: 'ai_response',
                  tokens_before: 50000
                },
                {
                  compression_count: 3,
                  has_summary: true,
                  summary_source: 'model',
                  tokens_before: 50000,
                  tokens_after: 15000,
                  compression_ratio: 0.7,
                  duration: 2500
                },
                'completed'
              )}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>上下文压缩 - 本地 fallback</h3>
            <ContextCompressionDisplay
              toolItem={createMockToolItem('ContextCompression',
                {
                  trigger: 'manual',
                  tokens_before: 42000
                },
                {
                  compression_count: 4,
                  has_summary: false,
                  summary_source: 'local_fallback',
                  tokens_before: 42000,
                  tokens_after: 18000,
                  compression_ratio: 0.43,
                  duration: 1800
                },
                'completed'
              )}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>上下文压缩 - 执行中</h3>
            <ContextCompressionDisplay
              toolItem={createMockToolItem('ContextCompression',
                { trigger: 'user_message' },
                undefined,
                'running'
              )}
            />
          </div>
        ),
      },
      {
        id: 'skill-card',
        name: 'Skill - 技能调用',
        description: '展示Skill技能调用组件',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Skill调用</h3>
            <SkillDisplay
              toolItem={createMockToolItem('Skill',
                {
                  skill_name: 'code-review',
                  skill_input: { file_path: 'src/App.tsx' }
                },
                {
                  result: '代码审查已完成',
                  suggestions: ['使用 React.memo', '优化渲染性能', '修复现有警告']
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Skill']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'ask-user-card',
        name: 'AskUserQuestion - 用户问题',
        description: 'AI 用户提问卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>向用户提问 - 单题示例</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: '您更偏好哪个选项?',
                      header: '问题',
                      options: [
                        { label: '选项 1', description: '第一个选项' },
                        { label: '选项 2', description: '第二个选项' },
                        { label: '选项 3', description: '第三个选项' }
                      ],
                      multiSelect: false
                    }
                  ]
                },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['AskUserQuestion']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>多问题 - 单选和多选</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: '您想使用哪种UI框架?',
                      header: 'UI框架',
                      options: [
                        { label: 'React', description: '使用React框架' },
                        { label: 'Vue', description: '使用Vue框架' },
                        { label: 'Angular', description: '使用Angular框架' }
                      ],
                      multiSelect: false
                    },
                    {
                      question: '需要哪些开发工具?',
                      header: '开发工具',
                      options: [
                        { label: 'TypeScript', description: '使用TypeScript' },
                        { label: 'ESLint', description: '代码规范检查' },
                        { label: 'Prettier', description: '代码格式化工具' },
                        { label: '其他', description: '其他开发工具' }
                      ],
                      multiSelect: true
                    }
                  ]
                },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['AskUserQuestion']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>已回答 - 数据库选择</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: '您想使用哪种数据库?',
                      header: '数据库',
                      options: [
                        { label: 'PostgreSQL', description: '关系型数据库' },
                        { label: 'MongoDB', description: 'NoSQL文档数据库' },
                        { label: 'SQLite', description: '轻量级嵌入式数据库' }
                      ],
                      multiSelect: false
                    }
                  ]
                },
                {
                  status: 'answered',
                  answers: { "0": "PostgreSQL" }
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['AskUserQuestion']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'reproduction-steps-card',
        name: 'ReproductionSteps - 复现步骤',
        description: '用于展示问题复现步骤并等待用户操作确认的卡片',
        category: 'flowchat-cards',
        component: () => {
          const CompletedReproductionSteps = () => {
            const [hasProceeded] = React.useState(true);
            return (
              <div className={`reproduction-steps-block ${hasProceeded ? 'proceeded' : ''}`}>
                <div className="reproduction-steps-header">
                  <div className="reproduction-steps-icon">
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
                  </div>
                  <div className="reproduction-steps-title">Steps</div>
                </div>
                <div className="reproduction-steps-content">
                  <ol className="reproduction-steps-list">
                    <li className="reproduction-step-item">Step 1</li>
                    <li className="reproduction-step-item">Step 2</li>
                    <li className="reproduction-step-item">Step 3</li>
                  </ol>
                </div>
                <div className="reproduction-steps-completed">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
                  <span>Waiting for AI to proceed...</span>
                </div>
              </div>
            );
          };

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
              <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Reproduction Steps</h3>
              <ReproductionStepsBlock
                steps={`1. Run npm run dev
2. Open http://localhost:3000
3. Click "Button"
4. Check console`}
                onProceed={() => {}}
              />

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>已完成</h3>
              <CompletedReproductionSteps />
            </div>
          );
        },
      },
      {
        id: 'create-plan-card',
        name: 'CreatePlan - 计划创建',
        description: '计划创建卡片',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Create Plan - Streaming</h3>
            <CreatePlanDisplay
              toolItem={createMockToolItem('CreatePlan',
                {
                  name: 'Plan Name',
                  overview: 'Plan overview...'
                },
                null,
                'streaming'
              )}
              config={TOOL_CARD_CONFIGS['CreatePlan']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>创建计划 - 已完成</h3>
            <CreatePlanDisplay
              toolItem={createMockToolItem('CreatePlan',
                {},
                {
                  plan_file_path: '/Users/demo/.bitfun/projects/project-slug/plans/refactor-user-module.plan.md',
                  name: 'Refactor Module',
                  overview: 'Plan overview',
                  todos: [
                    { id: 'todo-1', content: 'Task 1', status: 'completed' },
                    { id: 'todo-2', content: 'Task 2', status: 'completed' },
                    { id: 'todo-3', content: 'Task 3', status: 'in_progress' },
                    { id: 'todo-4', content: 'Add CRUD operations', status: 'pending' },
                    { id: 'todo-5', content: 'Task 5', status: 'pending' },
                    { id: 'todo-6', content: 'API integration', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['CreatePlan']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Create Plan - Dark Mode</h3>
            <CreatePlanDisplay
              toolItem={createMockToolItem('CreatePlan',
                {},
                {
                  plan_file_path: '/Users/demo/.bitfun/projects/project-slug/plans/add-dark-mode.plan.md',
                  name: 'Dark Mode',
                  overview: 'Add dark mode support',
                  todos: [
                    { id: 'dm-1', content: 'Add CSS variables', status: 'completed' },
                    { id: 'dm-2', content: 'Update components', status: 'completed' },
                    { id: 'dm-3', content: 'Diagram 3', status: 'completed' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['CreatePlan']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'init-live-app-card',
        name: 'InitLiveApp - 灵动应用创建',
        description: '创建 Live App 骨架后的工具卡片（InitLiveApp）',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>InitLiveApp - 执行中</h3>
            <InitLiveAppDisplay
              toolItem={createMockToolItem(
                'InitLiveApp',
                { name: 'Weather Dashboard', description: 'A small weather widget' },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['InitLiveApp']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>InitLiveApp - 参数流式</h3>
            <InitLiveAppDisplay
              toolItem={
                {
                  ...createMockToolItem('InitLiveApp', {}, undefined, 'streaming'),
                  isParamsStreaming: true,
                  partialParams: { name: 'My Live App' },
                } as FlowToolItem
              }
              config={TOOL_CARD_CONFIGS['InitLiveApp']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>InitLiveApp - 创建成功</h3>
            <InitLiveAppDisplay
              toolItem={createMockToolItem(
                'InitLiveApp',
                { name: 'Weather Dashboard' },
                {
                  app_id: 'la-preview-001',
                  path: '.bitfun/liveapps/la-preview-001',
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['InitLiveApp']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'model-thinking-card',
        name: 'ModelThinking - 思考过程',
        description: '展示 AI 模型推理过程的思考状态组件',
        category: 'flowchat-cards',
        component: () => {
          const createMockThinkingItem = (
            content: string,
            isStreaming: boolean,
            status: 'streaming' | 'completed'
          ): FlowThinkingItem => ({
            id: `thinking-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
            type: 'thinking',
            timestamp: Date.now(),
            status,
            content,
            isStreaming,
            isCollapsed: !isStreaming
          });

          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
              <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>模型思考 - 流式输出</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `正在分析用户的请求..

让我仔细思考这个问题的解决方案
- 首先需要理解用户的具体需求
- 然后考虑可行的实现方案
- 最后选择最优的解决方案

继续深入分析相关细节..`,
                  true,
                  'streaming'
                )}
              />

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>模型思考 - 折叠完成状态</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `分析了用户关于性能优化的问题

解决方案
1. 使用虚拟列表减少DOM渲染
2. 引入 React.memo 缓存组件
3. 懒加载非关键资源

优化重点
- 减少不必要的重渲染
- 合理使用 memoization
- 分割大型组件

以上方案可以显著提升应用性能`,
                  false,
                  'completed'
                )}
              />

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>模型思考 - 长内容展示</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `这是一个复杂任务，需要多步骤分析

背景信息
用户希望在组件库预览页面中展示各种工具卡片的效果，包括FlowChat 相关的 ModelThinkingDisplay 组件

需求分析
我需要为预览页面创建ModelThinkingDisplay 的示例数据，包含以下场景
- 流式输出状态（模拟AI正在思考的动态效果）
- 完成折叠状态（思考完成后默认折叠）

实现计划
在 registry.tsx 中
1. 导入 ModelThinkingDisplay 组件
2. 创建 FlowThinkingItem 类型数据
3. 设置不同的状态场景
4. 添加展示样例

执行结果
已成功创建三种不同状态的思考展示示例

总结
ModelThinkingDisplay 组件展示效果符合预期`,
                  false,
                  'completed'
                )}
              />
            </div>
          );
        },
      },
    ],
  },
];
