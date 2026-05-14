# TODO.md — kbr-api-rust

## Status: Phase 1 Complete (CORS + Graceful Shutdown), 330 tests passing

### ✅ All 24 Handlers Converted to Real SQLx Queries (316 tests passing)

Every handler now queries PostgreSQL via `web::Data<AppState>`. Zero mock data remains. S3/ActiveStorage integration complete.

---

## Remaining Work

### Priority 1: Infrastructure

| Task | Description | Status |
|---|---|---|
| Graceful Shutdown | Signal handling (SIGTERM/SIGINT) in `main.rs` for clean Actix server shutdown | ✅ Done |
| CORS | `actix-cors`, origins from `CORS_ORIGINS` env var | ✅ Done |

### Priority 2: Missing Rails Endpoints

| Rails Route | Handler | Complexity | Status |
|---|---|---|---|
| `GET /v1/available_link_types` | `artists.rs` | Low — hardcoded enum (11 types) | ⬜ Not started |
| `POST /v1/artist/add_artist_links` | `artists.rs` | Low — bulk DB insert, auth check | ⬜ Not started |
| `DELETE /v1/artist/delete_artist_links` | `artists.rs` | Low — bulk DB delete, auth check | ⬜ Not started |
| `POST /v1/news/:id/toggle_comments` | `news.rs` | Low — DB toggle, owner/admin auth | ⬜ Not started |
| `GET /v1/artist_merchandise/by_artist/:artist_id` | `merchandise.rs` | Medium — pagination, eager load shopify cache | ⬜ Not started |
| `GET /v1/dashboard/subscribed_artists` | `dashboard.rs` (new) | Medium — join through mail_subscribers | ⬜ Not started |

### Priority 3: External Service Integrations

| Service | Used By | Complexity | Status |
|---|---|---|---|
| Shopify GraphQL | Merchandise sync (FE button), campaign activation | High — GraphQL client, 7 operations | ⬜ Not started |
| Mailchimp REST | Subscriber add/unsubscribe | Medium — REST POST/PATCH | ⬜ Not started |
| Google Safe Browsing | URL safety check on news creation | Low — single API call | ⬜ Not started |
| OpenAI REST | AI text generation | Medium — REST API | ⏭️ Skipped (never used in production) |

### Priority 4: Background Jobs & Email

**Decision: In-process queue** (tokio tasks with bounded channels). Email deferred until queue is built — if volume is high, emails will be queued through the same system.

| Job / Mailer | Purpose | Status |
|---|---|---|
| `CreateCampaignJob` | Creates Album + Campaign + enqueues CampaignSetupJob | ⬜ Not started |
| `CampaignSetupJob` | Creates CampaignPage, Songs, ArtistLinks | ⬜ Not started |
| `SendEventAttendeeEmailJob` | Sends QR code email | ⬜ Not started |
| `SendEventUpdateEmailJob` | Sends text update email | ⬜ Not started |
| `ProspectMailer` | Welcome email | ⬜ Not started |
| `ResetTriggerMailer` | Password reset email | ⬜ Not started |
| `UserSignUpTriggerMailer` | Sign-up confirmation email | ⬜ Not started |
| `UnsubscribeMailer` | Unsubscribe confirmation email | ⬜ Not started |
| `KbrEventAttendeeMailer` | Event QR code email | ⬜ Not started |
| `KbrEventUpdateAttendeeMailer` | Event update email | ⬜ Not started |

### Execution Plan

| Phase | Tasks | Est. Time |
|-------|-------|-----------|
| **Phase 1** | CORS + Graceful Shutdown | 2h |
| **Phase 2** | 6 missing endpoints | 6h |
| **Phase 3** | Mailchimp + Safe Browsing | 4h |
| **Phase 4** | Shopify GraphQL (merch sync + campaign activation) | 6h |
| **Phase 5** | In-process queue + Jobs + Email | 15-20h |

### Completed

- ✅ All 24 handlers with real SQLx queries (316 tests passing)
- ✅ S3/ActiveStorage integration (Linode Object Storage, rs-vips image processing)
- ✅ All clippy warnings resolved (0 remaining across 25 files)
- ✅ Data API endpoints (`last_logins`, `event_attendees_present`)
- ✅ Webhook endpoints (`update_progress`, `customers_data_request`, `customers_redact`, `shop_redact`)
- ✅ Unsubscribe flow (`POST /v1/unsubscribe`, `GET /v1/unsubscribe/:token` in mailing.rs)
- ✅ Dashboard playlists (full CRUD + reorder)
- ✅ Artist merchandise (full CRUD)
- ✅ Campaign activation (DB-only, Shopify integration pending)
