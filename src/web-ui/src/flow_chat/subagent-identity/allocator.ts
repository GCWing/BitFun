import {
  SUBAGENT_IDENTITY_CATALOG_VERSION,
  SUBAGENT_NAME_IDS,
  type SubagentAvatarId,
  type SubagentNameId,
} from './catalog';
import { resolveSubagentAvatarId } from './avatarResolver';

export interface SubagentIdentitySubject {
  sessionId: string;
  createdAt: number;
  active: boolean;
}

export interface SubagentIdentityAssignment {
  rootSessionId: string;
  sessionId: string;
  avatarId: SubagentAvatarId;
  nameId: SubagentNameId;
}

export type SubagentIdentityAssignments = Record<string, SubagentIdentityAssignment>;

function hashString(value: string): number {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function seededShuffle<T>(values: readonly T[], seed: string): T[] {
  const shuffled = [...values];
  let state = hashString(seed) || 0x6d2b79f5;
  const random = () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };

  for (let index = shuffled.length - 1; index > 0; index -= 1) {
    const target = Math.floor(random() * (index + 1));
    [shuffled[index], shuffled[target]] = [shuffled[target], shuffled[index]];
  }
  return shuffled;
}

function normalizedSubjects(subjects: readonly SubagentIdentitySubject[]): SubagentIdentitySubject[] {
  const unique = new Map<string, SubagentIdentitySubject>();
  for (const subject of subjects) {
    if (!subject.sessionId.trim()) continue;
    unique.set(subject.sessionId, {
      ...subject,
      createdAt: Number.isFinite(subject.createdAt) ? subject.createdAt : 0,
    });
  }
  return [...unique.values()].sort((left, right) =>
    left.createdAt - right.createdAt || left.sessionId.localeCompare(right.sessionId)
  );
}

function chooseCatalogId<T extends string>(options: {
  pool: readonly T[];
  seed: string;
  rootSessionId: string;
  active: boolean;
  assignments: SubagentIdentityAssignments;
  activeSessionIds: ReadonlySet<string>;
  read: (assignment: SubagentIdentityAssignment) => T;
}): T {
  const candidates = seededShuffle(options.pool, options.seed);
  const rootAssignments = Object.values(options.assignments)
    .filter(assignment => assignment.rootSessionId === options.rootSessionId);
  const usedEver = new Set(rootAssignments.map(options.read));

  if (usedEver.size < options.pool.length) {
    return candidates.find(candidate => !usedEver.has(candidate)) ?? candidates[0];
  }

  if (options.active) {
    const usedByActive = new Set(
      rootAssignments
        .filter(assignment => options.activeSessionIds.has(assignment.sessionId))
        .map(options.read),
    );
    const freeCandidate = candidates.find(candidate => !usedByActive.has(candidate));
    if (freeCandidate) return freeCandidate;
  }

  const usage = new Map<T, number>(options.pool.map(id => [id, 0]));
  for (const assignment of rootAssignments) {
    const id = options.read(assignment);
    usage.set(id, (usage.get(id) ?? 0) + 1);
  }
  return candidates.reduce((best, candidate) =>
    (usage.get(candidate) ?? 0) < (usage.get(best) ?? 0) ? candidate : best
  , candidates[0]);
}

function repairActiveCollisions<T extends string>(options: {
  pool: readonly T[];
  salt: string;
  rootSessionId: string;
  subjects: readonly SubagentIdentitySubject[];
  assignments: SubagentIdentityAssignments;
  read: (assignment: SubagentIdentityAssignment) => T;
  write: (assignment: SubagentIdentityAssignment, value: T) => SubagentIdentityAssignment;
}): SubagentIdentityAssignments {
  const next = { ...options.assignments };
  const used = new Set<T>();

  for (const subject of options.subjects) {
    if (!subject.active) continue;
    const assignment = next[subject.sessionId];
    if (!assignment || assignment.rootSessionId !== options.rootSessionId) continue;
    const current = options.read(assignment);
    if (!used.has(current)) {
      used.add(current);
      continue;
    }

    const candidates = seededShuffle(
      options.pool,
      `${SUBAGENT_IDENTITY_CATALOG_VERSION}:${options.rootSessionId}:${options.salt}:${subject.sessionId}:repair`,
    );
    const replacement = candidates.find(candidate => !used.has(candidate));
    if (!replacement) continue;
    next[subject.sessionId] = options.write(assignment, replacement);
    used.add(replacement);
  }

  return next;
}

export function reconcileSubagentIdentityAssignments(
  rootSessionId: string,
  subjects: readonly SubagentIdentitySubject[],
  previous: SubagentIdentityAssignments = {},
): SubagentIdentityAssignments {
  const orderedSubjects = normalizedSubjects(subjects);
  const activeSessionIds = new Set(
    orderedSubjects.filter(subject => subject.active).map(subject => subject.sessionId),
  );
  let next = { ...previous };

  for (const subject of orderedSubjects) {
    const existing = next[subject.sessionId];
    const avatarId = resolveSubagentAvatarId(subject.sessionId);
    const existingNameIsValid = existing?.rootSessionId === rootSessionId
      && SUBAGENT_NAME_IDS.includes(existing.nameId);
    if (existingNameIsValid) {
      if (existing.avatarId !== avatarId) {
        next[subject.sessionId] = { ...existing, avatarId };
      }
      continue;
    }

    const nameId = chooseCatalogId({
      pool: SUBAGENT_NAME_IDS,
      seed: `${SUBAGENT_IDENTITY_CATALOG_VERSION}:${rootSessionId}:name:${subject.sessionId}`,
      rootSessionId,
      active: subject.active,
      assignments: next,
      activeSessionIds,
      read: assignment => assignment.nameId,
    });
    next[subject.sessionId] = {
      rootSessionId,
      sessionId: subject.sessionId,
      avatarId,
      nameId,
    };
  }

  return repairActiveCollisions({
    pool: SUBAGENT_NAME_IDS,
    salt: 'name',
    rootSessionId,
    subjects: orderedSubjects,
    assignments: next,
    read: assignment => assignment.nameId,
    write: (assignment, nameId) => ({ ...assignment, nameId }),
  });
}
