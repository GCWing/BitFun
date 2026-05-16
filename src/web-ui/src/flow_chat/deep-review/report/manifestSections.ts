import {
  getActiveReviewTeamManifestMembers,
  type ReviewTeamManifestMember,
  type ReviewTeamRunManifest,
} from '@/shared/services/reviewTeamService';
import type { CodeReviewReportMarkdownLabels } from './codeReviewReport';

function pushList(lines: string[], items: string[], emptyLabel: string): void {
  if (items.length === 0) {
    lines.push(`- ${emptyLabel}`);
    return;
  }

  for (const item of items) {
    lines.push(`- ${item}`);
  }
}

function manifestTarget(manifest: ReviewTeamRunManifest): string {
  return manifest.target.tags.length > 0
    ? manifest.target.tags.join(', ')
    : manifest.target.source;
}

function manifestMemberLabel(member: ReviewTeamManifestMember): string {
  return member.displayName || member.subagentId;
}

function manifestMemberLine(member: ReviewTeamManifestMember): string {
  return `${manifestMemberLabel(member)} (${member.subagentId})`;
}

function pluralize(count: number, singular: string): string {
  return `${count} ${singular}${count === 1 ? '' : 's'}`;
}

function pushPreReviewSummarySection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const summary = manifest.preReviewSummary;
  if (!summary) {
    return;
  }

  lines.push(`### Pre-review summary`);
  lines.push(`- ${summary.summary}`);
  lines.push(`- Files: ${summary.fileCount}`);
  if (summary.lineCount !== undefined) {
    lines.push(`- Lines changed: ${summary.lineCount} (${summary.lineCountSource})`);
  } else {
    lines.push(`- Lines changed: unknown (${summary.lineCountSource})`);
  }
  if (summary.workspaceAreas.length > 0) {
    for (const area of summary.workspaceAreas) {
      const sampleFiles = area.sampleFiles.length > 0
        ? ` (${area.sampleFiles.join(', ')})`
        : '';
      lines.push(`- ${area.key}: ${pluralize(area.fileCount, 'file')}${sampleFiles}`);
    }
  }
  lines.push('');
}

function pushEvidencePackSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const pack = manifest.evidencePack;
  if (!pack) {
    return;
  }

  lines.push(`### Evidence pack`);
  lines.push(`- Source: ${pack.source}; privacy: ${pack.privacy.content}`);
  lines.push(
    `- Changed files: ${pack.changedFiles.length}; hunk hints: ${pack.hunkHints.length}; contract hints: ${pack.contractHints.length}; packet ids: ${pack.packetIds.length}`,
  );
  lines.push(
    `- Omitted metadata: changed files ${pack.budget.omittedChangedFileCount}, hunk hints ${pack.budget.omittedHunkHintCount}, contract hints ${pack.budget.omittedContractHintCount}`,
  );
  lines.push('- Hints are orientation only and require tool confirmation before findings.');
  lines.push('');
}

function pushSharedContextCacheSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const cachePlan = manifest.sharedContextCache;
  if (!cachePlan) {
    return;
  }

  lines.push(`### Shared context cache`);
  if (cachePlan.entries.length === 0) {
    lines.push('- None.');
  } else {
    for (const entry of cachePlan.entries) {
      lines.push(
        `- ${entry.cacheKey}: ${entry.path} -> ${entry.consumerPacketIds.join(', ')}`,
      );
    }
  }
  if (cachePlan.omittedEntryCount > 0) {
    lines.push(`- Omitted entries: ${cachePlan.omittedEntryCount}`);
  }
  lines.push('');
}

function pushIncrementalReviewCacheSection(
  lines: string[],
  manifest: ReviewTeamRunManifest,
): void {
  const cachePlan = manifest.incrementalReviewCache;
  if (!cachePlan) {
    return;
  }

  lines.push(`### Incremental review cache`);
  lines.push(`- Cache key: ${cachePlan.cacheKey}`);
  lines.push(`- Fingerprint: ${cachePlan.fingerprint}`);
  lines.push(`- Strategy: ${cachePlan.strategy}`);
  lines.push(`- Reviewer packets: ${cachePlan.reviewerPacketIds.join(', ') || 'none'}`);
  lines.push(`- Invalidates on: ${cachePlan.invalidatesOn.join(', ') || 'none'}`);
  lines.push('');
}

export function formatRunManifestMarkdownSection(
  manifest: ReviewTeamRunManifest,
  labels: CodeReviewReportMarkdownLabels,
): string {
  const lines: string[] = [];
  const activeReviewers = getActiveReviewTeamManifestMembers(manifest);

  lines.push(`## ${labels.runManifest}`);
  lines.push(`- ${labels.target}: ${manifestTarget(manifest)}`);
  lines.push(`- ${labels.budget}: ${manifest.tokenBudget.mode}`);
  lines.push(`- ${labels.estimatedCalls}: ${manifest.tokenBudget.estimatedReviewerCalls}`);
  if (manifest.scopeProfile) {
    lines.push(`- Review depth: ${manifest.scopeProfile.reviewDepth}`);
    lines.push(`- Coverage expectation: ${manifest.scopeProfile.coverageExpectation}`);
  }
  if (manifest.strategyRecommendation) {
    lines.push(`- Recommended strategy: ${manifest.strategyRecommendation.strategyLevel}`);
    lines.push(`- Recommendation score: ${manifest.strategyRecommendation.score}`);
    lines.push(`- Recommendation rationale: ${manifest.strategyRecommendation.rationale}`);
  }
  lines.push('');
  lines.push(`### ${labels.activeReviewers}`);
  pushList(
    lines,
    activeReviewers.map((member) => manifestMemberLine(member)),
    labels.noItems,
  );
  lines.push('');
  lines.push(`### ${labels.skippedReviewers}`);
  pushList(
    lines,
    manifest.skippedReviewers.map((member) =>
      `${manifestMemberLine(member)}: ${member.reason ?? 'skipped'}`,
    ),
    labels.noItems,
  );
  lines.push('');
  pushPreReviewSummarySection(lines, manifest);
  pushEvidencePackSection(lines, manifest);
  pushSharedContextCacheSection(lines, manifest);
  pushIncrementalReviewCacheSection(lines, manifest);

  return lines.join('\n').trimEnd();
}
