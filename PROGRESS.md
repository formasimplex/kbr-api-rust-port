# PROGRESS.md — kbr-api-rust

## Plan: 8-Stage Port
| Stage | Scope | Status |
|-------|-------|--------|
| 0 | Infrastructure (error, DB, JWT, roles, health) | ✅ Done |
| 1 | Auth + Users (login, session, user CRUD, permissions) | ✅ Done + SQLx |
| 2 | Core Content (albums, songs, artists, campaigns, etc.) | ✅ Done + SQLx |
| 3 | Social (comments, news, playlists) | ✅ Done + SQLx |
| 4 | Events + Mailing | ✅ Done + SQLx |
| 5 | Commerce + Config | ✅ Done + SQLx |
| 6 | External Services (S3/Storage, Shopify, Mailchimp, OpenAI) | S3/Storage Done, rest Pending |
| 7 | Webhooks + Missing Endpoints | ✅ Done |

## Stage 2a — SQLx Real DB Integration (COMPLETE)
All 19 handlers converted from mock data to real PostgreSQL queries via `sqlx`. Zero mock data remains.

### Infrastructure Changes
- `app.rs` — `AppState` shared module for handler access
- `main.rs` — refactored to use lib crate (`kbr_api_rust::`)
- `Cargo.toml` — `[[bin]]` section for binary/library separation
- `.env` — test DATABASE_URL configuration
- `.gitignore` — includes `.env`

### Key Patterns Established
- **Rust 2024 edition**: `$"..."` is a format string — SQL `$1`/`$2` params require `r"..."` raw strings
- **TIMESTAMP vs TIMESTAMPTZ**: `timestamp without time zone` maps to `chrono::NaiveDateTime`
- **Connection string**: TCP `postgresql://ws@localhost:5432/kbr_test`
- **Test pattern**: `web::Data<AppState>` with real `PgPool`; `serde_json::Value` for response parsing
- **Partial updates**: `COALESCE($1, column)` pattern
- **CamelCase columns**: double-quoted in SQL, `#[sqlx(rename = "camelCase")]` on FromRow fields
- **Seed uniqueness**: timestamp suffixes for test data
- **FK constraints**: seed parent tables before child inserts

### All Handlers Converted (305 tests passing)
| # | Handler | Tests | Endpoints |
|---|---------|-------|-----------|
| 1 | `health.rs` | 1 | health check |
| 2 | `auth.rs` | 7 | login, session |
| 3 | `users.rs` | 10 | CRUD + role checks |
| 4 | `permissions.rs` | 11 | CRUD + resource validation |
| 5 | `sign_up_trigger.rs` | 4 | token CRUD + expiry |
| 6 | `reset_trigger.rs` | 6 | token CRUD + password reset |
| 7 | `albums.rs` | 5 | simple CRUD |
| 8 | `songs.rs` | 5 | simple CRUD |
| 9 | `artists.rs` | 12 | CRUD + links + sign-up trigger |
| 10 | `producers.rs` | 5 | simple CRUD |
| 11 | `campaigns.rs` | 13 | soft-delete, user-scoped, shopify |
| 12 | `campaign_pages.rs` | 5 | read-only |
| 13 | `merchandise.rs` | 11 | shopify cache, artist-scoped |
| 14 | `configs.rs` | 4 | soft-delete, tenant lookup |
| 15 | `comments.rs` | 8 | polymorphic, nested replies |
| 16 | `news.rs` | 9 | URL safety, OG tags, playlist add |
| 17 | `playlists.rs` | 11 | admin + dashboard, reorder, ownership |
| 18 | `events.rs` | 10 | user-scoped, pagination |
| 19 | `event_attendees.rs` | 7 | QR scan, email jobs |
| 20 | `mailing.rs` | 13 | mailchimp, unsubscribe flow |

### Test Summary
- **316 tests passing** (0 failing, 0 flaky)
- All handler tests use real PostgreSQL queries against `kbr_test`
- Seed data uses timestamp-suffixed names for uniqueness
- Cleanup after each test prevents cross-test interference

## Stage 7 — Missing Endpoints (COMPLETE)
Implemented remaining Rails controllers not yet ported:

| # | Handler | Tests | Endpoints |
|---|---------|-------|-----------|
| 21 | `data_api.rs` | 5 | `GET /v1/data/last_logins`, `GET /v1/data/last_logins/:id`, `GET /v1/data/event_attendees_present/:id` |
| 22 | `webhook.rs` | 6 | `POST /v1/webhook/update_progress`, `customers_data_request`, `customers_redact`, `shop_redact` |

## Clippy & Code Quality Cleanup (COMPLETE)
- Eliminated all 11 remaining clippy warnings across 25 files
- `src/handlers/configs.rs` — Renamed `instaUrl`, `twitterUrl`, `tiktokUrl`, `spotifyId` to snake_case (DB column mapping preserved via `#[sqlx(rename)]`)
- `src/handlers/webhook.rs` — Made `WebhookInventoryParams` and `WebhookPayload` `pub` to match handler visibility
- `src/auth/roles.rs` — Replaced `Role::from_str` with proper `impl std::str::FromStr for Role`
- `src/auth/middleware.rs` — Updated role parsing to use `.parse::<Role>()`
- 25 files — Auto-fixed `unnecessary_to_owned` (`.to_string()` on raw string literals)
- Final: 0 clippy warnings, 316 tests passing, clean build

