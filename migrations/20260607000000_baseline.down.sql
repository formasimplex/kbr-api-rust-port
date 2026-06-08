-- Reverse of baseline: drop all tables and indexes in reverse dependency order

-- Indexes
DROP INDEX IF EXISTS idx_job_queue_retry;
DROP INDEX IF EXISTS idx_job_queue_status;
DROP INDEX IF EXISTS delayed_jobs_priority;
DROP INDEX IF EXISTS idx_active_storage_variant_records_uniqueness;
DROP INDEX IF EXISTS idx_active_storage_attachments_uniqueness;
DROP INDEX IF EXISTS idx_active_storage_blobs_on_key;
DROP INDEX IF EXISTS idx_tenant_configs_on_deleted_at;
DROP INDEX IF EXISTS idx_tenant_configs_on_tenant_id;
DROP INDEX IF EXISTS idx_mail_subscribers_on_unsubscribed_at;
DROP INDEX IF EXISTS idx_mail_subscribers_on_unsubscribe_token;
DROP INDEX IF EXISTS idx_mail_subscribers_on_email_artist;
DROP INDEX IF EXISTS idx_comments_on_commentable;
DROP INDEX IF EXISTS idx_users_news_on_position;
DROP INDEX IF EXISTS idx_users_news_on_playlist_position;
DROP INDEX IF EXISTS idx_campaigns_on_deleted_at;
DROP INDEX IF EXISTS idx_revoked_tokens_revoked_at;
DROP INDEX IF EXISTS idx_revoked_tokens_user_id;

-- Tables (children first, parents last)
DROP TABLE IF EXISTS active_storage_variant_records CASCADE;
DROP TABLE IF EXISTS active_storage_attachments CASCADE;
DROP TABLE IF EXISTS active_storage_blobs CASCADE;
DROP TABLE IF EXISTS generated_responses CASCADE;
DROP TABLE IF EXISTS customers CASCADE;
DROP TABLE IF EXISTS shopify_json_caches CASCADE;
DROP TABLE IF EXISTS tenant_configs CASCADE;
DROP TABLE IF EXISTS mail_subscribers CASCADE;
DROP TABLE IF EXISTS comments CASCADE;
DROP TABLE IF EXISTS users_news CASCADE;
DROP TABLE IF EXISTS news_playlists CASCADE;
DROP TABLE IF EXISTS news CASCADE;
DROP TABLE IF EXISTS kbr_event_attendees CASCADE;
DROP TABLE IF EXISTS kbr_events CASCADE;
DROP TABLE IF EXISTS artist_merchandise CASCADE;
DROP TABLE IF EXISTS producers CASCADE;
DROP TABLE IF EXISTS campaign_pages CASCADE;
DROP TABLE IF EXISTS campaigns CASCADE;
DROP TABLE IF EXISTS songs CASCADE;
DROP TABLE IF EXISTS albums CASCADE;
DROP TABLE IF EXISTS social_media CASCADE;
DROP TABLE IF EXISTS artist_links CASCADE;
DROP TABLE IF EXISTS artists CASCADE;
DROP TABLE IF EXISTS permissions CASCADE;
DROP TABLE IF EXISTS sign_up_triggers CASCADE;
DROP TABLE IF EXISTS reset_triggers CASCADE;
DROP TABLE IF EXISTS revoked_tokens CASCADE;
DROP TABLE IF EXISTS delayed_jobs CASCADE;
DROP TABLE IF EXISTS job_queue CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Extension
DROP EXTENSION IF EXISTS "pgcrypto";
