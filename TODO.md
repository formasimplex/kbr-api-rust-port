# TODO.md — kbr-api-rust

## Status: Phase 3 + 4 Complete (Mailchimp, Safe Browsing, Shopify GraphQL), 327 tests passing

### ✅ All 25 Handlers Converted to Real SQLx Queries (327 tests passing)

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
| `GET /v1/available_link_types` | `artists.rs` | Low — hardcoded enum (11 types) | ✅ Done |
| `POST /v1/artist/add_artist_links` | `artists.rs` | Low — bulk DB insert, auth check | ✅ Done |
| `DELETE /v1/artist/delete_artist_links` | `artists.rs` | Low — bulk DB delete, auth check | ✅ Done |
| `POST /v1/news/:id/toggle_comments` | `news.rs` | Low — DB toggle, owner/admin auth | ✅ Done |
| `GET /v1/artist_merchandise/by_artist/:artist_id` | `merchandise.rs` | Medium — pagination, eager load shopify cache | ✅ Done |
| `GET /v1/dashboard/subscribed_artists` | `dashboard.rs` (new) | Medium — join through mail_subscribers | ✅ Done |

### Priority 3: External Service Integrations

| Service | Used By | Complexity | Status |
|---|---|---|---|
| Shopify GraphQL | Merchandise sync (FE button), campaign activation | High — GraphQL client, 7 operations | ✅ Done (activation) |
| Mailchimp REST | Subscriber add/unsubscribe | Medium — REST POST/PATCH | ✅ Done |
| Google Safe Browsing | URL safety check on news creation | Low — single API call | ✅ Done |
| OpenAI REST | AI text generation | Medium — REST API | ⏭️ Skipped (never used in production) |

### Priority 4: Background Jobs & Email

**Decision: In-process queue** (tokio tasks with bounded channels). Email deferred until queue is built — if volume is high, emails will be queued through the same system.

| Job / Mailer | Purpose | Status |
|---|---|---|
| `CreateCampaignJob` | Creates Album + Campaign + enqueues CampaignSetupJob | ⬜ Not started |
| `CampaignSetupJob` | Creates CampaignPage, Songs, ArtistLinks | ⬜ Not started |
| `SendEventAttendeeEmailJob` | Sends QR code email | ✅ Done |
| `SendEventUpdateEmailJob` | Sends text update email | ✅ Done |
| `ProspectMailer` | Welcome email | ✅ Done |
| `ResetTriggerMailer` | Password reset email | ✅ Done |
| `UserSignUpTriggerMailer` | Sign-up confirmation email | ✅ Done |
| `UnsubscribeMailer` | Unsubscribe confirmation email | ✅ Done |
| `KbrEventAttendeeMailer` | Event QR code email | ✅ Done |
| `KbrEventUpdateAttendeeMailer` | Event update email | ✅ Done |

### Execution Plan

| Phase | Tasks | Est. Time | Status |
|-------|-------|-----------|--------|
| **Phase 1** | CORS + Graceful Shutdown | 2h | ✅ Done |
| **Phase 2** | 6 missing endpoints | 6h | ✅ Done |
| **Phase 3** | Mailchimp + Safe Browsing | 4h | ✅ Done |
| **Phase 4** | Shopify GraphQL (merch sync + campaign activation) | 6h | ✅ Done |
| **Phase 5** | In-process queue + Jobs + Email | 15-20h | 🟡 ~90% done |

### Completed

- ✅ All 25 handlers with real SQLx queries (327 tests passing)
- ✅ S3/ActiveStorage integration (Linode Object Storage, rs-vips image processing)
- ✅ All clippy warnings resolved (0 remaining across 26 files)
- ✅ Data API endpoints (`last_logins`, `event_attendees_present`)
- ✅ Webhook endpoints (`update_progress`, `customers_data_request`, `customers_redact`, `shop_redact`)
- ✅ Unsubscribe flow (`POST /v1/unsubscribe`, `GET /v1/unsubscribe/:token` in mailing.rs)
- ✅ Dashboard playlists (full CRUD + reorder)
- ✅ Dashboard subscribed artists (`GET /v1/dashboard/subscribed_artists`)
- ✅ Artist merchandise (full CRUD + `by_artist` endpoint)
- ✅ Artist links (`available_link_types`, `add_artist_links`, `delete_artist_links`)
- ✅ News comments toggle (`POST /v1/news/:id/toggle_comments`)
- ✅ Shopify GraphQL integration (ShopifyClient, ShopifyGraphQl, 6 operations)
- ✅ Campaign activation rewrite (full Shopify flow: product, variant, publish)
- ✅ Mailchimp REST integration (MailchimpClient, subscribe/unsubscribe, non-fatal sync)
- ✅ Google Safe Browsing (SafeBrowsingClient, URL threat check on news creation, fail-open)
- ✅ In-process job queue (tokio mpsc channel, 256 capacity, single worker task)
- ✅ SMTP email service (lettre-based, attachments, optional env config)
- ✅ Email job handlers (event QR, event update, sign-up trigger, reset trigger)
- ✅ Email templates (6: qr_code, sign_up_trigger, text_copy, reset_trigger, prospect_welcome, unsubscribe)
- ✅ QR code generation (qrcode + image crate, PNG output)
- ✅ Sign-up trigger handler (`POST /v1/sign_up_trigger`, `GET /v1/sign_up_trigger/:token`)
- ✅ Reset trigger handler (`POST /v1/reset_trigger`, `POST /v1/reset_trigger/:token`, rate limiting, timing jitter)
- ✅ AppState integration (email: Option<EmailClient>, job_handle: JobHandle)
- ✅ AppError::Email variant
- ✅ Job worker spawned at server startup (setup.rs)
