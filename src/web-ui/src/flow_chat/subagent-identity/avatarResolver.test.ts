import { describe, expect, it } from 'vitest';
import {
  SUBAGENT_AVATAR_COLOR_CATALOG,
  SUBAGENT_AVATAR_IDS,
} from './catalog';
import {
  resolveSubagentAvatarColor,
  resolveSubagentAvatarId,
  resolveSubagentAvatarPresentation,
} from './avatarResolver';

describe('subagent avatar resolver', () => {
  it('maps a session ID to the same catalog avatar without stored state', () => {
    const first = resolveSubagentAvatarId('child-session');

    expect(resolveSubagentAvatarId('child-session')).toBe(first);
    expect(SUBAGENT_AVATAR_IDS).toContain(first);
  });

  it('uses the default avatar when the session ID is empty', () => {
    expect(resolveSubagentAvatarId('   ')).toBe(SUBAGENT_AVATAR_IDS[0]);
  });

  it('maps color through an independently salted stable catalog', () => {
    const first = resolveSubagentAvatarColor('child-session');

    expect(first).toEqual({ colorId: 'magenta', hueShiftDegrees: 120 });
    expect(resolveSubagentAvatarColor('child-session')).toEqual(first);
    expect(SUBAGENT_AVATAR_COLOR_CATALOG).toContainEqual({
      id: first.colorId,
      hueShiftDegrees: first.hueShiftDegrees,
    });
    expect(resolveSubagentAvatarPresentation('child-session')).toEqual({
      avatarId: resolveSubagentAvatarId('child-session'),
      ...first,
    });
  });

  it('uses the unrotated cyan color when the session ID is empty', () => {
    expect(resolveSubagentAvatarColor('   ')).toEqual({
      colorId: 'cyan',
      hueShiftDegrees: 0,
    });
  });
});
