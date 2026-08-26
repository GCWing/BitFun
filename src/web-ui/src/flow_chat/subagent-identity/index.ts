export { SubagentAvatar } from './SubagentAvatar';
export {
  resolveSubagentAvatarColor,
  resolveSubagentAvatarId,
  resolveSubagentAvatarPresentation,
  type SubagentAvatarColor,
  type SubagentAvatarPresentation,
} from './avatarResolver';
export {
  getSubagentAvatarDefinition,
  getSubagentNameDefinition,
  SUBAGENT_AVATAR_CATALOG,
  SUBAGENT_AVATAR_COLOR_CATALOG,
  SUBAGENT_NAME_CATALOG,
  type SubagentAvatarColorId,
  type SubagentAvatarId,
  type SubagentNameId,
} from './catalog';
export {
  reconcileSubagentIdentityAssignments,
  type SubagentIdentityAssignment,
  type SubagentIdentityAssignments,
  type SubagentIdentitySubject,
} from './allocator';
export {
  collectSubagentIdentitySubjects,
  getSubagentIdentity,
  reconcileSubagentIdentitiesFromFlowState,
  resolveSubagentIdentityRootSessionId,
  useSubagentIdentityStore,
} from './store';
export { useSubagentIdentity } from './useSubagentIdentity';
