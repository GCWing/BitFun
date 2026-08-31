import React, { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createRoot, type Root } from 'react-dom/client';
import { JSDOM } from 'jsdom';

import { MCPToolDisplay } from './MCPToolDisplay';
import type { FlowToolItem, ToolCardConfig } from '../types/flow-chat';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mcpMocks = vi.hoisted(() => ({
  getCachedToolInfo: vi.fn(),
  getMCPToolUiUri: vi.fn(),
  fetchMCPAppResource: vi.fn(),
  sendMCPAppMessage: vi.fn(),
}));

vi.mock('react-i18next', async () => {
  const { createTestI18nT } = await import('@/test/i18nTestUtils');
  return {
    useTranslation: () => ({
      t: createTestI18nT('flow-chat'),
    }),
  };
});

vi.mock('../../component-library', () => ({
  CubeLoading: () => <span data-testid="cube-loading" />,
  IconButton: ({
    children,
    tooltip,
    variant: _variant,
    size: _size,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    tooltip?: React.ReactNode;
    variant?: string;
    size?: string;
  }) => (
    <button
      type="button"
      aria-label={typeof tooltip === 'string' ? tooltip : undefined}
      {...props}
    >
      {children}
    </button>
  ),
}));

vi.mock('@/infrastructure/mcp/toolInfoCache', () => ({
  getCachedToolInfo: mcpMocks.getCachedToolInfo,
}));

vi.mock('@/infrastructure/api/service-api/MCPAPI', () => ({
  MCP_APPS_PROTOCOL_VERSION: '2026-01-26',
  MCPAPI: {
    getMCPToolUiUri: mcpMocks.getMCPToolUiUri,
    fetchMCPAppResource: mcpMocks.fetchMCPAppResource,
    sendMCPAppMessage: mcpMocks.sendMCPAppMessage,
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
  }),
}));

const config: ToolCardConfig = {
  toolName: 'mcp__example__search',
  displayName: 'Search',
  icon: 'MCP',
  requiresConfirmation: false,
  resultDisplayType: 'detailed',
  description: 'Search through an MCP server',
  displayMode: 'compact',
};

function toolItem(overrides: Partial<FlowToolItem> = {}): FlowToolItem {
  return {
    id: 'tool-mcp-1',
    type: 'tool',
    toolName: config.toolName,
    status: 'completed',
    timestamp: Date.now(),
    toolCall: {
      id: 'call-mcp-1',
      input: {
        query: 'BitFun',
        limit: 5,
        _early_detection: false,
      },
    },
    toolResult: {
      success: true,
      result: {
        content: [{ type: 'text', text: 'Search result' }],
      },
    },
    ...overrides,
  };
}

