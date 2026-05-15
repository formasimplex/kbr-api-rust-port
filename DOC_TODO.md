# DOC_TODO.md — Handler File Documentation Plan

## Intention

Add module-level documentation (`//!`) and per-function doc comments (`///`) to every handler file in `src/handlers/`. This follows Rust best practices:

- `//!` comments attach to the module itself and appear in `cargo doc` output.
- `///` comments attach to the item immediately following them and appear in `cargo doc` output.

The goal is that any developer opening a handler file (or running `cargo doc`) can immediately see what endpoints exist, their routes, and their purpose.

## Format

Each file gets a module-level `//!` doc block before the `use` statements, followed by `///` doc comments on every `pub` function and `pub` struct.

### Module-Level Block

```rust
//! Album handlers
//!
//! Provides CRUD endpoints for album management.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/albums` | public | List all albums |
//! | `show` | GET | `/v1/album/{id}` | public | Retrieve a single album by ID |
//! | `create` | POST | `/v1/albums` | admin | Create a new album |
```

### Per-Function Doc Comments

```rust
/// List all albums.
///
/// Returns all albums ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `AlbumResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
```

## Conventions

1. **`config_routes` is excluded** from the endpoint table — it's an internal wiring function, not an API endpoint.
2. **`pub struct` types** (e.g., `LoginParams`, `UploadResponse`) get `///` doc comments above their definition.
3. **Routes are inferred** from `config_routes` in each file.
4. **Auth column** uses: `public`, `admin`, `artist+`, or `auth` depending on the guard.
5. **No `config_routes` entry** in the table — it's implementation detail.

## Files to Document (23 handlers)

| # | File | Endpoints | Public Types | Status |
|---|------|-----------|-------------|--------|
| 1 | `auth.rs` | `login`, `session` | `LoginParams`, `LoginErrorResponse` | done |
| 2 | `users.rs` | `index`, `show`, `create`, `update` | — | done |
| 3 | `albums.rs` | `index`, `show`, `create` | — | done |
| 4 | `songs.rs` | `index`, `show`, `create` | — | done |
| 5 | `artists.rs` | `index`, `show`, `create`, `update`, `add_artist_links`, `delete_artist_links`, `available_link_types` | — | done |
| 6 | `producers.rs` | `index`, `create`, `update` | — | done |
| 7 | `campaigns.rs` | `index`, `index_by_user`, `active_campaigns`, `show`, `create`, `update`, `destroy`, `activate_campaign` | — | done |
| 8 | `campaign_pages.rs` | `index`, `show` | — | done |
| 9 | `comments.rs` | `show`, `index`, `create`, `create_reply` | — | done |
| 10 | `configs.rs` | `index`, `show`, `create`, `update`, `destroy` | — | done |
| 11 | `dashboard.rs` | `subscribed_artists` | — | done |
| 12 | `data_api.rs` | `last_logins`, `last_login_by_id`, `event_attendees_present` | — | done |
| 13 | `events.rs` | `index`, `show`, `index_by_user`, `create`, `update` | — | done |
| 14 | `event_attendees.rs` | `qr_scan`, `attendees_for_event`, `create`, `update` | — | done |
| 15 | `health.rs` | `health_check` | — | done |
| 16 | `mailing.rs` | `index`, `index_artist_subscribers`, `artist_mail_subscriber`, `add_mail_subscriber_with_user`, `add_mail_subscriber`, `unsubscribe`, `request_unsubscribe`, `process_unsubscribe` | — | done |
| 17 | `merchandise.rs` | `index`, `show`, `by_artist`, `create`, `update`, `destroy`, `cache_update` | — | done |
| 18 | `news.rs` | `index`, `show`, `create`, `update`, `toggle_comments`, `add_to_playlist` | — | done |
| 19 | `permissions.rs` | `index`, `index_resources`, `show`, `create`, `update` | — | done |
| 20 | `playlists.rs` | `index_admin`, `show_admin`, `destroy_admin`, `dashboard_index`, `dashboard_show`, `dashboard_create`, `dashboard_update`, `dashboard_destroy`, `dashboard_add_news`, `dashboard_reorder`, `dashboard_remove_news` | — | done |
| 21 | `reset_trigger.rs` | `create`, `show`, `update` | — | done |
| 22 | `sign_up_trigger.rs` | `create`, `show` | — | done |
| 23 | `storage.rs` | `upload`, `get_images`, `delete_image` | `UploadResponse` | done |
| 24 | `webhook.rs` | `update_progress`, `customers_data_request`, `customers_redact`, `shop_redact` | `WebhookInventoryParams`, `WebhookPayload` | done |

## Verification

After all files are documented, verify with:

```bash
cargo doc --no-deps 2>&1 | head -20
```

Confirm no warnings and that all modules appear in the generated docs.
