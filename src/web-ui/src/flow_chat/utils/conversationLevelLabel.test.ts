import { describe, expect, it } from 'vitest';
import { conversationLevelLabel } from './conversationLevelLabel';

/** Minimal react-i18next-style t stub backed by the real zh-CN flow-chat resources. */
const zhCnFlowChat = {
  'chatInput.conversationLevel.main': '主会话',
  'chatInput.conversationLevel.child': '子会话',
  'chatInput.conversationLevel.senior': '士官',
  'chatInput.conversationLevel.childWithSeq': '子会话 {{seq}}',
} as const;

const t = (key: string, options?: Record<string, unknown>): string => {
  const template = (zhCnFlowChat as Record<string, string>)[key] ?? key;
  return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (_, name: string) =>
    String(options?.[name] ?? ''),
  );
};

describe('conversationLevelLabel', () => {
  it('resolves ancestor chain depth 0 to the main label', () => {
    expect(conversationLevelLabel({ level: 0, isDescendant: false }, t)).toBe('主会话');
  });

  it('resolves ancestor chain depth 1-3 to the child label', () => {
    expect(conversationLevelLabel({ level: 1, isDescendant: false }, t)).toBe('子会话');
    expect(conversationLevelLabel({ level: 2, isDescendant: false }, t)).toBe('子会话');
    expect(conversationLevelLabel({ level: 3, isDescendant: false }, t)).toBe('子会话');
  });

  it('resolves ancestor chain depth 4 and beyond to the senior label', () => {
    expect(conversationLevelLabel({ level: 4, isDescendant: false }, t)).toBe('士官');
    expect(conversationLevelLabel({ level: 10, isDescendant: false }, t)).toBe('士官');
  });

  it('resolves descendant entries to the child label plus a peer sequence number', () => {
    expect(conversationLevelLabel({ level: 1, isDescendant: true }, t)).toBe('子会话 1');
    expect(conversationLevelLabel({ level: 2, isDescendant: true }, t)).toBe('子会话 2');
    expect(conversationLevelLabel({ level: 3, isDescendant: true }, t)).toBe('子会话 3');
  });

  it('keeps the raw label for an impossible negative ancestor level', () => {
    expect(conversationLevelLabel({ level: -1, isDescendant: false }, t)).toBe('L-1');
  });
});

describe('conversationLevelLabel with real i18next interpolation', () => {
  it('interpolates {{seq}} through the t function', () => {
    let capturedKey = '';
    let capturedOptions: Record<string, unknown> | undefined;
    const capturingT = (key: string, options?: Record<string, unknown>) => {
      capturedKey = key;
      capturedOptions = options;
      return `interpolated:${options?.seq}`;
    };

    const result = conversationLevelLabel({ level: 7, isDescendant: true }, capturingT);
    expect(capturedKey).toBe('chatInput.conversationLevel.childWithSeq');
    expect(capturedOptions).toEqual({ seq: 7 });
    expect(result).toBe('interpolated:7');
  });
});
