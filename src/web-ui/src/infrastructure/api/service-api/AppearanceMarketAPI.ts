import { createTauriCommandError } from '../errors/TauriCommandError';
import { api } from './ApiClient';

export type AppearanceMarketSort = 'newest' | 'downloads';
export type AppearanceMarketMode = 'light' | 'dark';

export interface AppearanceMarketUserSummary {
  githubId: number;
  login: string;
  avatarUrl: string;
}

export interface AppearanceMarketListingSummary {
  listingId: string;
  slug: string;
  packageId: string;
  name: string;
  description: string;
  author?: string;
  mode: AppearanceMarketMode;
  packageVersion: string;
  latestRelease: number;
  minOpenBitFunVersion: string;
  requiredCapabilities: string[];
  owner: AppearanceMarketUserSummary;
  previewUrl: string;
  downloadCount: number;
  publishedAt: number;
}

export interface AppearanceMarketRelease {
  releaseId: string;
  listingId: string;
  releaseNumber: number;
  packageVersion: string;
  minOpenBitFunVersion: string;
  packageSha256: string;
  packageSize: number;
  reviewBundleHash: string;
  publishedAt: number;
  yanked: boolean;
}

export interface AppearanceMarketLicense {
  spdxExpression?: string;
  customUrl?: string;
}

export interface AppearanceMarketListingDetail extends AppearanceMarketListingSummary {
  changelog: string;
  license: AppearanceMarketLicense;
  repositoryUrl?: string;
  releases: AppearanceMarketRelease[];
}

export type AppearanceMarketSubmissionStatus =
  | 'draft'
  | 'submitted'
  | 'approved'
  | 'rejected'
  | 'withdrawn';

export type AppearanceMarketPublicationStatus = 'published' | 'yanked' | 'unpublished';

export interface AppearanceMarketSubmission {
  submissionId: string;
  listingId?: string;
  slug: string;
  releaseNumber: number;
  packageId?: string;
  name?: string;
  description?: string;
  author?: string;
  mode?: AppearanceMarketMode;
  packageVersion?: string;
  minOpenBitFunVersion: string;
  requiredCapabilities: string[];
  changelog: string;
  license: AppearanceMarketLicense;
  repositoryUrl?: string;
  status: AppearanceMarketSubmissionStatus;
  publicationStatus?: AppearanceMarketPublicationStatus;
  packageSha256?: string;
  packageSize?: number;
  previewUrl?: string;
  rejectionReason?: string;
  createdAt: number;
  updatedAt: number;
}

export interface AppearanceAdminSubmissionDetail {
  submission: AppearanceMarketSubmission;
  manifest?: unknown;
  packageSha256?: string;
  previewSha256?: string;
  reviewBundleHash?: string;
}

export interface AppearanceMarketCursorPage<T> {
  items: T[];
  nextCursor?: string;
}

export interface AppearanceMarketBrowseRequest {
  query?: string;
  mode?: AppearanceMarketMode | 'all';
  sort?: AppearanceMarketSort;
  cursor?: string;
  limit?: number;
}

export interface AppearanceMarketDownloadRequest {
  slug: string;
  releaseNumber: number;
  packageId: string;
  packageVersion: string;
  packageSha256: string;
  packageSize: number;
}

export interface AppearanceMarketSubmitPackageRequest {
  packagePath: string;
  slug?: string;
  minOpenBitFunVersion?: string;
  changelog?: string;
  license: AppearanceMarketLicense;
  repositoryUrl?: string;
}

function isolatedArrayBuffer(value: ArrayBuffer | Uint8Array): ArrayBuffer {
  if (value instanceof ArrayBuffer) return value.slice(0);
  if (ArrayBuffer.isView(value)) {
    return value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength) as ArrayBuffer;
  }
  throw new Error('Appearance market returned an invalid binary package response.');
}

export class AppearanceMarketAPI {
  async browse(
    request: AppearanceMarketBrowseRequest,
  ): Promise<AppearanceMarketCursorPage<AppearanceMarketListingSummary>> {
    try {
      return await api.invoke('appearance_market_browse', { request });
    } catch (error) {
      throw createTauriCommandError('appearance_market_browse', error, request);
    }
  }

  async getListing(slug: string): Promise<AppearanceMarketListingDetail> {
    try {
      return await api.invoke('appearance_market_get_listing', { request: { slug } });
    } catch (error) {
      throw createTauriCommandError('appearance_market_get_listing', error, { slug });
    }
  }

  async downloadRelease(request: AppearanceMarketDownloadRequest): Promise<ArrayBuffer> {
    try {
      const response = await api.invoke<ArrayBuffer | Uint8Array>(
        'appearance_market_download_release',
        { request },
        { timeout: 180_000 },
      );
      return isolatedArrayBuffer(response);
    } catch (error) {
      throw createTauriCommandError(
        'appearance_market_download_release',
        error,
        request,
      );
    }
  }

  async listSubmissions(): Promise<AppearanceMarketSubmission[]> {
    try {
      return await api.invoke('appearance_market_list_submissions', {});
    } catch (error) {
      throw createTauriCommandError('appearance_market_list_submissions', error);
    }
  }

  async chooseSubmissionPackage(title: string): Promise<string | null> {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
      directory: false,
      multiple: false,
      title,
      filters: [{ name: 'OpenBitFun Appearance', extensions: ['openbitfun-appearance'] }],
    });
    return typeof selected === 'string' && selected.length > 0 ? selected : null;
  }

  async submitPackage(
    request: AppearanceMarketSubmitPackageRequest,
  ): Promise<AppearanceMarketSubmission> {
    try {
      return await api.invoke(
        'appearance_market_submit_package',
        { request },
        { timeout: 300_000 },
      );
    } catch (error) {
      throw createTauriCommandError('appearance_market_submit_package', error, {
        ...request,
        packagePath: request.packagePath.split(/[\\/]/).pop() ?? '<selected package>',
      });
    }
  }

  async withdrawSubmission(submissionId: string): Promise<AppearanceMarketSubmission> {
    try {
      return await api.invoke('appearance_market_withdraw_submission', {
        request: { submissionId },
      });
    } catch (error) {
      throw createTauriCommandError('appearance_market_withdraw_submission', error, {
        submissionId,
      });
    }
  }

  async listReviewSubmissions(): Promise<AppearanceMarketSubmission[]> {
    try {
      return await api.invoke('appearance_market_list_review_submissions', {});
    } catch (error) {
      throw createTauriCommandError('appearance_market_list_review_submissions', error);
    }
  }

  async getReviewSubmission(submissionId: string): Promise<AppearanceAdminSubmissionDetail> {
    try {
      return await api.invoke('appearance_market_get_review_submission', {
        request: { submissionId },
      });
    } catch (error) {
      throw createTauriCommandError('appearance_market_get_review_submission', error, {
        submissionId,
      });
    }
  }

  async reviewSubmission(
    submissionId: string,
    decision: 'approve' | 'reject',
    reason = '',
  ): Promise<AppearanceAdminSubmissionDetail> {
    try {
      return await api.invoke('appearance_market_review_submission', {
        request: { submissionId, decision, reason },
      });
    } catch (error) {
      throw createTauriCommandError('appearance_market_review_submission', error, {
        submissionId,
        decision,
      });
    }
  }
}

export const appearanceMarketAPI = new AppearanceMarketAPI();