## Key Decisions
- `web::Query<serde_json::Value>` for flexible query param parsing
- `web::Json<serde_json::Value>` for flexible request body parsing
- `OnceLock` for thread-safe lazy bcrypt hash generation in test mocks
- `role` claim in JWT to avoid DB lookups in middleware during testing
- `release_date` as `Option<String>` to avoid `chrono::Date` deprecation
- `#[allow(dead_code)]` at crate level — model/service helpers prepared for SQLx
- `AppState` in shared `app.rs` module — accessible from both binary and lib crate
- `r"..."` raw strings for SQL — avoids Rust 2024 `$"..."` format string parsing
- `chrono::NaiveDateTime` for `TIMESTAMP` — `DateTime<Utc>` only for `TIMESTAMPTZ`
- Test DB: `kbr_test` on localhost:5432 — shared schema with Rails test database

## Notes
- Rust replaces Rails; shared PostgreSQL DB only during transition
- S3/Storage is highest priority among external integrations
- Background jobs (email queueing) deferred to later stages

## Stage 6a — S3 Storage Integration (COMPLETE)
Linode Object Storage integration with `rust-s3` and `rs-vips` for image uploads, variant generation, and presigned URL delivery.

### Dependencies Added
- `rust-s3 0.37.2` — S3-compatible client (tokio-native-tls)
- `rs-vips 0.7` — image resizing and WebP encoding
- `actix-multipart 0.7` — multipart form parsing for uploads
- `sha256 1.6` — SHA-256 file checksums (replaced insecure MD5)
- `futures-util 0.3` — async stream iteration
- `hex 0.4` — hex encoding for checksums

### Infrastructure Changes
- `src/app.rs` — `AppState.s3: Box<S3Bucket>`
- `src/error.rs` — `Storage(String)` error variant (HTTP 500)
- `src/main.rs` — initializes vips, creates S3 bucket, registers storage routes
- `src/services/storage_service.rs` — S3 config, upload, DB attachment, presigned URLs, image URL fetching, deletion, variation digest
- `src/handlers/storage.rs` — `POST /upload`, `GET /images/:type/:id`, `DELETE /blob/:id`, route config, unit/integration tests
- `src/models/artist.rs` — `ArtistResponse` includes `image_urls` and `image_thumbnail_urls`
- `src/handlers/artists.rs` — `index`/`show` call `storage_service::get_image_urls`; `create`/`update` pass empty image vectors
- 22 handler test files — `get_state()` updated with S3 bucket initialization

### Key Design Decisions
- **Rails ActiveStorage-compatible**: stores files in `active_storage_blobs`/`active_storage_attachments` tables for seamless Rails/Rust transition
- **WebP variants**: 512x512 (medium) and 100x100 (thumbnail) via `rs-vips::VipsImage::thumbnail_image`
- **Path-style S3**: required for Linode Object Storage compatibility
- **`Region::Custom`**: embeds custom endpoint directly in region config
- **SHA-256 checksums**: file integrity via `sha256::digest()` — MD5 removed per CVE-2005-4959 / CVE-2010-5121
- **Presigned URLs**: 1-hour expiry, no query params
- **Upload limits**: 10MB max, `image/*` content type enforced
- **Variant records**: `active_storage_variant_records` tracked with `ON CONFLICT DO NOTHING`

### S3 Environment Variables
| Variable | Description |
|----------|-------------|
| `S3_ACCESS_KEY` | Linode API access key |
| `S3_SECRET_KEY` | Linode API secret key |
| `S3_ENDPOINT` | `https://kbr-storage-01.us-southeast-1.linodeobjects.com` |
| `S3_BUCKET` | Bucket name |
| `S3_REGION` | Region (default: `us-southeast-1`) |

## Key Decisions
- `web::Query<serde_json::Value>` for flexible query param parsing
- `web::Json<serde_json::Value>` for flexible request body parsing
- `OnceLock` for thread-safe lazy bcrypt hash generation in test mocks
- `role` claim in JWT to avoid DB lookups in middleware during testing
- `release_date` as `Option<String>` to avoid `chrono::Date` deprecation
- `#[allow(dead_code)]` at crate level — model/service helpers prepared for SQLx
- `AppState` in shared `app.rs` module — accessible from both binary and lib crate
- `r"..."` raw strings for SQL — avoids Rust 2024 `$"..."` format string parsing
- `chrono::NaiveDateTime` for `TIMESTAMP` — `DateTime<Utc>` only for `TIMESTAMPTZ`
- Test DB: `kbr_test` on localhost:5432 — shared schema with Rails test database
- `libvips` system library required at runtime (available on test/prod servers)
