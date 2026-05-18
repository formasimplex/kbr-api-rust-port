-- kbr-api-rust database schema
-- Derived from Rails db/schema.rb (version: 2026_02_15_233710)
-- Idempotent: safe to run repeatedly on existing or fresh databases

-- Enable UUID extension for revoked_tokens
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Users & authentication
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    email VARCHAR NOT NULL,
    password_digest VARCHAR NOT NULL,
    role VARCHAR,
    session_token VARCHAR,
    username VARCHAR,
    token_version BIGINT DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_username_key UNIQUE (username)
);

-- Add token_version column if it doesn't exist (for existing tables)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'users' AND column_name = 'token_version'
    ) THEN
        ALTER TABLE users ADD COLUMN token_version BIGINT DEFAULT 1;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS revoked_tokens (
    jti UUID PRIMARY KEY,
    user_id BIGINT REFERENCES users(id),
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_revoked_tokens_user_id ON revoked_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_revoked_tokens_revoked_at ON revoked_tokens(revoked_at);

CREATE TABLE IF NOT EXISTS reset_triggers (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER,
    token VARCHAR,
    expires_at VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS sign_up_triggers (
    id BIGSERIAL PRIMARY KEY,
    email VARCHAR,
    token VARCHAR,
    expires_at VARCHAR,
    role VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS permissions (
    id BIGSERIAL PRIMARY KEY,
    resource VARCHAR,
    can_create BOOLEAN DEFAULT FALSE,
    can_read BOOLEAN DEFAULT TRUE,
    can_update BOOLEAN DEFAULT FALSE,
    can_delete BOOLEAN DEFAULT FALSE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Artists
CREATE TABLE IF NOT EXISTS artists (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    genre VARCHAR,
    bio TEXT,
    user_id BIGINT REFERENCES users(id),
    prospect BOOLEAN DEFAULT FALSE,
    "spotifyId" VARCHAR,
    "subHeading" VARCHAR,
    intro VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS artist_links (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id),
    link_type INTEGER,
    url VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS social_media (
    id BIGSERIAL PRIMARY KEY,
    url VARCHAR,
    name VARCHAR,
    icon VARCHAR,
    artist_id BIGINT REFERENCES artists(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Albums & songs
CREATE TABLE IF NOT EXISTS albums (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    release_date DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS songs (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    duration VARCHAR,
    album_id BIGINT NOT NULL REFERENCES albums(id),
    artist_id BIGINT NOT NULL REFERENCES artists(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Campaigns
CREATE TABLE IF NOT EXISTS campaigns (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id),
    name VARCHAR,
    active BOOLEAN,
    vinyl_sold_count INTEGER,
    campaign_start_date TIMESTAMPTZ,
    campaign_end_date TIMESTAMPTZ,
    progress INTEGER,
    album_id BIGINT REFERENCES albums(id),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_campaigns_on_deleted_at ON campaigns(deleted_at);

CREATE TABLE IF NOT EXISTS campaign_pages (
    id BIGSERIAL PRIMARY KEY,
    campaign_id BIGINT NOT NULL REFERENCES campaigns(id),
    title VARCHAR,
    description TEXT,
    page_type INTEGER,
    inventory_item_id VARCHAR,
    inventory_url VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Merchandise
CREATE TABLE IF NOT EXISTS producers (
    id BIGSERIAL PRIMARY KEY,
    description TEXT,
    producer_name VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS artist_merchandise (
    id BIGSERIAL PRIMARY KEY,
    artist_id BIGINT NOT NULL REFERENCES artists(id),
    producer_id BIGINT NOT NULL REFERENCES producers(id),
    merchandise_id VARCHAR,
    description TEXT,
    created_on_producer BOOLEAN DEFAULT FALSE,
    merch_title VARCHAR NOT NULL,
    merch_product_title VARCHAR,
    set_price DECIMAL(10, 2),
    cost_price DECIMAL(10, 2),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Events
CREATE TABLE IF NOT EXISTS kbr_events (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    description VARCHAR,
    active BOOLEAN,
    event_start_date TIMESTAMPTZ,
    event_end_date TIMESTAMPTZ,
    create_by_user_id INTEGER,
    event_url VARCHAR,
    qr_encode_string VARCHAR,
    ticket_url VARCHAR,
    external_url VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS kbr_event_attendees (
    id BIGSERIAL PRIMARY KEY,
    kbr_event_id INTEGER,
    mail_subscriber_id INTEGER,
    scan_count INTEGER DEFAULT 0,
    headcount INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- News & playlists
CREATE TABLE IF NOT EXISTS news (
    id BIGSERIAL PRIMARY KEY,
    url VARCHAR,
    title VARCHAR,
    vote_score INTEGER DEFAULT 0,
    flagged BOOLEAN,
    flagged_at TIMESTAMPTZ,
    user_id BIGINT NOT NULL REFERENCES users(id),
    image_url VARCHAR,
    active BOOLEAN DEFAULT TRUE,
    comments_enabled BOOLEAN DEFAULT TRUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS news_playlists (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    name VARCHAR NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS users_news (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    news_id BIGINT NOT NULL REFERENCES news(id),
    playlist_id BIGINT NOT NULL REFERENCES news_playlists(id),
    position INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_users_news_on_playlist_position ON users_news(playlist_id, position);
CREATE INDEX IF NOT EXISTS idx_users_news_on_position ON users_news(position);

CREATE TABLE IF NOT EXISTS comments (
    id BIGSERIAL PRIMARY KEY,
    content TEXT,
    flagged BOOLEAN,
    flagged_at TIMESTAMPTZ,
    commentable_type VARCHAR NOT NULL,
    commentable_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users(id),
    parent_id INTEGER REFERENCES comments(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_comments_on_commentable ON comments(commentable_type, commentable_id);

-- Mailing
CREATE TABLE IF NOT EXISTS mail_subscribers (
    id BIGSERIAL PRIMARY KEY,
    full_name VARCHAR NOT NULL,
    email VARCHAR NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    artist_id BIGINT REFERENCES artists(id),
    unsubscribed_at TIMESTAMPTZ,
    unsubscribe_token VARCHAR,
    unsubscribe_token_expires_at TIMESTAMPTZ,
    user_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE mail_subscribers ADD COLUMN IF NOT EXISTS unsubscribe_token_expires_at TIMESTAMPTZ;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mail_subscribers_on_email_artist ON mail_subscribers(email, artist_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_mail_subscribers_on_unsubscribe_token ON mail_subscribers(unsubscribe_token);
CREATE INDEX IF NOT EXISTS idx_mail_subscribers_on_unsubscribed_at ON mail_subscribers(unsubscribed_at);

-- Tenant configs
CREATE TABLE IF NOT EXISTS tenant_configs (
    tenant_id UUID NOT NULL DEFAULT gen_random_uuid(),
    logo_url VARCHAR,
    short_name VARCHAR NOT NULL,
    long_name VARCHAR NOT NULL,
    footer_logo_url VARCHAR,
    contact_email VARCHAR NOT NULL,
    site_header_description TEXT NOT NULL,
    deleted_at TIMESTAMPTZ,
    "instaUrl" VARCHAR,
    "twitterUrl" VARCHAR,
    "tiktokUrl" VARCHAR,
    "spotifyId" VARCHAR,
    featured_artist_id BIGINT REFERENCES artists(id),
    mantine_theme JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_tenant_configs_on_tenant_id ON tenant_configs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_tenant_configs_on_deleted_at ON tenant_configs(deleted_at);

-- Shopify integration
CREATE TABLE IF NOT EXISTS shopify_json_caches (
    id BIGSERIAL PRIMARY KEY,
    cached_item_id VARCHAR,
    json_entry VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Customers (legacy)
CREATE TABLE IF NOT EXISTS customers (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR,
    email VARCHAR,
    password_digest VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Generated responses (AI/chat)
CREATE TABLE IF NOT EXISTS generated_responses (
    id BIGSERIAL PRIMARY KEY,
    response VARCHAR,
    user_id BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Active Storage (Rails file uploads - may not be needed in Rust)
CREATE TABLE IF NOT EXISTS active_storage_blobs (
    id BIGSERIAL PRIMARY KEY,
    key VARCHAR NOT NULL,
    filename VARCHAR NOT NULL,
    content_type VARCHAR,
    metadata TEXT,
    service_name VARCHAR NOT NULL,
    byte_size BIGINT NOT NULL,
    checksum VARCHAR,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_active_storage_blobs_on_key ON active_storage_blobs(key);

CREATE TABLE IF NOT EXISTS active_storage_attachments (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    record_type VARCHAR NOT NULL,
    record_id BIGINT NOT NULL,
    blob_id BIGINT NOT NULL REFERENCES active_storage_blobs(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_active_storage_attachments_uniqueness ON active_storage_attachments(record_type, record_id, name, blob_id);

CREATE TABLE IF NOT EXISTS active_storage_variant_records (
    id BIGSERIAL PRIMARY KEY,
    blob_id BIGINT NOT NULL REFERENCES active_storage_blobs(id),
    variation_digest VARCHAR NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_active_storage_variant_records_uniqueness ON active_storage_variant_records(blob_id, variation_digest);

-- Delayed jobs (Rails background jobs - may not be needed in Rust)
CREATE TABLE IF NOT EXISTS delayed_jobs (
    id BIGSERIAL PRIMARY KEY,
    priority INTEGER DEFAULT 0 NOT NULL,
    attempts INTEGER DEFAULT 0 NOT NULL,
    handler TEXT NOT NULL,
    last_error TEXT,
    run_at TIMESTAMPTZ,
    locked_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    locked_by VARCHAR,
    queue VARCHAR,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS delayed_jobs_priority ON delayed_jobs(priority, run_at);

-- Job queue for retry and dead-letter tracking
CREATE TABLE IF NOT EXISTS job_queue (
    id UUID PRIMARY KEY,
    job_type VARCHAR NOT NULL,
    payload JSONB NOT NULL,
    attempts INTEGER DEFAULT 0 NOT NULL,
    max_attempts INTEGER DEFAULT 3 NOT NULL,
    last_error TEXT,
    status VARCHAR DEFAULT 'pending' NOT NULL,
    next_retry_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_queue_retry ON job_queue(status, next_retry_at) WHERE status = 'retrying';
CREATE INDEX IF NOT EXISTS idx_job_queue_status ON job_queue(status);
