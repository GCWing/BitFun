export type AppearanceMode = 'light' | 'dark';
export type AppearanceSort = 'newest' | 'downloads';
export type AppearanceModeFilter = 'all' | AppearanceMode;

export interface AppearanceMarketUser {
  githubId: number;
  login: string;
  avatarUrl: string;
}

export interface SharedMarketAccount {
  user: AppearanceMarketUser;
  isAdmin: boolean;
}

export interface SharedMarketAccountConfig {
  githubAuthConfigured: boolean;
}

export interface AppearanceListingSummary {
  listingId: string;
  slug: string;
  packageId: string;
  name: string;
  description: string;
  author?: string;
  mode: AppearanceMode;
  packageVersion: string;
  latestRelease: number;
  minOpenBitFunVersion: string;
  requiredCapabilities: string[];
  owner: AppearanceMarketUser;
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

export interface AppearanceListingDetail extends AppearanceListingSummary {
  changelog: string;
  license: AppearanceMarketLicense;
  repositoryUrl?: string;
  releases: AppearanceMarketRelease[];
}

export type AppearanceSubmissionStatus =
  | 'draft'
  | 'submitted'
  | 'approved'
  | 'rejected'
  | 'withdrawn';

export type AppearancePublicationStatus = 'published' | 'yanked' | 'unpublished';

export interface AppearanceSubmission {
  submissionId: string;
  listingId?: string;
  slug: string;
  releaseNumber: number;
  packageId?: string;
  name?: string;
  description?: string;
  author?: string;
  mode?: AppearanceMode;
  packageVersion?: string;
  minOpenBitFunVersion: string;
  requiredCapabilities: string[];
  changelog: string;
  license: AppearanceMarketLicense;
  repositoryUrl?: string;
  status: AppearanceSubmissionStatus;
  publicationStatus?: AppearancePublicationStatus;
  packageSha256?: string;
  packageSize?: number;
  previewUrl?: string;
  rejectionReason?: string;
  createdAt: number;
  updatedAt: number;
}

export interface AppearanceAdminSubmissionDetail {
  submission: AppearanceSubmission;
  submitter?: AppearanceMarketUser;
  manifest?: unknown;
  packageSha256?: string;
  previewSha256?: string;
  reviewBundleHash?: string;
}

export interface CursorPage<T> {
  items: T[];
  nextCursor?: string;
}

export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
    requestId: string;
    details?: unknown;
  };
}

export interface ListAppearancesRequest {
  query?: string;
  mode?: AppearanceModeFilter;
  sort?: AppearanceSort;
  cursor?: string;
  limit?: number;
}
