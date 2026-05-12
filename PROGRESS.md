# PROGRESS.md — kbr-api-rust

## Plan: 8-Stage Port
| Stage | Scope | Status |
|-------|-------|--------|
| 0 | Infrastructure (error, DB, JWT, roles, health) | ✅ Done (41 tests) |
| 1 | Auth + Users (login, session, user CRUD, permissions) | ✅ Done |
| 2 | Core Content (albums, songs, artists, campaigns, etc.) | ✅ Done |
| 3 | Social (comments, news, playlists) | ✅ Done |
| 4 | Events + Mailing | ✅ Done |
| 5 | Commerce + Config | ✅ Done (configs, merchandise) |
| 6 | External Services (S3/Storage) | Pending |
| 7 | Webhooks | Pending |

## Stage 0 — Infrastructure (Done)
- `error.rs` — `AppError` + `ResponseError` (9 tests)
- `db/pool.rs` — `PgPool` from `DATABASE_URL` (2 tests)
- `auth/jwt.rs` — `Claims`, `encode_token`, `decode_token`, `encode_token_with_role` (10 tests)
- `auth/roles.rs` — `Role`, guards, `PermissionResource`, `RESOURCES` (13 tests)
- `auth/middleware.rs` — `CurrentUser` extractor (6 tests)
- `handlers/health.rs` — `/health` endpoint (1 test)
- `main.rs` — server bootstrap, `AppState`, route config

## Stage 1 — Auth + Users (Done)
- `services/auth_service.rs` — login, session, bcrypt hashing, JWT creation (8 tests)
- `services/user_service.rs` — user validation, credential verification (10 tests)
- `services/permission_service.rs` — permission checks, resource building (4 tests)
- `handlers/auth.rs` — login endpoint, mock bcrypt hashes (4 tests)
- `handlers/users.rs` — user CRUD, role-based access (12 tests)
- `handlers/permissions.rs` — permission CRUD (10 tests)
- `handlers/sign_up_trigger.rs` — sign-up trigger endpoints (4 tests)
- `handlers/reset_trigger.rs` — reset trigger endpoints (4 tests)
- `models/user.rs` — `validate_email`, `validate_password`, `validate_role`
- `models/permission.rs` — permission model, helper builders
- `models/sign_up_trigger.rs` — sign-up trigger model, expiry check
- `models/reset_trigger.rs` — reset trigger model

## Stage 2 — Core Content (Done)
- `handlers/albums.rs` — album CRUD (4 tests)
- `handlers/songs.rs` — song CRUD (4 tests)
- `handlers/artists.rs` — artist CRUD, link validation (6 tests)
- `handlers/producers.rs` — producer CRUD (4 tests)
- `handlers/campaigns.rs` — campaign CRUD, vinyl count validation (6 tests)
- `handlers/campaign_pages.rs` — campaign page CRUD (4 tests)
- `handlers/merchandise.rs` — merchandise CRUD (4 tests)
- `handlers/configs.rs` — config CRUD, field validation (6 tests)
- `models/album.rs` — album model, response serialization
- `models/song.rs` — song model
- `models/artist.rs` — `validate_intro`, `validate_bio`
- `models/artist_link.rs` — `validate_url`, link type validation
- `models/producer.rs` — producer model
- `models/campaign.rs` — `validate_vinyl_sold_count`, `is_deleted`
- `models/campaign_page.rs` — campaign page model
- `models/artist_merchandise.rs` — merchandise model
- `models/tenant_config.rs` — config model, `is_deleted`

## Stage 3 — Social (Done)
- `handlers/comments.rs` — comment CRUD, commentable type validation (6 tests)
- `handlers/news.rs` — news CRUD, URL safety checks (6 tests)
- `handlers/playlists.rs` — playlist CRUD, dashboard endpoints (12 tests)
- `models/comment.rs` — `valid_commentable_type`, `is_reply`
- `models/news.rs` — `is_malicious_url`
- `models/news_playlist.rs` — playlist model, request/response types
- `models/users_news.rs` — users-news join model

## Stage 4 — Events + Mailing (Done)
- `handlers/events.rs` — event CRUD (6 tests)
- `handlers/event_attendees.rs` — attendee CRUD, query param parsing (8 tests)
- `handlers/mailing.rs` — subscriber CRUD, query param parsing (8 tests)
- `models/kbr_event.rs` — event model
- `models/kbr_event_attendee.rs` — attendee model, `has_scanned`
- `models/mail_subscriber.rs` — `validate_email`, `is_subscribed`

## Stage 5 — Commerce + Config (Done)
- Configs and merchandise handlers/models covered in Stages 2-4

## Test Summary
- **254 tests passing** (all unit + handler integration tests)
- **Clean build** (zero warnings)

## Key Decisions
- Mock data in handlers for TDD without DB dependency (will be replaced with SQLx queries)
- `web::Query<serde_json::Value>` for flexible query param parsing
- `web::Json<serde_json::Value>` for flexible request body parsing
- `OnceLock` for thread-safe lazy bcrypt hash generation in test mocks
- `role` claim in JWT to avoid DB lookups in middleware during testing
- `release_date` as `Option<String>` to avoid `chrono::Date` deprecation
- `#[allow(dead_code)]` at crate level — model/service helpers prepared for SQLx integration

## Notes
- Rust replaces Rails; shared PostgreSQL DB only during transition
- S3/Storage is highest priority among external integrations
- Background jobs (email queueing) deferred to later stages
