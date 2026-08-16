import { describe, expect, it } from 'vitest';
import {
  isActionableSourceDiagnostic,
  sourceDiagnosticCategory,
} from './presentation';

describe('external source diagnostic presentation', () => {
  it('keeps unknown warnings visible through the safe generic category', () => {
    expect(isActionableSourceDiagnostic({
      severity: 'warning',
      code: 'future_provider.unknown_condition',
      message: 'Opaque provider detail.',
    })).toBe(true);
    expect(sourceDiagnosticCategory('future_provider.unknown_condition'))
      .toBe('sourceIssue');
  });

  it('hides informational diagnostics that do not require user action', () => {
    expect(isActionableSourceDiagnostic({
      severity: 'info',
      code: 'future_provider.discovered',
      message: 'Discovery completed.',
    })).toBe(false);
  });

  it('gives an unavailable MCP runtime a concrete user category', () => {
    expect(sourceDiagnosticCategory('external_mcp.runtime_unavailable'))
      .toBe('runtimeUnavailable');
  });
});
