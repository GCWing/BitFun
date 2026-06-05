import { describe, expect, it } from 'vitest';
import {
  classifyReviewTargetFromFiles,
  createUnknownReviewTargetClassification,
  getReviewerApplicabilityRule,
  normalizeReviewPath,
  shouldRunReviewerForTarget,
} from './reviewTargetClassifier';

describe('reviewTargetClassifier', () => {
  it('normalizes Windows and relative paths for review classification', () => {
    expect(normalizeReviewPath('.\\src\\web-ui\\src\\App.tsx')).toBe(
      'src/web-ui/src/App.tsx',
    );
  });

  it('classifies frontend source, style, locale, and contract files', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/App.tsx',
        'src/web-ui/src/app/App.scss',
        'src/web-ui/src/locales/en-US/flow-chat.json',
        'src/apps/desktop/src/api/agentic_api.rs',
      ],
      'session_files',
    );

    expect(target.resolution).toBe('resolved');
    expect(target.tags).toEqual(
      expect.arrayContaining([
        'frontend_ui',
        'frontend_style',
        'frontend_i18n',
        'desktop_contract',
        'frontend_contract',
      ]),
    );
    expect(target.files[0]).toMatchObject({
      path: 'src/web-ui/src/App.tsx',
      normalizedPath: 'src/web-ui/src/App.tsx',
      source: 'session_files',
      tags: expect.arrayContaining(['frontend_ui']),
    });
  });

  it('classifies backend core files without frontend tags', () => {
    const target = classifyReviewTargetFromFiles(
      ['src/crates/assembly/core/src/service/config/types.rs'],
      'session_files',
    );

    expect(target.resolution).toBe('resolved');
    expect(target.tags).toEqual(['backend_core']);
  });

  it('classifies layered Rust crate paths from the current Cargo layout', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/crates/contracts/runtime-ports/src/lib.rs',
        'src/crates/execution/agent-runtime/src/lib.rs',
        'src/crates/execution/tool-contracts/src/lib.rs',
        'src/crates/services/services-core/src/lib.rs',
        'src/crates/assembly/product-capabilities/src/lib.rs',
        'src/crates/interfaces/acp/src/lib.rs',
        'src/crates/adapters/webdriver/src/lib.rs',
      ],
      'session_files',
    );

    expect(target.resolution).toBe('resolved');
    expect(target.tags).toEqual(
      expect.arrayContaining(['backend_core']),
    );
    expect(target.tags).not.toContain('unknown');
    expect(target.files.every((file) => !file.tags.includes('unknown'))).toBe(true);
    expect(target.files.find((file) =>
      file.normalizedPath === 'src/crates/interfaces/acp/src/lib.rs',
    )?.tags).toEqual(expect.arrayContaining(['backend_core', 'transport']));
  });

  it('classifies installer and core locale resources as i18n targets', () => {
    const target = classifyReviewTargetFromFiles(
      [
        'src/web-ui/src/locales/zh-TW/flow-chat.json',
        'src/crates/assembly/core/locales/zh-TW.ftl',
        'BitFun-Installer/src/i18n/locales/zh-TW.json',
      ],
      'session_files',
    );

    expect(target.resolution).toBe('resolved');
    expect(target.tags).toEqual(
      expect.arrayContaining(['frontend_i18n', 'installer_ui']),
    );
    expect(target.files.every((file) => file.tags.includes('frontend_i18n'))).toBe(true);
    expect(target.files.find((file) =>
      file.normalizedPath === 'BitFun-Installer/src/i18n/locales/zh-TW.json',
    )?.tags).toEqual(expect.arrayContaining(['frontend_i18n', 'installer_ui']));
  });

  it('returns an unknown target when no file list is available', () => {
    const target = createUnknownReviewTargetClassification('unknown');

    expect(target.resolution).toBe('unknown');
    expect(target.tags).toEqual(['unknown']);
    expect(target.warnings).toEqual([
      expect.objectContaining({ code: 'target_unknown' }),
    ]);
  });

  it('keeps frontend reviewer applicability in a reusable registry', () => {
    const rule = getReviewerApplicabilityRule('ReviewFrontend');

    expect(rule).toEqual(
      expect.objectContaining({
        subagentId: 'ReviewFrontend',
        runWhenTargetUnknown: true,
        matchingTags: expect.arrayContaining([
          'frontend_ui',
          'frontend_contract',
        ]),
      }),
    );
  });

  it('evaluates conditional reviewer applicability from registry tags', () => {
    const backendTarget = classifyReviewTargetFromFiles(
      ['src/crates/assembly/core/src/service/config/types.rs'],
      'session_files',
    );
    const frontendTarget = classifyReviewTargetFromFiles(
      ['src/web-ui/src/App.tsx'],
      'session_files',
    );
    const unknownTarget = createUnknownReviewTargetClassification('manual_prompt');

    expect(shouldRunReviewerForTarget('ReviewFrontend', backendTarget)).toBe(false);
    expect(shouldRunReviewerForTarget('ReviewFrontend', frontendTarget)).toBe(true);
    expect(shouldRunReviewerForTarget('ReviewFrontend', unknownTarget)).toBe(true);
    expect(shouldRunReviewerForTarget('ReviewSecurity', backendTarget)).toBe(true);
  });
});
