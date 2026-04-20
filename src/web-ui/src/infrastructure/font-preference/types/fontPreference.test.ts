import { describe, expect, it } from 'vitest';
import {
  deriveFontSizeTokens,
  resolveFontSizeTokens,
  resolveFlowChatFontSizeTokens,
  DEFAULT_FONT_PREFERENCE,
  PRESET_UI_BASE_PX,
} from './index';

describe('deriveFontSizeTokens', () => {
  it('returns correct token ladder for default base (14px)', () => {
    const tokens = deriveFontSizeTokens(14);
    expect(tokens.base).toBe('14px');
    expect(tokens.sm).toBe('13px');
    expect(tokens.xs).toBe('12px');
    expect(tokens.lg).toBe('15px');
    expect(tokens.xl).toBe('16px');
    expect(tokens['2xl']).toBe('18px');
  });

  it('clamps below minimum (12px)', () => {
    const tokens = deriveFontSizeTokens(8);
    expect(tokens.base).toBe('12px');
  });

  it('clamps above maximum (20px)', () => {
    const tokens = deriveFontSizeTokens(24);
    expect(tokens.base).toBe('20px');
  });
});

describe('resolveFontSizeTokens', () => {
  it('returns preset tokens for named levels', () => {
    const tokens = resolveFontSizeTokens({ level: 'default' });
    expect(tokens.base).toBe(`${PRESET_UI_BASE_PX.default}px`);
  });

  it('derives tokens for custom level', () => {
    const tokens = resolveFontSizeTokens({ level: 'custom', customPx: 16 });
    expect(tokens.base).toBe('16px');
  });

  it('falls back to 14px when custom has no customPx', () => {
    const tokens = resolveFontSizeTokens({ level: 'custom' });
    expect(tokens.base).toBe('14px');
  });
});

describe('resolveFlowChatFontSizeTokens', () => {
  it('lift mode now matches UI base (previously +1)', () => {
    const pref = { ...DEFAULT_FONT_PREFERENCE, flowChat: { mode: 'lift' as const } };
    const uiBase = PRESET_UI_BASE_PX[pref.uiSize.level as Exclude<typeof pref.uiSize.level, 'custom'>];
    const tokens = resolveFlowChatFontSizeTokens(pref);
    expect(tokens.base).toBe(`${Math.min(20, uiBase)}px`);
  });

  it('sync mode matches UI tokens exactly', () => {
    const pref = { uiSize: { level: 'default' as const }, flowChat: { mode: 'sync' as const } };
    const tokens = resolveFlowChatFontSizeTokens(pref);
    expect(tokens.base).toBe(`${PRESET_UI_BASE_PX.default}px`);
  });

  it('independent mode uses custom basePx', () => {
    const pref = { uiSize: { level: 'default' as const }, flowChat: { mode: 'independent' as const, basePx: 18 } };
    const tokens = resolveFlowChatFontSizeTokens(pref);
    expect(tokens.base).toBe('18px');
  });
});
