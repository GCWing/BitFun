import { DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL, DEFAULT_REVIEW_TEAM_MODEL } from './defaults';
import { getReviewStrategyProfile } from './strategy';
import type {
  ReviewModelFallbackReason,
  ReviewRoleDirectiveKey,
  ReviewStrategyLevel,
  ReviewTeamManifestMember,
  ReviewTeamMember,
} from './types';

// Centralizes member-to-manifest projection so strategy overrides and model
// fallback semantics stay identical across prompt blocks and work packets.
export function toManifestMember(
  member: ReviewTeamMember,
  reason?: ReviewTeamManifestMember['reason'],
): ReviewTeamManifestMember {
  const strategyProfile = getReviewStrategyProfile(member.strategyLevel);
  const roleDirective =
    strategyProfile.roleDirectives[member.subagentId as ReviewRoleDirectiveKey];
  return {
    subagentId: member.subagentId,
    displayName: member.displayName,
    roleName: member.roleName,
    model: member.model || DEFAULT_REVIEW_TEAM_MODEL,
    configuredModel: member.configuredModel || member.model || DEFAULT_REVIEW_TEAM_MODEL,
    modelFallbackReason: member.modelFallbackReason,
    defaultModelSlot: member.defaultModelSlot ?? strategyProfile.defaultModelSlot,
    strategyLevel: member.strategyLevel,
    strategySource: member.strategySource,
    strategyDirective:
      member.strategyDirective || roleDirective || strategyProfile.promptDirective,
    locked: member.locked,
    source: member.source,
    subagentSource: member.subagentSource,
    ...(reason ? { reason } : {}),
  };
}

function resolveManifestMemberModelForStrategy(
  member: ReviewTeamMember,
  strategyLevel: ReviewStrategyLevel,
): {
  model: string;
  configuredModel: string;
  modelFallbackReason?: ReviewModelFallbackReason;
} {
  const strategyProfile = getReviewStrategyProfile(strategyLevel);

  if (member.modelFallbackReason === 'model_removed') {
    return {
      model: strategyProfile.defaultModelSlot,
      configuredModel: member.configuredModel,
      modelFallbackReason: member.modelFallbackReason,
    };
  }

  const configuredModel =
    member.configuredModel?.trim() || member.model?.trim() || DEFAULT_REVIEW_TEAM_MODEL;
  if (
    !configuredModel ||
    configuredModel === 'fast' ||
    configuredModel === 'primary'
  ) {
    return {
      model: strategyProfile.defaultModelSlot,
      configuredModel: configuredModel || strategyProfile.defaultModelSlot,
    };
  }

  return {
    model: configuredModel,
    configuredModel,
  };
}

export function applyTeamStrategyOverrideToMember(
  member: ReviewTeamMember,
  strategyLevel: ReviewStrategyLevel,
): ReviewTeamMember {
  if (member.strategySource === 'member' || member.strategyLevel === strategyLevel) {
    return member;
  }

  const strategyProfile = getReviewStrategyProfile(strategyLevel);
  const model = resolveManifestMemberModelForStrategy(member, strategyLevel);
  return {
    ...member,
    model: model.model,
    configuredModel: model.configuredModel,
    modelFallbackReason: model.modelFallbackReason,
    strategyOverride: DEFAULT_REVIEW_MEMBER_STRATEGY_LEVEL,
    strategyLevel,
    strategySource: 'team',
    defaultModelSlot: strategyProfile.defaultModelSlot,
    strategyDirective:
      strategyProfile.roleDirectives[member.subagentId as ReviewRoleDirectiveKey] ||
      strategyProfile.promptDirective,
  };
}
