import { describe, expect, it } from 'vitest';
import {
  extractReviewSubjectCandidates,
  trimReviewUrlToken,
} from './subjectCandidates';

describe('Review subject candidate extraction', () => {
  it('extracts a GitHub issue without creating a workspace candidate', () => {
    const result = extractReviewSubjectCandidates(
      'https://github.com/org/repo/issues/42',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'issue',
        id: 'candidate-1',
        web_url: 'https://github.com/org/repo/issues/42',
        host: 'github.com',
        project_path: 'org/repo',
        issue_id: '42',
      },
    ]);
    expect(result.candidates.some((candidate) => candidate.kind === 'workspace')).toBe(false);
    expect(result.remainingFocus).toBe('');
    expect(result.unparsedFragments).toEqual([]);
  });

  it('preserves issue, pull request, and local files in textual order', () => {
    const result = extractReviewSubjectCandidates(
      'using https://github.com/org/repo/issues/42 compare https://github.com/org/repo/pull/57 with src/runtime',
      'D:/workspace/repo',
    );

    expect(result.candidates.map((candidate) => candidate.kind)).toEqual([
      'issue',
      'pull_request',
      'explicit_files',
    ]);
    expect(result.candidates.map((candidate) => candidate.id)).toEqual([
      'candidate-1',
      'candidate-2',
      'candidate-3',
    ]);
    expect(result.candidates[2]).toMatchObject({ paths: ['src/runtime'] });
    expect(result.remainingFocus).toBe('using compare with');
  });

  it('parses nested GitLab projects with query, fragment, and trailing slash variants', () => {
    const result = extractReviewSubjectCandidates(
      [
        'https://gitlab.com/group/subgroup/project/-/issues/7/?scope=all#note_1',
        'https://gitlab.com/group/subgroup/project/-/merge_requests/9#diffs',
      ].join(' '),
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        id: 'candidate-1',
        host: 'gitlab.com',
        project_path: 'group/subgroup/project',
        issue_id: '7',
      }),
      expect.objectContaining({
        kind: 'pull_request',
        id: 'candidate-2',
        host: 'gitlab.com',
        project_path: 'group/subgroup/project',
        pull_request_id: '9',
      }),
    ]);
  });

  it('deduplicates repeated URL subject identities while preserving first occurrence', () => {
    const focus = [
      'https://github.com/org/repo/issues/42?view=1#discussion',
      'https://docs.example.com/review-guide/',
      'https://github.com/org/repo/issues/42?view=1#discussion',
    ].join(' then ');

    const first = extractReviewSubjectCandidates(focus);
    const second = extractReviewSubjectCandidates(focus);

    expect(first.candidates.map((candidate) => candidate.kind)).toEqual([
      'issue',
      'external_reference',
    ]);
    expect(first.candidates.map((candidate) => candidate.id)).toEqual([
      'candidate-1',
      'candidate-2',
    ]);
    expect(second.candidates).toEqual(first.candidates);
  });

  it('does not interpret unknown URL path fragments as local files', () => {
    const result = extractReviewSubjectCandidates(
      'review https://example.com/compare/main..feature?file=src/runtime/file.ts#change',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'external_reference',
        id: 'candidate-1',
        url: 'https://example.com/compare/main..feature?file=src/runtime/file.ts#change',
      },
    ]);
    expect(result.remainingFocus).toBe('review');
  });

  it('does not interpret an unknown URL after commit as a Git ref', () => {
    const commitUrlResult = extractReviewSubjectCandidates(
      'commit https://example.com/releases/commit-notes',
      'D:/workspace/repo',
    );
    expect(commitUrlResult.candidates).toEqual([
      {
        kind: 'external_reference',
        id: 'candidate-1',
        url: 'https://example.com/releases/commit-notes',
      },
    ]);
  });

  it('preserves an explicit Windows path alongside a URL subject', () => {
    const result = extractReviewSubjectCandidates(
      'D:\\workspace\\repo\\src\\App.tsx and https://github.com/org/repo/pull/8',
      'D:\\workspace\\repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'explicit_files',
        id: 'candidate-1',
        paths: ['D:\\workspace\\repo\\src\\App.tsx'],
      },
      expect.objectContaining({
        kind: 'pull_request',
        id: 'candidate-2',
        pull_request_id: '8',
      }),
    ]);
  });

  it('keeps issue-shaped URLs on unknown hosts as external references', () => {
    const result = extractReviewSubjectCandidates(
      'src/first.ts https://unknown.example/team/project/issues/5 docs/second.md',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'explicit_files',
        id: 'candidate-1',
        paths: ['src/first.ts', 'docs/second.md'],
      },
      {
        kind: 'external_reference',
        id: 'candidate-2',
        url: 'https://unknown.example/team/project/issues/5',
      },
    ]);
  });

  it('extracts Git ranges through the command parser and retains prose as focus', () => {
    const result = extractReviewSubjectCandidates(
      'compare main..feature for authorization regressions',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'git_range',
        id: 'candidate-1',
        source_ref: 'main',
        target_ref: 'feature',
      },
    ]);
    expect(result.remainingFocus).toBe('compare for authorization regressions');
  });

  it('keeps Chinese prose and unresolved fragments out of target identity', () => {
    const result = extractReviewSubjectCandidates(
      '请重点检查身份验证流程 "config.custom"，不要扩大审查范围',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([]);
    expect(result.remainingFocus).toContain('请重点检查身份验证流程');
    expect(result.unparsedFragments).toEqual(['config.custom']);
  });

  it('does not add workspace identity when no workspace is available', () => {
    const result = extractReviewSubjectCandidates('只检查安全边界');

    expect(result.candidates).toEqual([]);
    expect(result.remainingFocus).toBe('只检查安全边界');
    expect(result.unparsedFragments).toEqual([]);
  });

  it('keeps unresolved explicit fragments fail-closed while ordinary prose uses workspace', () => {
    const unresolved = extractReviewSubjectCandidates(
      'inspect "config.custom" carefully',
      'D:/workspace/repo',
    );
    expect(unresolved.candidates).toEqual([]);
    expect(unresolved.remainingFocus).toBe('inspect "config.custom" carefully');
    expect(unresolved.unparsedFragments).toEqual(['config.custom']);

    const prose = extractReviewSubjectCandidates(
      'review authentication behavior',
      'D:/workspace/repo',
    );
    expect(prose.candidates).toEqual([
      {
        kind: 'workspace',
        id: 'candidate-1',
        workspace_path: 'D:/workspace/repo',
      },
    ]);
  });

  it('extracts every distinct two-dot Git range in textual order', () => {
    const result = extractReviewSubjectCandidates(
      'compare main..feature with release/v1..release/v2 and main..feature',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'git_range',
        id: 'candidate-1',
        source_ref: 'main',
        target_ref: 'feature',
      },
      {
        kind: 'git_range',
        id: 'candidate-2',
        source_ref: 'release/v1',
        target_ref: 'release/v2',
      },
    ]);
    expect(result.remainingFocus).toBe('compare with and');
  });

  it('keeps three-dot ranges unparsed and does not widen to workspace', () => {
    const result = extractReviewSubjectCandidates(
      'compare main...feature for regressions',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([]);
    expect(result.remainingFocus).toBe('compare main...feature for regressions');
    expect(result.unparsedFragments).toEqual(['main...feature']);

    const explicit = extractReviewSubjectCandidates(
      'commit main...feature',
      'D:/workspace/repo',
    );
    expect(explicit.candidates).toEqual([]);
    expect(explicit.remainingFocus).toBe('commit main...feature');
    expect(explicit.unparsedFragments).toEqual(['main...feature']);
  });

  it('extracts explicit commit and ref syntax with exact spans', () => {
    const result = extractReviewSubjectCandidates(
      'commit "feature/auth"; ref release/v2, then compare main..next',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'git_range',
        id: 'candidate-1',
        source_ref: 'feature/auth^',
        target_ref: 'feature/auth',
      },
      {
        kind: 'git_range',
        id: 'candidate-2',
        source_ref: 'release/v2^',
        target_ref: 'release/v2',
      },
      {
        kind: 'git_range',
        id: 'candidate-3',
        source_ref: 'main',
        target_ref: 'next',
      },
    ]);
    expect(result.remainingFocus).toBe('then compare');
    expect(result.unparsedFragments).toEqual([]);
  });

  it('does not infer commit syntax from prose', () => {
    const result = extractReviewSubjectCandidates(
      'review the commit handling logic',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'workspace',
        id: 'candidate-1',
        workspace_path: 'D:/workspace/repo',
      },
    ]);
    expect(result.remainingFocus).toBe('review the commit handling logic');
  });

  it('uses configured trusted provider mappings for self-hosted instances', () => {
    const result = extractReviewSubjectCandidates(
      'https://git.example.com/group/sub/project/-/merge_requests/12',
      undefined,
      {
        trustedProviderHosts: {
          'git.example.com': 'gitlab',
        },
      },
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'pull_request',
        host: 'git.example.com',
        project_path: 'group/sub/project',
        pull_request_id: '12',
      }),
    ]);
  });

  it('normalizes trusted hosts and safely decodes provider path segments', () => {
    const result = extractReviewSubjectCandidates(
      'https://GITHUB.COM/m%C3%BD-org/repo/issues/42',
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        host: 'github.com',
        project_path: 'mý-org/repo',
        issue_id: '42',
      }),
    ]);
  });

  it('rejects unsafe encoded provider paths and userinfo URLs', () => {
    for (const url of [
      'https://github.com/org%2Frepo/project/issues/42',
      'https://github.com/org%5Crepo/project/issues/42',
      'https://gitlab.com/group/%2e%2e/project/-/issues/42',
      'https://user@github.com/org/repo/issues/42',
    ]) {
      const result = extractReviewSubjectCandidates(url);
      expect(result.candidates).toEqual([
        {
          kind: 'external_reference',
          id: 'candidate-1',
          url,
        },
      ]);
    }
  });

  it('uses only provider pathname identity and ignores query or fragment spoofing', () => {
    for (const url of [
      'https://github.com/docs?target=/org/repo/issues/42',
      'https://gitlab.com/help#group/project/-/issues/42',
      'https://github.com/org/repo/issues/42/extra',
      'https://github.com/org/repo/issues/42suffix',
    ]) {
      expect(extractReviewSubjectCandidates(url).candidates).toEqual([
        {
          kind: 'external_reference',
          id: 'candidate-1',
          url,
        },
      ]);
    }

    const valid = 'https://github.com/org/repo/issues/42?redirect=%2Fdocs#note';
    expect(extractReviewSubjectCandidates(valid).candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        web_url: valid,
        issue_id: '42',
      }),
    ]);
  });

  it('rejects control characters and residual percent escapes in provider paths', () => {
    for (const url of [
      'https://github.com/org%00/repo/issues/42',
      'https://github.com/org%252Frepo/project/issues/42',
      'https://gitlab.com/group/%252e%252e/project/-/issues/42',
    ]) {
      expect(extractReviewSubjectCandidates(url).candidates).toEqual([
        {
          kind: 'external_reference',
          id: 'candidate-1',
          url,
        },
      ]);
    }
  });

  it.each([
    'https://github.com/org%3F/repo/issues/42',
    'https://gitlab.com/group/proj%23/-/issues/42',
    'https://github.com/my%20org/repo/issues/42',
    'https://gitlab.com/group/proj%E2%80%83/-/issues/42',
  ])('rejects decoded provider delimiters or whitespace in %s', (url) => {
    expect(extractReviewSubjectCandidates(url).candidates).toEqual([
      {
        kind: 'external_reference',
        id: 'candidate-1',
        url,
      },
    ]);
  });

  it('does not treat prose slash compounds as file paths', () => {
    const result = extractReviewSubjectCandidates(
      'review UI/UX and read/write behavior',
      'D:/workspace/repo',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'workspace',
        id: 'candidate-1',
        workspace_path: 'D:/workspace/repo',
      },
    ]);
    expect(result.unparsedFragments).toEqual([]);
  });

  it('accepts quoted slash compounds as explicit user-selected paths', () => {
    const result = extractReviewSubjectCandidates('"UI/UX" "read/write"');

    expect(result.candidates).toEqual([
      {
        kind: 'explicit_files',
        id: 'candidate-1',
        paths: ['UI/UX', 'read/write'],
      },
    ]);
  });

  it('accepts recognized project-root directories and explicit trailing slashes', () => {
    for (const path of [
      'src/runtime',
      'src/runtime/',
      'docs/design',
      'tests/fixtures',
      'scripts/release',
      'packages/client',
      'apps/desktop',
      'crates/runtime',
      'BitFun-Installer/assets',
      'custom/directory/',
    ]) {
      expect(extractReviewSubjectCandidates(path).candidates).toEqual([
        {
          kind: 'explicit_files',
          id: 'candidate-1',
          paths: [path],
        },
      ]);
    }
  });

  it('accepts strong path evidence and strips sentence punctuation', () => {
    const result = extractReviewSubjectCandidates(
      'review src/file.ts. "docs/review notes.md" /tmp/check.rs ./src and C:\\repo\\App.tsx',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'explicit_files',
        id: 'candidate-1',
        paths: [
          'src/file.ts',
          'docs/review notes.md',
          '/tmp/check.rs',
          './src',
          'C:\\repo\\App.tsx',
        ],
      },
    ]);
  });

  it('stops known provider URLs before adjacent Chinese focus text', () => {
    const result = extractReviewSubjectCandidates(
      'https://github.com/org/repo/issues/42请检查权限边界',
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        web_url: 'https://github.com/org/repo/issues/42',
        issue_id: '42',
      }),
    ]);
    expect(result.remainingFocus).toBe('请检查权限边界');
  });

  it('stops public provider URLs before Chinese sentence punctuation', () => {
    const result = extractReviewSubjectCandidates(
      'https://github.com/org/repo/issues/42\u3002\u8bf7\u68c0\u67e5',
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        web_url: 'https://github.com/org/repo/issues/42',
        issue_id: '42',
      }),
    ]);
    expect(result.remainingFocus).toBe('\u3002\u8bf7\u68c0\u67e5');
  });

  it('stops trusted self-hosted provider URLs before adjacent Chinese focus text', () => {
    const result = extractReviewSubjectCandidates(
      'https://git.example.com/group/project/-/issues/42\u8bf7\u68c0\u67e5',
      undefined,
      {
        trustedProviderHosts: {
          'git.example.com': 'gitlab',
        },
      },
    );

    expect(result.candidates).toEqual([
      expect.objectContaining({
        kind: 'issue',
        host: 'git.example.com',
        project_path: 'group/project',
        issue_id: '42',
      }),
    ]);
    expect(result.remainingFocus).toBe('\u8bf7\u68c0\u67e5');
  });

  it('preserves complete Unicode external URLs and public-provider query text', () => {
    const external = 'https://例え.テスト/路径/页面?查询=值#片段';
    expect(extractReviewSubjectCandidates(external)).toMatchObject({
      candidates: [
        {
          kind: 'external_reference',
          id: 'candidate-1',
          url: external,
        },
      ],
      remainingFocus: '',
    });

    const provider = 'https://github.com/org/repo/issues/42?note=请检查#讨论';
    expect(extractReviewSubjectCandidates(provider)).toMatchObject({
      candidates: [
        expect.objectContaining({
          kind: 'issue',
          web_url: provider,
          issue_id: '42',
        }),
      ],
      remainingFocus: '',
    });
  });

  it('extracts multiple explicit refs after English and Chinese conjunctions', () => {
    const result = extractReviewSubjectCandidates(
      'commit abc123 and ref release/v2; commit def456 以及 ref release/v3; commit fedcba 和 ref release/v4; commit 123abc 与 ref release/v5',
    );

    expect(result.candidates).toEqual([
      ['abc123^', 'abc123'],
      ['release/v2^', 'release/v2'],
      ['def456^', 'def456'],
      ['release/v3^', 'release/v3'],
      ['fedcba^', 'fedcba'],
      ['release/v4^', 'release/v4'],
      ['123abc^', '123abc'],
      ['release/v5^', 'release/v5'],
    ].map(([source_ref, target_ref], index) => ({
      kind: 'git_range',
      id: `candidate-${index + 1}`,
      source_ref,
      target_ref,
    })));
    expect(result.unparsedFragments).toEqual([]);
  });

  it('keeps invalid Git refs and ranges unparsed and fail-closed', () => {
    for (const fragment of [
      'main..bad~ref',
      'main..bad.lock',
      'main..bad//ref',
      'main..@',
      'commit bad^ref',
    ]) {
      const result = extractReviewSubjectCandidates(fragment, 'D:/workspace/repo');
      expect(result.candidates).toEqual([]);
      expect(result.remainingFocus).toBe(fragment);
      expect(result.unparsedFragments).toEqual([
        fragment.startsWith('commit ') ? fragment.slice('commit '.length) : fragment,
      ]);
    }
  });

  it('preserves balanced URL parentheses and strips unmatched sentence punctuation', () => {
    const result = extractReviewSubjectCandidates(
      'see https://example.com/docs/(draft) and https://example.com/end).',
    );

    expect(result.candidates).toEqual([
      {
        kind: 'external_reference',
        id: 'candidate-1',
        url: 'https://example.com/docs/(draft)',
      },
      {
        kind: 'external_reference',
        id: 'candidate-2',
        url: 'https://example.com/end',
      },
    ]);
  });

  it('trims URL punctuation and unmatched closers in one helper pass', () => {
    expect(trimReviewUrlToken('https://example.com/docs/(draft)')).toBe(
      'https://example.com/docs/(draft)',
    );
    expect(trimReviewUrlToken('https://example.com/docs/(draft))).')).toBe(
      'https://example.com/docs/(draft)',
    );
    expect(
      trimReviewUrlToken(`https://example.com/end${')'.repeat(10_000)}`),
    ).toBe('https://example.com/end');
  });

  it('deduplicates normalized provider subjects, ranges, and file paths uniformly', () => {
    const result = extractReviewSubjectCandidates(
      [
        'https://github.com/org/repo/issues/42',
        'https://github.com/org/repo/issues/42?view=all#note',
        'main..feature',
        'main..feature',
        'src/file.ts',
        'src/file.ts',
      ].join(' '),
    );

    expect(result.candidates.map((candidate) => candidate.kind)).toEqual([
      'issue',
      'git_range',
      'explicit_files',
    ]);
    expect(result.candidates[2]).toMatchObject({ paths: ['src/file.ts'] });
    expect(result.candidates.map((candidate) => candidate.id)).toEqual([
      'candidate-1',
      'candidate-2',
      'candidate-3',
    ]);
  });
});