describe('MCPToolDisplay', () => {
  let dom: JSDOM;
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mcpMocks.getCachedToolInfo.mockReset().mockImplementation(() => new Promise(() => {}));
    mcpMocks.getMCPToolUiUri.mockReset().mockImplementation(() => new Promise(() => {}));
    mcpMocks.fetchMCPAppResource.mockReset();
    mcpMocks.sendMCPAppMessage.mockReset();

    dom = new JSDOM('<!doctype html><html><body><div id="root"></div></body></html>', {
      pretendToBeVisual: true,
    });
    Object.defineProperty(dom.window, 'matchMedia', {
      configurable: true,
      value: () => ({ matches: true }),
    });
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    vi.stubGlobal('HTMLElement', dom.window.HTMLElement);
    vi.stubGlobal('CustomEvent', dom.window.CustomEvent);
    vi.stubGlobal('ResizeObserver', class {
      observe = vi.fn();
      disconnect = vi.fn();
    });

    container = dom.window.document.getElementById('root') as HTMLDivElement;
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    vi.unstubAllGlobals();
  });

  it('keeps invocation parameters collapsed until their nested toggle is used', () => {
    act(() => {
      root.render(<MCPToolDisplay toolItem={toolItem()} config={config} />);
    });

    expect(container.querySelector('.mcp-input-code')).toBeNull();

    act(() => {
      container.querySelector('.base-tool-card')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    const input = container.querySelector('.content-item-input');
    const result = container.querySelector('.content-item-text');
    expect(input?.textContent).toContain('Input Parameters');
    expect(container.querySelector('.mcp-input-code')).toBeNull();
    expect(result?.textContent).toContain('Search result');
    expect(input?.compareDocumentPosition(result as Node) & dom.window.Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    act(() => {
      container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(input?.textContent).toContain('"query": "BitFun"');
    expect(input?.textContent).toContain('"limit": 5');
    expect(input?.textContent).not.toContain('_early_detection');
    expect(container.querySelector('[data-bf-component="mcp-tool-display"]')?.getAttribute('data-bf-state')).toContain('expanded');
  });

  it('can expand a completed call that has parameters but no result content', () => {
    const item = toolItem({
      toolCall: {
        id: 'call-mcp-2',
        input: '{"query":"input only"}',
      },
      toolResult: {
        success: true,
        result: { content: [] },
      },
    });

    act(() => {
      root.render(<MCPToolDisplay toolItem={item} config={config} />);
    });

    const toggle = container.querySelector<HTMLButtonElement>('.preview-toggle-btn');
    expect(toggle).not.toBeNull();

    act(() => {
      toggle?.dispatchEvent(new dom.window.MouseEvent('click', { bubbles: true }));
    });

    expect(container.querySelector('.mcp-input-code')).toBeNull();

    act(() => {
      container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(container.querySelector('.mcp-input-code')?.textContent).toContain('"query": "input only"');
  });

  it('keeps failed calls collapsed until the user expands their parameters', () => {
    const item = toolItem({
      status: 'error',
      toolResult: {
        success: false,
        result: null,
        error: 'MCP server rejected the request',
      },
    });

    act(() => {
      root.render(<MCPToolDisplay toolItem={item} config={config} />);
    });

    expect(container.querySelector('.mcp-input-code')).toBeNull();
    expect(container.textContent).toContain('MCP server rejected the request');
    expect(container.querySelector('[data-bf-component="mcp-tool-display"]')?.getAttribute('data-bf-state')).toBe('error');

    act(() => {
      container.querySelector<HTMLButtonElement>('.preview-toggle-btn')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(container.querySelector('[data-bf-component="mcp-tool-display"]')?.getAttribute('data-bf-state')).toContain('expanded');
    expect(container.querySelector('.mcp-input-code')).toBeNull();

    act(() => {
      container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(container.querySelector('.mcp-input-code')?.textContent).toContain('"query": "BitFun"');
  });

  it('does not expose incomplete streaming parameters as expandable details', () => {
    const item = toolItem({
      status: 'streaming',
      isParamsStreaming: true,
      toolResult: undefined,
    });

    act(() => {
      root.render(<MCPToolDisplay toolItem={item} config={config} />);
    });

    expect(container.querySelector('.preview-toggle-btn')).toBeNull();
    expect(container.querySelector('.mcp-input-code')).toBeNull();
  });

  it('auto-expands an MCP App with input collapsed and resets input when the parent closes', async () => {
    const resourceUri = 'ui://example/search';
    mcpMocks.getCachedToolInfo.mockReset().mockResolvedValue({
      dynamic_info: {
        mcp: {
          serverId: 'example',
          toolName: 'search',
        },
      },
    });
    mcpMocks.fetchMCPAppResource.mockResolvedValue({
      contents: [{
        uri: resourceUri,
        content: '<!doctype html><html><body>MCP App</body></html>',
      }],
    });

    const item = toolItem({
      toolResult: {
        success: true,
        result: {
          content: [{
            type: 'resource',
            resource: { uri: resourceUri },
          }],
        },
      },
    });

    await act(async () => {
      root.render(<MCPToolDisplay toolItem={item} config={config} />);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const cardRoot = container.querySelector('[data-bf-component="mcp-tool-display"]');
    expect(container.querySelector('.mcp-app-iframe')).not.toBeNull();
    expect(cardRoot?.getAttribute('data-bf-state')).toContain('expanded');
    expect(container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.mcp-input-code')).toBeNull();

    act(() => {
      container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(cardRoot?.getAttribute('data-bf-state')).toContain('expanded');
    expect(container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.getAttribute('aria-expanded')).toBe('true');
    expect(container.querySelector('.mcp-input-code')).not.toBeNull();

    act(() => {
      container.querySelector<HTMLButtonElement>('.preview-toggle-btn')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(cardRoot?.getAttribute('data-bf-state') ?? '').not.toContain('expanded');

    act(() => {
      container.querySelector<HTMLButtonElement>('.preview-toggle-btn')?.dispatchEvent(
        new dom.window.MouseEvent('click', { bubbles: true })
      );
    });

    expect(cardRoot?.getAttribute('data-bf-state')).toContain('expanded');
    expect(container.querySelector<HTMLButtonElement>('.mcp-input-toggle')?.getAttribute('aria-expanded')).toBe('false');
    expect(container.querySelector('.mcp-input-code')).toBeNull();
  });
});
