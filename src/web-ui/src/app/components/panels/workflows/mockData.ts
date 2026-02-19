/**
 * Mock workflow data for development.
 */

import type { Workflow } from './types';

export const MOCK_WORKFLOWS: Workflow[] = [
  {
    id: 'wf-email-assistant',
    name: 'email-assistant',
    displayName: '邮件助手',
    description: '智能邮件撰写、回复和管理',
    icon: 'mail',
    version: '1.0',
    location: 'user',
    enabled: true,
    tags: ['邮件', '办公'],
    trigger: { type: 'slash_command', command: '/email' },
    input: {
      description: '邮件相关的需求',
      examples: [
        '帮我给张三写一封项目进度汇报邮件',
        '回复这封邮件，同意对方的方案',
      ],
    },
    agents: [
      {
        id: 'coordinator',
        role: 'orchestrator',
        inline: {
          name: '邮件协调员',
          description: '分析需求并协调邮件撰写流程',
          prompt: '你是邮件助手的协调员。根据用户需求决定如何处理邮件任务。',
          model: 'primary',
          tools: ['mcp_gmail_send_email', 'mcp_gmail_read_email', 'mcp_gmail_search', 'WebSearch'],
          skills: ['email-writing'],
          readonly: false,
        },
      },
      {
        id: 'researcher',
        role: 'worker',
        inline: {
          name: '信息调研员',
          description: '搜索和整理相关背景信息',
          prompt: '你负责搜索和整理撰写邮件所需的背景信息。',
          model: 'fast',
          tools: ['WebSearch', 'Read', 'Grep'],
          skills: [],
          readonly: true,
        },
      },
      {
        id: 'reviewer',
        role: 'reviewer',
        inline: {
          name: '邮件审核员',
          description: '审核邮件的语法、语气和格式',
          prompt: '审核邮件的：1) 语法和拼写 2) 语气是否恰当 3) 格式是否规范',
          model: 'primary',
          tools: [],
          skills: ['email-writing'],
          readonly: true,
        },
      },
    ],
    orchestration: {
      pattern: 'supervisor',
      supervisor: { agentId: 'coordinator', maxDelegationDepth: 2 },
    },
  },
  {
    id: 'wf-code-review',
    name: 'code-review',
    displayName: '代码审查',
    description: '多维度代码质量审查流程',
    icon: 'scan-search',
    version: '1.0',
    location: 'project',
    enabled: true,
    tags: ['开发', '审查'],
    trigger: { type: 'slash_command', command: '/review' },
    input: {
      description: '要审查的代码或文件路径',
      examples: ['审查 src/auth 模块', '检查最近的 PR 变更'],
    },
    agents: [
      {
        id: 'analyzer',
        role: 'worker',
        inline: {
          name: '静态分析员',
          description: '分析代码结构和潜在问题',
          prompt: '分析代码的结构、依赖关系和潜在问题。',
          model: 'primary',
          tools: ['Read', 'Grep', 'Glob', 'ReadLints'],
          skills: ['code-review'],
          readonly: true,
        },
      },
      {
        id: 'logic-reviewer',
        role: 'worker',
        inline: {
          name: '逻辑审查员',
          description: '深入审查代码逻辑',
          prompt: '基于分析结果进行深入的逻辑审查。',
          model: 'primary',
          tools: ['Read', 'Grep'],
          skills: ['code-review'],
          readonly: true,
        },
      },
      {
        id: 'reporter',
        role: 'worker',
        inline: {
          name: '报告生成员',
          description: '汇总生成结构化审查报告',
          prompt: '汇总审查结果，生成结构化的审查报告。',
          model: 'primary',
          tools: ['Write'],
          skills: [],
          readonly: false,
        },
      },
    ],
    orchestration: {
      pattern: 'pipeline',
      steps: [
        { agentId: 'analyzer' },
        { agentId: 'logic-reviewer' },
        { agentId: 'reporter' },
      ],
    },
  },
  {
    id: 'wf-translator',
    name: 'translator',
    displayName: '翻译助手',
    description: '中英互译，保持技术术语准确',
    icon: 'languages',
    version: '1.0',
    location: 'user',
    enabled: true,
    tags: ['翻译', '写作'],
    trigger: { type: 'hotkey', hotkey: 'Ctrl+Shift+T' },
    input: {
      description: '要翻译的文本',
      examples: ['翻译这段英文文档', '把这封邮件翻译成英文'],
    },
    agents: [
      {
        id: 'translator',
        role: 'orchestrator',
        inline: {
          name: '翻译专家',
          description: '专业的中英互译助手',
          prompt: '你是专业的中英互译助手，保持技术术语准确，语言自然流畅。',
          model: 'fast',
          tools: ['WebSearch'],
          skills: [],
          readonly: true,
        },
      },
    ],
    orchestration: { pattern: 'single' },
  },
  {
    id: 'wf-daily-report',
    name: 'daily-report',
    displayName: '日报生成',
    description: '自动汇总今日工作生成日报',
    icon: 'file-bar-chart',
    version: '1.0',
    location: 'user',
    enabled: false,
    tags: ['办公', '报告'],
    trigger: { type: 'slash_command', command: '/daily' },
    agents: [
      {
        id: 'git-collector',
        role: 'worker',
        inline: {
          name: 'Git 收集员',
          description: '收集今天的 Git 提交记录',
          prompt: '收集今天的 git 提交记录并分类整理。',
          model: 'fast',
          tools: ['Bash', 'Read'],
          skills: [],
          readonly: true,
        },
      },
      {
        id: 'summarizer',
        role: 'orchestrator',
        inline: {
          name: '日报撰写员',
          description: '整合信息生成日报',
          prompt: '将收集到的信息整合为日报格式。',
          model: 'primary',
          tools: ['Write'],
          skills: [],
          readonly: false,
        },
      },
    ],
    orchestration: {
      pattern: 'fan_out',
      steps: [
        { agentId: 'git-collector' },
        { agentId: 'summarizer', condition: 'after_all' },
      ],
    },
  },
];
