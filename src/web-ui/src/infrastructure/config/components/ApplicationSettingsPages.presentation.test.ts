import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const source = readFileSync(
  fileURLToPath(new URL('./ApplicationSettingsPages.tsx', import.meta.url)),
  'utf8',
);

describe('Application settings presentation', () => {
  it('groups general settings by task instead of rendering one section per row', () => {
    expect(source).toContain("applicationGroups.startupAndUpdates.title");
    expect(source).toContain("applicationGroups.windowAndNotifications.title");
    expect(source).toContain('<LaunchAtLoginSetting />');
    expect(source).toContain('<PreventSleepSetting />');
    expect(source).toContain('<AutoUpdateSetting />');
    expect(source).toContain('<WindowBehaviorSetting />');
    expect(source).toContain('<NotificationSettings />');
    expect(source).not.toContain('function LaunchAtLoginSection');
    expect(source).not.toContain('function NotificationsSection');
  });

  it('does not render an empty desktop-only startup group on web', () => {
    expect(source).toContain('{isTauri && (');
  });
});
