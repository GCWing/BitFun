CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    github_id INTEGER NOT NULL UNIQUE,
    login TEXT NOT NULL,
    avatar_url TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS web_sessions (
    token_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    csrf_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_web_sessions_user ON web_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_web_sessions_expiry ON web_sessions(expires_at);

CREATE TABLE IF NOT EXISTS api_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_type TEXT NOT NULL CHECK(token_type IN ('access', 'refresh')),
    family_id TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_api_tokens_family ON api_tokens(family_id);

CREATE TABLE IF NOT EXISTS desktop_auth_transactions (
    id TEXT PRIMARY KEY,
    secret_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'authorized', 'consumed', 'expired')),
    user_id INTEGER REFERENCES users(id),
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS oauth_flows (
    state_hash TEXT PRIMARY KEY,
    flow_kind TEXT NOT NULL CHECK(flow_kind IN ('web', 'desktop')),
    transaction_id TEXT REFERENCES desktop_auth_transactions(id) ON DELETE CASCADE,
    code_verifier TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_oauth_flows_expiry ON oauth_flows(expires_at);

CREATE TABLE IF NOT EXISTS listings (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    owner_user_id INTEGER NOT NULL REFERENCES users(id),
    latest_release_id TEXT,
    is_published INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_listings_owner ON listings(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_listings_published ON listings(is_published, updated_at DESC);

CREATE TABLE IF NOT EXISTS submissions (
    id TEXT PRIMARY KEY,
    listing_id TEXT REFERENCES listings(id),
    owner_user_id INTEGER NOT NULL REFERENCES users(id),
    slug TEXT NOT NULL,
    release_number INTEGER NOT NULL,
    metadata_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('draft', 'submitted', 'approved', 'rejected', 'withdrawn')),
    package_sha256 TEXT,
    package_size INTEGER,
    rejection_reason TEXT,
    submitted_at INTEGER,
    reviewed_at INTEGER,
    reviewer_user_id INTEGER REFERENCES users(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_submissions_owner ON submissions(owner_user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_submissions_status ON submissions(status, submitted_at);

CREATE TABLE IF NOT EXISTS releases (
    id TEXT PRIMARY KEY,
    listing_id TEXT NOT NULL REFERENCES listings(id),
    submission_id TEXT NOT NULL UNIQUE REFERENCES submissions(id),
    release_number INTEGER NOT NULL,
    metadata_json TEXT NOT NULL,
    package_sha256 TEXT NOT NULL,
    package_size INTEGER NOT NULL,
    review_bundle_hash TEXT NOT NULL,
    published_at INTEGER NOT NULL,
    yanked_at INTEGER,
    yank_reason TEXT,
    UNIQUE(listing_id, release_number)
);
CREATE INDEX IF NOT EXISTS idx_releases_listing ON releases(listing_id, release_number DESC);

CREATE TABLE IF NOT EXISTS screenshots (
    id TEXT PRIMARY KEY,
    submission_id TEXT NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    release_id TEXT REFERENCES releases(id),
    position INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    media_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(submission_id, position)
);
CREATE INDEX IF NOT EXISTS idx_screenshots_release ON screenshots(release_id, position);

CREATE TABLE IF NOT EXISTS ratings (
    listing_id TEXT NOT NULL REFERENCES listings(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    value INTEGER NOT NULL CHECK(value BETWEEN 1 AND 5),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(listing_id, user_id)
);

CREATE TABLE IF NOT EXISTS favorites (
    listing_id TEXT NOT NULL REFERENCES listings(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(listing_id, user_id)
);

CREATE TABLE IF NOT EXISTS download_days (
    listing_id TEXT NOT NULL REFERENCES listings(id) ON DELETE CASCADE,
    day TEXT NOT NULL,
    visitor_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(listing_id, day, visitor_hash)
);
CREATE INDEX IF NOT EXISTS idx_download_days_listing ON download_days(listing_id);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    actor_user_id INTEGER REFERENCES users(id),
    action TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    details_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_target ON audit_log(target_kind, target_id, created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS listing_search USING fts5(
    listing_id UNINDEXED,
    name,
    description,
    tags,
    category,
    tokenize = 'unicode61'
);
