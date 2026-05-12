# TODO.md — kbr-api-rust

## Status: All Handlers SQLx-Ready, Missing Endpoints + External Services Remaining

### ✅ All 19 Handlers Converted to Real SQLx Queries (305 tests passing)

Every handler now queries PostgreSQL via `web::Data<AppState>`. Zero mock data remains.

---

## Remaining Work

### Priority 1: Missing Rails Endpoints

| Rails Controller | Endpoints | Complexity |
|---|---|---|
| `DataApiController` | `GET /v1/data/last_logins`, `GET /v1/data/last_logins/:id`, `GET /v1/data/event_attendees_present/:id` | Low — read-only |
| `GenerateTextController` | `GET /v1/generate_text/cp/:type`, `GET /v1/generate_bio/:type` | Medium — OpenAI |
| `UnsubscribeController` | `POST /v1/unsubscribe`, `GET /v1/unsubscribe/:token` | Low-Medium — JWT flow |
| `WebhookController` | `POST /v1/webhook/update_progress`, `customers_data_request`, `customers_redact`, `shop_redact` | Medium — Shopify + GDPR |

### Priority 2: External Service Integrations

| Service | Used By | Complexity |
|---|---|---|
| AWS S3 / ActiveStorage | Artist images, Campaign images, Album images | High — presigned URLs |
| Shopify GraphQL | Campaign activation, merchandise cache, webhook progress | High — GraphQL client |
| Mailchimp REST | Subscriber add/unsubscribe | Medium — POST/PATCH |
| OpenAI REST | AI text generation (bio, campaign descriptions) | Medium — REST API |
| Google Safe Browsing | URL safety check on news creation | Low — single API call |

### Priority 3: Background Jobs & Email

| Job | Purpose |
|---|---|
| `CreateCampaignJob` | Creates Album + Campaign + enqueues CampaignSetupJob |
| `CampaignSetupJob` | Creates CampaignPage, Songs, ArtistLinks |
| `SendEventAttendeeEmailJob` | Sends QR code email |
| `SendEventUpdateEmailJob` | Sends text update email |
| 6 Mailer deliveries | Welcome, reset, signup, unsubscribe, QR, update |

Need decision: in-process queue vs Redis/BullMQ/SQS.

### Priority 4: Cleanup

- 1 flaky test: `get_jwt_secret_returns_custom_and_default` (env var race condition)
- 10 unused `encode_token` imports across handler test modules
- 4 camelCase field warnings in `configs.rs`
