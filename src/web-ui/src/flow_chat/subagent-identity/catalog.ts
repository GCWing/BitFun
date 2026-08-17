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

export const SUBAGENT_IDENTITY_CATALOG_VERSION = 'subagent-identity-v1';

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

export const SUBAGENT_NAME_CATALOG = [
  { id: 'name-01', labelKey: 'subagentIdentity.names.name01', fallback: 'Starbit' },
  { id: 'name-02', labelKey: 'subagentIdentity.names.name02', fallback: 'Moonbud' },
  { id: 'name-03', labelKey: 'subagentIdentity.names.name03', fallback: 'Cloudlet' },
  { id: 'name-04', labelKey: 'subagentIdentity.names.name04', fallback: 'Glimmer' },
  { id: 'name-05', labelKey: 'subagentIdentity.names.name05', fallback: 'Aurora' },
  { id: 'name-06', labelKey: 'subagentIdentity.names.name06', fallback: 'Mochi' },
  { id: 'name-07', labelKey: 'subagentIdentity.names.name07', fallback: 'Pudding' },
  { id: 'name-08', labelKey: 'subagentIdentity.names.name08', fallback: 'Gummy' },
  { id: 'name-09', labelKey: 'subagentIdentity.names.name09', fallback: 'Puff' },
  { id: 'name-10', labelKey: 'subagentIdentity.names.name10', fallback: 'Bunbun' },
  { id: 'name-11', labelKey: 'subagentIdentity.names.name11', fallback: 'Bubbles' },
  { id: 'name-12', labelKey: 'subagentIdentity.names.name12', fallback: 'Dottie' },
  { id: 'name-13', labelKey: 'subagentIdentity.names.name13', fallback: 'Clicky' },
  { id: 'name-14', labelKey: 'subagentIdentity.names.name14', fallback: 'Tocky' },
  { id: 'name-15', labelKey: 'subagentIdentity.names.name15', fallback: 'Chirpy' },
  { id: 'name-16', labelKey: 'subagentIdentity.names.name16', fallback: 'Minty' },
  { id: 'name-17', labelKey: 'subagentIdentity.names.name17', fallback: 'Hazel' },
  { id: 'name-18', labelKey: 'subagentIdentity.names.name18', fallback: 'Berry' },
  { id: 'name-19', labelKey: 'subagentIdentity.names.name19', fallback: 'Peachy' },
  { id: 'name-20', labelKey: 'subagentIdentity.names.name20', fallback: 'Yuzu' },
  { id: 'name-21', labelKey: 'subagentIdentity.names.name21', fallback: 'Pixel' },
  { id: 'name-22', labelKey: 'subagentIdentity.names.name22', fallback: 'Coglet' },
  { id: 'name-23', labelKey: 'subagentIdentity.names.name23', fallback: 'Sparky' },
  { id: 'name-24', labelKey: 'subagentIdentity.names.name24', fallback: 'Blinky' },
  { id: 'name-25', labelKey: 'subagentIdentity.names.name25', fallback: 'Wavy' },
  { id: 'name-26', labelKey: 'subagentIdentity.names.name26', fallback: 'Pompom' },
  { id: 'name-27', labelKey: 'subagentIdentity.names.name27', fallback: 'Rollo' },
  { id: 'name-28', labelKey: 'subagentIdentity.names.name28', fallback: 'Fluffy' },
  { id: 'name-29', labelKey: 'subagentIdentity.names.name29', fallback: 'Jolly' },
  { id: 'name-30', labelKey: 'subagentIdentity.names.name30', fallback: 'Mellow' },
] as const;

export type SubagentAvatarId = typeof SUBAGENT_AVATAR_CATALOG[number]['id'];
export type SubagentNameId = typeof SUBAGENT_NAME_CATALOG[number]['id'];

export const SUBAGENT_AVATAR_IDS = SUBAGENT_AVATAR_CATALOG.map(item => item.id);
export const SUBAGENT_NAME_IDS = SUBAGENT_NAME_CATALOG.map(item => item.id);

const avatarById = new Map<SubagentAvatarId, typeof SUBAGENT_AVATAR_CATALOG[number]>(
  SUBAGENT_AVATAR_CATALOG.map(item => [item.id, item]),
);
const nameById = new Map<SubagentNameId, typeof SUBAGENT_NAME_CATALOG[number]>(
  SUBAGENT_NAME_CATALOG.map(item => [item.id, item]),
);

export function getSubagentAvatarDefinition(id: SubagentAvatarId) {
  return avatarById.get(id) ?? SUBAGENT_AVATAR_CATALOG[0];
}

export function getSubagentNameDefinition(id: SubagentNameId) {
  return nameById.get(id) ?? SUBAGENT_NAME_CATALOG[0];
}
