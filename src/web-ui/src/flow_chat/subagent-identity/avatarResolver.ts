import {
  SUBAGENT_AVATAR_COLOR_CATALOG,
  SUBAGENT_AVATAR_COLOR_CATALOG_VERSION,
  SUBAGENT_AVATAR_IDS,
  SUBAGENT_AVATAR_CATALOG_VERSION,
  type SubagentAvatarColorId,
  type SubagentAvatarId,
} from './catalog';

export interface SubagentAvatarColor {
  colorId: SubagentAvatarColorId;
  hueShiftDegrees: number;
}

export interface SubagentAvatarPresentation extends SubagentAvatarColor {
  avatarId: SubagentAvatarId;
}

function hashString(value: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/**
 * Resolve the Web UI avatar directly from the stable subagent session ID.
 *
 * The mapping deliberately does not depend on lineage hydration, active state,
 * allocation order, or persisted frontend state. Avatar collisions are allowed.
 */
export function resolveSubagentAvatarId(sessionId: string): SubagentAvatarId {
  const normalizedSessionId = sessionId.trim();
  if (!normalizedSessionId) {
    return SUBAGENT_AVATAR_IDS[0];
  }

  const hash = hashString(
    `${SUBAGENT_AVATAR_CATALOG_VERSION}:avatar:${normalizedSessionId}`,
  );
  return SUBAGENT_AVATAR_IDS[hash % SUBAGENT_AVATAR_IDS.length];
}

/** Resolve a stable color independently from the avatar shape hash. */
export function resolveSubagentAvatarColor(sessionId: string): SubagentAvatarColor {
  const normalizedSessionId = sessionId.trim();
  const color = normalizedSessionId
    ? SUBAGENT_AVATAR_COLOR_CATALOG[
      hashString(
        `${SUBAGENT_AVATAR_COLOR_CATALOG_VERSION}:color:${normalizedSessionId}`,
      ) % SUBAGENT_AVATAR_COLOR_CATALOG.length
    ]
    : SUBAGENT_AVATAR_COLOR_CATALOG[0];

  return {
    colorId: color.id,
    hueShiftDegrees: color.hueShiftDegrees,
  };
}

export function resolveSubagentAvatarPresentation(
  sessionId: string,
): SubagentAvatarPresentation {
  return {
    avatarId: resolveSubagentAvatarId(sessionId),
    ...resolveSubagentAvatarColor(sessionId),
  };
}
