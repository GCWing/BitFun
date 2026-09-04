export interface MarketUser {
  githubId: number;
  login: string;
  avatarUrl: string;
}

export interface NodePermissions {
  enabled: boolean;
  max_memory_mb?: number;
  timeout_ms?: number;
}

export interface MiniAppPermissions {
  fs?: { read?: string[]; write?: string[] };
  shell?: { allow?: string[] };
  net?: { allow?: string[] };
  node?: NodePermissions;
  ai?: {
    enabled: boolean;
    allowed_models?: string[];
    max_tokens_per_request?: number;
    rate_limit_per_minute?: number;
  };
  agent?: { enabled: boolean; rate_limit_per_minute?: number };
  notifications?: { system: boolean };
  host?: {
    dialog?: boolean;
    clipboard_read?: boolean;
    clipboard_write?: boolean;
    open_external?: boolean;
    reveal_in_folder?: boolean;
    deck_render?: boolean;
    chat_composer?: boolean;
    system_info?: boolean;
  };
}

export interface MiniAppI18n {
  locales: Record<
    string,
    {
      name?: string;
      description?: string;
      tags?: string[];
    }
  >;
}

export interface MarketRelease {
  releaseId: string;
  listingId: string;
  releaseNumber: number;
  minOpenBitFunVersion: string;
  changelog: string;
  packageSha256: string;
  packageSize: number;
  reviewBundleHash: string;
  permissions: MiniAppPermissions;
  publishedAt: number;
  yanked: boolean;
}

export interface MarketListingSummary {
  listingId: string;
  slug: string;
  name: string;
  description: string;
  icon: string;
  category: string;
  tags: string[];
  owner: MarketUser;
  latestRelease: number;
  minOpenBitFunVersion: string;
  permissions: MiniAppPermissions;
  screenshotUrls: string[];
  ratingAverage: number;
  ratingCount: number;
  favoriteCount: number;
  downloadCount: number;
  publishedAt: number;
  i18n?: MiniAppI18n;
  isFavorited?: boolean;
  myRating?: number;
}

export interface MarketListingDetail extends MarketListingSummary {
  changelog: string;
  license: { spdxExpression?: string; customUrl?: string };
  repositoryUrl?: string;
  releases: MarketRelease[];
}

export type SubmissionStatus =
  | 'draft'
  | 'submitted'
  | 'approved'
  | 'rejected'
  | 'withdrawn';

export interface MarketSubmission {
  submissionId: string;
  listingId?: string;
  slug: string;
  releaseNumber: number;
  name: string;
  description: string;
  icon: string;
  category: string;
  tags: string[];
  minOpenBitFunVersion: string;
  changelog: string;
  license: { spdxExpression?: string; customUrl?: string };
  repositoryUrl?: string;
  permissions: MiniAppPermissions;
  status: SubmissionStatus;
  packageSha256?: string;
  packageSize?: number;
  screenshotUrls: string[];
  rejectionReason?: string;
  createdAt: number;
  updatedAt: number;
}

export interface AdminSubmission extends MarketSubmission {
  submitter?: MarketUser;
  submittedAt?: number;
}

export interface AdminSubmissionDetail {
  submission: MarketSubmission;
  submitter?: MarketUser;
  submittedAt?: number;
  sourceFiles: Record<string, string>;
  previousSourceFiles: Record<string, string>;
  sourceDiffs: Record<string, string>;
  screenshotHashes: string[];
}

export interface CursorPage<T> {
  items: T[];
  nextCursor?: string;
}

export interface Me {
  user: MarketUser;
  isAdmin: boolean;
}

export interface MarketConfig {
  githubAuthConfigured: boolean;
  publicBrowse: boolean;
  webSubmissionsEnabled: boolean;
  categories: string[];
}

export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
    requestId: string;
    details?: unknown;
  };
}
