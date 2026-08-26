import robot01 from '../assets/subagent-avatars/robot-01.webp';
import robot02 from '../assets/subagent-avatars/robot-02.webp';
import robot03 from '../assets/subagent-avatars/robot-03.webp';
import robot04 from '../assets/subagent-avatars/robot-04.webp';
import robot05 from '../assets/subagent-avatars/robot-05.webp';
import robot06 from '../assets/subagent-avatars/robot-06.webp';
import robot07 from '../assets/subagent-avatars/robot-07.webp';
import robot08 from '../assets/subagent-avatars/robot-08.webp';
import robot09 from '../assets/subagent-avatars/robot-09.webp';
import robot10 from '../assets/subagent-avatars/robot-10.webp';
import robot11 from '../assets/subagent-avatars/robot-11.webp';
import robot12 from '../assets/subagent-avatars/robot-12.webp';
import robot13 from '../assets/subagent-avatars/robot-13.webp';
import robot14 from '../assets/subagent-avatars/robot-14.webp';
import robot15 from '../assets/subagent-avatars/robot-15.webp';

// Keep the original seed so removing frontend-only names does not reshuffle avatars.
export const SUBAGENT_AVATAR_CATALOG_VERSION = 'subagent-identity-v1';
export const SUBAGENT_AVATAR_COLOR_CATALOG_VERSION = 'subagent-avatar-color-v1';

export const SUBAGENT_AVATAR_CATALOG = [
  { id: 'robot-01', src: robot01 },
  { id: 'robot-02', src: robot02 },
  { id: 'robot-03', src: robot03 },
  { id: 'robot-04', src: robot04 },
  { id: 'robot-05', src: robot05 },
  { id: 'robot-06', src: robot06 },
  { id: 'robot-07', src: robot07 },
  { id: 'robot-08', src: robot08 },
  { id: 'robot-09', src: robot09 },
  { id: 'robot-10', src: robot10 },
  { id: 'robot-11', src: robot11 },
  { id: 'robot-12', src: robot12 },
  { id: 'robot-13', src: robot13 },
  { id: 'robot-14', src: robot14 },
  { id: 'robot-15', src: robot15 },
] as const;

// The order is part of the session-to-color mapping contract. Bump
// SUBAGENT_AVATAR_COLOR_CATALOG_VERSION before changing this catalog.
export const SUBAGENT_AVATAR_COLOR_CATALOG = [
  { id: 'cyan', hueShiftDegrees: 0 },
  { id: 'azure', hueShiftDegrees: 30 },
  { id: 'indigo', hueShiftDegrees: 60 },
  { id: 'violet', hueShiftDegrees: 90 },
  { id: 'magenta', hueShiftDegrees: 120 },
  { id: 'rose', hueShiftDegrees: 150 },
  { id: 'red', hueShiftDegrees: 180 },
  { id: 'orange', hueShiftDegrees: 210 },
  { id: 'amber', hueShiftDegrees: 240 },
  { id: 'lime', hueShiftDegrees: 270 },
  { id: 'green', hueShiftDegrees: 300 },
  { id: 'teal', hueShiftDegrees: 330 },
] as const;

export type SubagentAvatarId = typeof SUBAGENT_AVATAR_CATALOG[number]['id'];
export type SubagentAvatarColorId = typeof SUBAGENT_AVATAR_COLOR_CATALOG[number]['id'];

export const SUBAGENT_AVATAR_IDS = SUBAGENT_AVATAR_CATALOG.map(item => item.id);
export const SUBAGENT_AVATAR_COLOR_IDS = SUBAGENT_AVATAR_COLOR_CATALOG.map(item => item.id);

const avatarById = new Map<SubagentAvatarId, typeof SUBAGENT_AVATAR_CATALOG[number]>(
  SUBAGENT_AVATAR_CATALOG.map(item => [item.id, item]),
);

export function getSubagentAvatarDefinition(id: SubagentAvatarId) {
  return avatarById.get(id) ?? SUBAGENT_AVATAR_CATALOG[0];
}
