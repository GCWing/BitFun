CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    github_id INTEGER NOT NULL UNIQUE,
    login TEXT NOT NULL,
    avatar_url TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE listings (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    package_id TEXT NOT NULL UNIQUE,
    owner_user_id INTEGER NOT NULL REFERENCES users(id),
    latest_release_id TEXT,
    is_published INTEGER NOT NULL DEFAULT 0,
    download_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_skin_listings_owner ON listings(owner_user_id);
CREATE INDEX idx_skin_listings_published ON listings(is_published, updated_at DESC);

CREATE TABLE submissions (
    id TEXT PRIMARY KEY,
    listing_id TEXT REFERENCES listings(id),
    owner_user_id INTEGER NOT NULL REFERENCES users(id),
    slug TEXT NOT NULL,
    release_number INTEGER NOT NULL,
    draft_json TEXT NOT NULL,
    package_meta_json TEXT,
    manifest_json TEXT,
    status TEXT NOT NULL CHECK(status IN ('draft', 'submitted', 'approved', 'rejected', 'withdrawn')),
    package_sha256 TEXT,
    package_size INTEGER,
    preview_sha256 TEXT,
    rejection_reason TEXT,
    submitted_at INTEGER,
    reviewed_at INTEGER,
    reviewer_user_id INTEGER REFERENCES users(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_skin_submissions_owner ON submissions(owner_user_id, updated_at DESC);
CREATE INDEX idx_skin_submissions_status ON submissions(status, submitted_at);
CREATE UNIQUE INDEX idx_skin_submissions_active_new_slug
    ON submissions(slug)
    WHERE listing_id IS NULL AND status IN ('draft', 'submitted');
CREATE UNIQUE INDEX idx_skin_submissions_active_release
    ON submissions(listing_id, release_number)
    WHERE listing_id IS NOT NULL AND status IN ('draft', 'submitted');

CREATE TABLE releases (
    id TEXT PRIMARY KEY,
    listing_id TEXT NOT NULL REFERENCES listings(id),
    submission_id TEXT NOT NULL UNIQUE REFERENCES submissions(id),
    release_number INTEGER NOT NULL,
    draft_json TEXT NOT NULL,
    package_meta_json TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    package_size INTEGER NOT NULL,
    preview_sha256 TEXT NOT NULL,
    review_bundle_hash TEXT NOT NULL,
    published_at INTEGER NOT NULL,
    yanked_at INTEGER,
    yank_reason TEXT,
    UNIQUE(listing_id, release_number)
);
CREATE INDEX idx_skin_releases_listing ON releases(listing_id, release_number DESC);

CREATE TABLE download_days (
    listing_id TEXT NOT NULL REFERENCES listings(id) ON DELETE CASCADE,
    day TEXT NOT NULL,
    visitor_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(listing_id, day, visitor_hash)
);
CREATE INDEX idx_skin_download_days_listing ON download_days(listing_id);

CREATE TABLE audit_log (
    id TEXT PRIMARY KEY,
    actor_user_id INTEGER REFERENCES users(id),
    action TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    details_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_skin_audit_target ON audit_log(target_kind, target_id, created_at DESC);
