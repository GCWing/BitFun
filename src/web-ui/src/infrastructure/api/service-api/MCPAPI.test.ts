import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MCPAPI } from './MCPAPI';

const invokeMock = vi.hoisted(() => vi.fn());
const scopeMock = vi.hoisted(() => ({ assertCurrent: vi.fn() }));
vi.mock('./ApiClient', () => ({ api: { invoke: invokeMock } }));
vi.mock('@/infrastructure/peer-device/deviceSurface', () => ({
  getActiveSurfaceScope: () => scopeMock,
}));

describe('MCP JSON save acknowledgement', () => {
  beforeEach(() => { invokeMock.mockReset(); scopeMock.assertCurrent.mockReset(); });

  it('accepts the existing void success response', async () => {
    invokeMock.mockResolvedValue(undefined);
    await expect(MCPAPI.saveMCPJsonConfig('{}', 'revision')).resolves.toEqual({ runtimeApplied: true });
    expect(invokeMock).toHaveBeenCalledWith('save_mcp_json_config', {
      jsonConfig: '{}', expectedFingerprint: 'revision',
    });
  });

  it.each([
    'MCP config was saved, but runtime reconciliation failed: connection refused',
    new Error('MCP config was saved, but runtime reconciliation failed: connection refused'),
  ])('recognizes an explicit persisted acknowledgement: %s', async (error) => {
    invokeMock.mockRejectedValue(error);
    await expect(MCPAPI.saveMCPJsonConfig('{}', 'revision')).resolves.toEqual({ runtimeApplied: false });
  });

  it.each([
    'Failed to save config: permission denied',
    'MCP configuration changed; reload before saving',
    new Error('Request timeout: save_mcp_json_config'),
    'connection refused',
  ])('does not assume an unacknowledged write succeeded: %s', async (error) => {
    invokeMock.mockRejectedValue(error);
    await expect(MCPAPI.saveMCPJsonConfig('{}', 'revision')).rejects.toBe(error);
  });

  it('confirms a timed-out save by reading back the same JSON regardless of key order', async () => {
    const error = Object.assign(new Error('Request timeout'), { code: 'REQUEST_TIMEOUT' });
    invokeMock.mockRejectedValueOnce(error).mockResolvedValueOnce({
      jsonConfig: '{"mcpServers":{"offline":{"autoStart":true,"args":["a","b"]}}}',
      fingerprint: 'saved',
    });
    await expect(MCPAPI.saveMCPJsonConfig(
      '{"mcpServers":{"offline":{"args":["a","b"],"autoStart":true}}}', 'revision',
    )).resolves.toEqual({ runtimeApplied: false });
    expect(invokeMock.mock.calls.map(call => call[0])).toEqual(['save_mcp_json_config', 'load_mcp_json_config']);
  });

  it.each([
    { jsonConfig: '{"mcpServers":{"offline":{"args":["b","a"]}}}', fingerprint: 'other' },
    null,
  ])('retains a timeout when read-back cannot confirm the requested content: %s', async (snapshot) => {
    const error = Object.assign(new Error('Request timeout'), { code: 'REQUEST_TIMEOUT' });
    invokeMock.mockRejectedValueOnce(error);
    if (snapshot) invokeMock.mockResolvedValueOnce(snapshot);
    else invokeMock.mockRejectedValueOnce(new Error('Host disconnected'));
    await expect(MCPAPI.saveMCPJsonConfig(
      '{"mcpServers":{"offline":{"args":["a","b"]}}}', 'revision',
    )).rejects.toBe(error);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('does not read configuration from a newly selected device after a timeout', async () => {
    const changed = new Error('Surface changed');
    scopeMock.assertCurrent.mockImplementation(() => { throw changed; });
    invokeMock.mockRejectedValueOnce(Object.assign(new Error('Request timeout'), { code: 'REQUEST_TIMEOUT' }));
    await expect(MCPAPI.saveMCPJsonConfig('{}', 'revision')).rejects.toBe(changed);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
