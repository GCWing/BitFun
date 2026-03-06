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
import { MermaidInteractiveDisplay } from '@/flow_chat/tool-cards/MermaidInteractiveDisplay';
import { ContextCompressionDisplay } from '@/flow_chat/tool-cards/ContextCompressionDisplay';
import { ImageAnalysisCard } from '@/flow_chat/tool-cards/ImageAnalysisCard';
import { IdeControlToolCard } from '@/flow_chat/tool-cards/IdeControlToolCard';
import { LinterToolCard } from '@/flow_chat/tool-cards/LinterToolCard';
import { SkillDisplay } from '@/flow_chat/tool-cards/SkillDisplay';
import { AskUserQuestionCard } from '@/flow_chat/tool-cards/AskUserQuestionCard';
import { GitToolDisplay } from '@/flow_chat/tool-cards/GitToolDisplay';
import { CreatePlanDisplay } from '@/flow_chat/tool-cards/CreatePlanDisplay';
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
      error: status === 'error' ? '????' : undefined
    } : undefined,
    config: config || {
      toolName,
      displayName: toolName,
      icon: '??',
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
    name: '????',
    description: '?????UI ??',
    layoutType: 'grid-4',
    components: [
      {
        id: 'button-primary',
        name: 'Button - Primary',
        description: '????',
        category: 'basic',
        component: () => <Button variant="primary">Primary Button</Button>,
      },
      {
        id: 'button-secondary',
        name: 'Button - Secondary',
        description: '????',
        category: 'basic',
        component: () => <Button variant="secondary">Secondary Button</Button>,
      },
      {
        id: 'button-ghost',
        name: 'Button - Ghost',
        description: '????',
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
        name: 'Tag - ??',
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
        name: 'IconButton - ??',
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
        name: 'IconButton - ??',
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
        name: 'IconButton - ??',
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
        name: 'WindowControls - ????',
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
    name: '????',
    description: 'Demo',
    layoutType: 'demo',
    components: [
      {
        id: 'cube-loading-variants',
        name: 'CubeLoading - ??????',
        description: '3x3x3 ??????????',
        category: 'feedback',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '32px', padding: '20px' }}>
            {}
            <div>
              <div style={{ fontSize: '12px', color: '#666', marginBottom: '16px', fontWeight: 500 }}>??</div>
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
                <CubeLoading text="????.." />
                <CubeLoading size="large" text="????.." />
              </div>
            </div>
          </div>
        ),
      },
      {
        id: 'modal-basic',
        name: 'Modal - Basic',
        description: '?????',
        category: 'feedback',
        component: () => {
          const [isOpen, setIsOpen] = React.useState(false);
          return (
            <>
              <Button onClick={() => setIsOpen(true)}>?????</Button>
              <Modal
                isOpen={isOpen}
                onClose={() => setIsOpen(false)}
                title="?????"
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
        name: 'Alert - ????',
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
        name: 'StreamText - ??????',
        description: 'AI ????????',
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
                ?? ????
              </Button>
            </div>
          );
        },
      },
    ],
  },
  {
    id: 'form',
    name: '????',
    description: '???????',
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
              label="??"
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
                placeholder="??????.."
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
        name: 'Select - ????',
        description: '??????????',
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
                placeholder="????"
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
        description: '??????????',
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
        name: 'Select - ????',
        description: '????????',
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
                label="?????"
                searchable
                options={options}
                placeholder="???"
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
        description: '????????',
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
        description: '?????????????',
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
        description: '?????',
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
                label="????"
                placeholder="???00??.."
                showCount
                maxLength={100}
                value={value}
                onChange={(e) => setValue(e.target.value)}
              />
              <Textarea
                label="??????"
                placeholder="?????????..."
                autoResize
              />
              <Textarea
                label="Error"
                error
                errorMessage="??????"
                placeholder="????.."
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
    name: '????',
    description: '??????????????',
    layoutType: 'large-card',
    components: [
      {
        id: 'markdown-viewer',
        name: 'Markdown Demo',
description: 'Markdown with GFM support',
        category: 'content',
        component: () => (
          <Markdown
            content={`# Markdown ??

??????**Markdown** ??????

## ???

        - ????
        - GFM ??
        - ????
        - ????

\`\`\`js
console.log('Hello, BitFun!');
\`\`\`

> ?????`}
          />
        ),
      },
      {
        id: 'code-editor',
        name: 'CodeEditor',
        description: '?? Monaco Editor ??????',
        category: 'content',
        component: () => {
          const [code, setCode] = React.useState(`// TypeScript ????
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
                showLineNumbers={true}
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
    name: '????',
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
                <div style={{ padding: '16px' }}>Line ?? - ??1</div>
              </TabPane>
              <TabPane tabKey="2" label="Tab 2">
                <div style={{ padding: '16px' }}>Line ?? - ??2</div>
              </TabPane>
              <TabPane tabKey="3" label="Tab 3">
                <div style={{ padding: '16px' }}>Line ?? - ??3</div>
              </TabPane>
            </Tabs>

            <Tabs type="card" defaultActiveKey="1">
              <TabPane tabKey="1" label="Card 1">
                <div style={{ padding: '16px' }}>Card ?? - ??1</div>
              </TabPane>
              <TabPane tabKey="2" label="Card 2">
                <div style={{ padding: '16px' }}>Card ?? - ??2</div>
              </TabPane>
            </Tabs>

            <Tabs type="pill" defaultActiveKey="1">
              <TabPane tabKey="1" label="Pill 1">
                <div style={{ padding: '16px' }}>Pill ?? - ??1</div>
              </TabPane>
              <TabPane tabKey="2" label="Pill 2">
                <div style={{ padding: '16px' }}>Pill ?? - ??2</div>
              </TabPane>
            </Tabs>
          </div>
        ),
      },
    ],
  },
  {
    id: 'advanced-feedback',
    name: '????',
    description: '??????',
    layoutType: 'grid-3',
    components: [
      {
        id: 'tooltip-demo',
        name: 'Tooltip - ????',
        description: '????',
        category: 'advanced-feedback',
        component: () => (
          <div style={{ display: 'flex', gap: '24px', flexWrap: 'wrap', justifyContent: 'center', padding: '40px' }}>
            <Tooltip content="Top??" placement="top">
              <Button>Top</Button>
            </Tooltip>
            <Tooltip content="Bottom??" placement="bottom">
              <Button>Bottom</Button>
            </Tooltip>
            <Tooltip content="Left??" placement="left">
              <Button>Left</Button>
            </Tooltip>
            <Tooltip content="Right??" placement="right">
              <Button>Right</Button>
            </Tooltip>
          </div>
        ),
      },
    ],
  },
  {
    id: 'flowchat-cards',
    name: 'FlowChat ??',
    description: '?? FlowChat ????????????????',
    layoutType: 'column',
    components: [
      {
        id: 'read-file-card',
        name: 'ReadFile - ??????',
        description: '????????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Read - Success</h3>
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

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Read - Running</h3>
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
        name: 'FileOperation - ??????',
        description: '???????????????????????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>????</h3>
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
              onConfirm={async () => alert('??????')}
              onReject={async () => alert('??????')}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>????</h3>
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
              onConfirm={async () => alert('??????')}
              onReject={async () => alert('??????')}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>????</h3>
            <FileOperationToolCard
              toolItem={createMockToolItem('Delete',
                { target_file: 'src/oldFile.ts' },
                { success: true },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Delete']}
              sessionId="preview-session"
              onConfirm={async () => alert('????')}
              onReject={async () => alert('????')}
            />
          </div>
        ),
      },
      {
        id: 'search-card',
        name: 'Search - ????',
        description: 'Demo',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Grep ????</h3>
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

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Grep - ????</h3>
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

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Glob ????</h3>
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

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>LS ????</h3>
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
        name: 'Task - AI????',
        description: 'AI task execution',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>AI Task - Running</h3>
            <TaskToolDisplay
              toolItem={createMockToolItem('Task',
                {
                  description: 'Demo',
                  prompt: 'Analyze the codebase structure',
                  model_name: 'claude-3.5-sonnet',
                  subagent_type: 'code-analyzer'
                },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['Task']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>AI Task - Completed</h3>
            <TaskToolDisplay
              toolItem={createMockToolItem('Task',
                {
                  description: 'Task description',
                  prompt: 'Create a new feature',
                  model_name: 'claude-3.5-sonnet',
                  subagent_type: 'architect'
                },
                {
                  status: 'completed',
                  result: `Task completed successfully

1. Setup React + TypeScript
2. Configure Zustand store
3. Add SCSS + BEM styles
4. Implement components

All requirements met`,
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
        name: 'TodoWrite - Todo??????',
        description: 'Demo',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Todo - Basic</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: 'Task A', status: 'completed' },
                    { id: '2', content: 'Task B', status: 'in_progress' },
                    { id: '3', content: 'Task C', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: 'Task A', status: 'completed' },
                    { id: '2', content: 'Task B', status: 'in_progress' },
                    { id: '3', content: 'Task C', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - Multiple</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'in_progress' },
                    { id: '3', content: 'API integration', status: 'in_progress' },
                    { id: '4', content: 'Task 4', status: 'in_progress' },
                    { id: '5', content: 'Task 5', status: 'pending' },
                    { id: '6', content: 'Task 6', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'in_progress' },
                    { id: '3', content: 'API integration', status: 'in_progress' },
                    { id: '4', content: 'Task 4', status: 'in_progress' },
                    { id: '5', content: 'Task 5', status: 'pending' },
                    { id: '6', content: 'Task 6', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo???? - ????</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'completed' },
                    { id: '3', content: 'Task 3', status: 'in_progress' },
                    { id: '4', content: 'Task 4', status: 'pending' },
                    { id: '5', content: 'Task 5', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'completed' },
                    { id: '3', content: 'Task 3', status: 'in_progress' },
                    { id: '4', content: 'Task 4', status: 'pending' },
                    { id: '5', content: 'Task 5', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - Pending</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: 'Item', status: 'pending' },
                    { id: '2', content: 'API integration', status: 'pending' },
                    { id: '3', content: 'Task 3', status: 'pending' },
                    { id: '4', content: 'Task 4', status: 'pending' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: 'Item', status: 'pending' },
                    { id: '2', content: 'API integration', status: 'pending' },
                    { id: '3', content: 'Task 3', status: 'pending' },
                    { id: '4', content: 'Task 4', status: 'pending' }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['TodoWrite']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Todo - Completed</h3>
            <TodoWriteDisplay
              toolItem={createMockToolItem('TodoWrite',
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'completed' },
                    { id: '3', content: 'Task 3', status: 'completed' }
                  ]
                },
                {
                  todos: [
                    { id: '1', content: 'Task 1', status: 'completed' },
                    { id: '2', content: 'Task 2', status: 'completed' },
                    { id: '3', content: 'Task 3', status: 'completed' }
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
        name: 'WebSearch - ??????',
        description: '???????URL??',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Web Search - Results</h3>
            <RealWebSearchCard
              toolItem={createMockToolItem('WebSearch',
                { query: 'React hooks tutorial' },
                {
                  results: [
                    {
                      title: 'React Hooks Guide',
                      url: 'https://react.dev/hooks',
                      snippet: 'Learn about React Hooks...'
                    }
                  ]
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['WebSearch']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>???? - ????</h3>
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
        name: 'MCP - MCP????',
        description: '??MCP????????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>MCP?? - ????</h3>
            <MCPToolDisplay
              toolItem={createMockToolItem('mcp_server_list_files',
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
                toolName: 'mcp_server_list_files',
                displayName: 'list_files',
                icon: '??',
                requiresConfirmation: false,
                resultDisplayType: 'detailed',
                description: 'MCP?? from server',
                displayMode: 'compact',
                primaryColor: '#8b5cf6'
              }}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>MCP - Running</h3>
            <MCPToolDisplay
              toolItem={createMockToolItem('mcp_server_fetch_data',
                { url: 'https://api.example.com/data' },
                undefined,
                'running'
              )}
              config={{
                toolName: 'mcp_server_fetch_data',
                displayName: 'fetch_data',
                icon: '??',
                requiresConfirmation: false,
                resultDisplayType: 'detailed',
                description: 'MCP?? from server',
                displayMode: 'compact',
                primaryColor: '#8b5cf6'
              }}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'mermaid-interactive-card',
        name: 'MermaidInteractive - Mermaid????',
        description: '??Mermaid??????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Mermaid - Completed</h3>
            <MermaidInteractiveDisplay
              toolItem={createMockToolItem('MermaidInteractive',
                {
                  mermaid_code: 'graph TD\n  A[Start] --> B[Process]\n  B --> C[End]',
                  title: 'Diagram',
                  mode: 'interactive'
                },
                {
                  panel_id: 'mermaid-123',
                  success: true
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['MermaidInteractive']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Mermaid - Running</h3>
            <MermaidInteractiveDisplay
              toolItem={createMockToolItem('MermaidInteractive',
                {
                  mermaid_code: 'sequenceDiagram\n  Alice->>Bob: Hello',
                  title: 'Diagram',
                  mode: 'interactive'
                },
                undefined,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['MermaidInteractive']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'context-compression-card',
        name: 'ContextCompression - Demo',
        description: 'Demo',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Context Compression - Demo</h3>
            <ContextCompressionDisplay
              toolItem={createMockToolItem('ContextCompression',
                {
                  trigger: 'ai_response',
                  tokens_before: 50000
                },
                {
                  compression_count: 3,
                  has_summary: true,
                  tokens_before: 50000,
                  tokens_after: 15000,
                  compression_ratio: 0.7,
                  duration: 2500
                },
                'completed'
              )}
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Context Compression - Running</h3>
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
        id: 'image-analysis-card',
        name: 'view_image - ????',
        description: '????????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Read - Success</h3>
            <ImageAnalysisCard
              toolItem={createMockToolItem('view_image',
                {
                  image_path: '/path/to/screenshot.png',
                  analysis_prompt: 'Analyze the UI components',
                  focus_areas: ['UI elements', 'Layout', 'Colors']
                },
                {
                  analysis: 'The screenshot shows a TypeScript/React application with a modern UI design. The layout includes a header, sidebar, and main content area.',
                  model_used: 'gpt-4-vision',
                  image_path: '/path/to/screenshot.png'
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['view_image']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'ide-control-card',
        name: 'IdeControl - IDE??',
        description: '??IDE????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>IDE Control - Demo</h3>
            <IdeControlToolCard
              toolItem={createMockToolItem('IdeControl',
                {
                  action: 'open_file',
                  file_path: 'src/App.tsx'
                },
                { success: true },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['IdeControl']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'linter-card',
        name: 'ReadLints - Demo',
        description: 'Demo',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Linter - Demo</h3>
            <LinterToolCard
              toolItem={createMockToolItem('ReadLints',
                { path: 'src/App.tsx' },
                {
                  path_type: 'file',
                  path: 'src/App.tsx',
                  diagnostics: {
                    'src/App.tsx': {
                      file_path: 'src/App.tsx',
                      language: 'typescript',
                      lsp_status: 'ready',
                      items: [
                        {
                          severity: 2,
                          severity_text: 'Warning',
                          line: 10,
                          column: 5,
                          message: "'unusedVar' is declared but its value is never read.",
                          code: '6133',
                          source: 'ts'
                        },
                        {
                          severity: 1,
                          severity_text: 'Error',
                          line: 25,
                          column: 12,
                          message: "Property 'handleClick' does not exist on type 'Props'.",
                          code: '2339',
                          source: 'ts'
                        }
                      ],
                      error_count: 1,
                      warning_count: 1,
                      info_count: 0,
                      hint_count: 0
                    }
                  },
                  summary: {
                    total_files: 1,
                    files_with_issues: 1,
                    total_diagnostics: 2,
                    error_count: 1,
                    warning_count: 1,
                    info_count: 0,
                    hint_count: 0
                  },
                  warnings: []
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['ReadLints']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'skill-card',
        name: 'Skill - Demo',
        description: '??Skill???????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Skill??</h3>
            <SkillDisplay
              toolItem={createMockToolItem('Skill',
                {
                  skill_name: 'code-review',
                  skill_input: { file_path: 'src/App.tsx' }
                },
                {
                  result: 'Code review completed',
                  suggestions: ['Use React.memo', 'Optimize render', 'Fix warnings']
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
        name: 'AskUserQuestion - ????',
        description: 'AI user question',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Ask User - Question</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: 'Which option do you prefer?',
                      header: 'Question',
                      options: [
                        { label: 'Option 1', description: 'First option' },
                        { label: 'Option 2', description: 'Second option' },
                        { label: 'Option 3', description: 'Third option' }
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

            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>???? - ??????</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: '??????UI????',
                      header: 'UI??',
                      options: [
                        { label: 'React', description: '??React????' },
                        { label: 'Vue', description: '??Vue????' },
                        { label: 'Angular', description: '??Angular????' }
                      ],
                      multiSelect: false
                    },
                    {
                      question: '?????????',
                      header: '????',
                      options: [
                        { label: 'TypeScript', description: '??TypeScript??' },
                        { label: 'ESLint', description: '???????' },
                        { label: 'Prettier', description: '????????' },
                        { label: '????', description: '????????' }
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

            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Read - Success</h3>
            <AskUserQuestionCard
              toolItem={createMockToolItem('AskUserQuestion',
                {
                  questions: [
                    {
                      question: '????????',
                      header: '????',
                      options: [
                        { label: 'PostgreSQL', description: '??????' },
                        { label: 'MongoDB', description: 'NoSQL??????' },
                        { label: 'SQLite', description: '??????????' }
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
        name: 'ReproductionSteps - ????',
        description: '????????????????????????????????',
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

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Completed</h3>
              <CompletedReproductionSteps />
            </div>
          );
        },
      },
      {
        id: 'create-plan-card',
        name: 'CreatePlan - ????',
        description: 'Demo',
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

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Create Plan - Completed</h3>
            <CreatePlanDisplay
              toolItem={createMockToolItem('CreatePlan',
                {},
                {
                  plan_file_path: '/project/.bitfun/plans/refactor-user-module.md',
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
                  plan_file_path: '/project/.bitfun/plans/add-dark-mode.md',
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
        id: 'git-tool-card',
        name: 'Git - ??????',
        description: '??Git????????????Git????',
        category: 'flowchat-cards',
        component: () => (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', padding: '20px' }}>
            <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>Git Status - Success</h3>
            <GitToolDisplay
              toolItem={createMockToolItem('Git',
                {
                  operation: 'status',
                  args: '',
                  working_directory: '/project'
                },
                {
                  success: true,
                  exit_code: 0,
                  stdout: `On branch main
Your branch is up to date with 'origin/main'.

Changes to be committed:
  (use "git restore --staged <file>..." to unstage)
        modified:   src/components/App.tsx
        new file:   src/utils/helpers.ts

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
        modified:   package.json`,
                  stderr: '',
                  execution_time_ms: 45,
                  working_directory: '/project',
                  command: 'git status',
                  operation: 'status'
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Git']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Git Commit - Success</h3>
            <GitToolDisplay
              toolItem={createMockToolItem('Git',
                {
                  operation: 'commit',
                  args: '-m "feat: add new feature"',
                  working_directory: '/project'
                },
                {
                  success: true,
                  exit_code: 0,
                  stdout: `[main abc1234] feat: add new feature
 2 files changed, 45 insertions(+), 12 deletions(-)
 create mode 100644 src/utils/helpers.ts`,
                  stderr: '',
                  execution_time_ms: 120,
                  command: 'git commit -m "feat: add new feature"',
                  operation: 'commit'
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Git']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Git Diff - View</h3>
            <GitToolDisplay
              toolItem={createMockToolItem('Git',
                {
                  operation: 'diff',
                  args: 'HEAD~1',
                  working_directory: '/project'
                },
                {
                  success: true,
                  exit_code: 0,
                  stdout: `diff --git a/src/App.tsx b/src/App.tsx
index abc1234..def5678 100644
--- a/src/App.tsx
+++ b/src/App.tsx
@@ -10,6 +10,8 @@ export function App() {
   const [count, setCount] = useState(0);
+  const [name, setName] = useState('');
+
   return (
     <div className="app">`,
                  stderr: '',
                  execution_time_ms: 35,
                  command: 'git diff HEAD~1',
                  operation: 'diff'
                },
                'completed'
              )}
              config={TOOL_CARD_CONFIGS['Git']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Git Push - Running</h3>
            <GitToolDisplay
              toolItem={createMockToolItem('Git',
                {
                  operation: 'push',
                  args: 'origin main',
                  working_directory: '/project'
                },
                null,
                'running'
              )}
              config={TOOL_CARD_CONFIGS['Git']}
              sessionId="preview-session"
            />

            <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>Git Pull - ????</h3>
            <GitToolDisplay
              toolItem={createMockToolItem('Git',
                {
                  operation: 'pull',
                  args: 'origin main',
                  working_directory: '/project'
                },
                {
                  success: false,
                  exit_code: 1,
                  stdout: '',
                  stderr: `error: Your local changes to the following files would be overwritten by merge:
        src/config.ts
Please commit your changes or stash them before you merge.
Aborting`,
                  execution_time_ms: 1500,
                  command: 'git pull origin main',
                  operation: 'pull'
                },
                'error'
              )}
              config={TOOL_CARD_CONFIGS['Git']}
              sessionId="preview-session"
            />
          </div>
        ),
      },
      {
        id: 'model-thinking-card',
        name: 'ModelThinking - Demo',
        description: '??AI????????????????????????',
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
              <h3 style={{ color: '#ffffff', marginBottom: '8px' }}>?????- ??????</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `??????????..

??????????????
- ???????????????
- ?????????????
- ????????????????

????????????????..`,
                  true,
                  'streaming'
                )}
              />

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>?????- ???????????</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `???????????????????

??????
1. ????????????
2. ???? React.memo ????
3. ??????????

????????
- ???????????
- ???? memoization
- ?????????

??????????????????????`,
                  false,
                  'completed'
                )}
              />

              <h3 style={{ color: '#ffffff', marginTop: '16px', marginBottom: '8px' }}>?????- ??????</h3>
              <ModelThinkingDisplay
                thinkingItem={createMockThinkingItem(
                  `???????????????????????????

????????
?????????????????????????????????????????????????FlowChat ???? ModelThinkingDisplay ????

?????????
????????????ModelThinkingDisplay ??????????????
- ?????????????????????????
- ???????????????????????

????????
??? registry.tsx ??
1. ?? ModelThinkingDisplay ??
2. ?? FlowThinkingItem ??
3. ??????????
4. ????????

????????
????????????????????????????

????
???????????????????????`,
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
