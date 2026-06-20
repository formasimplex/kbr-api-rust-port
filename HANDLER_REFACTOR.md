# Handler Refactor Plan

Extract SQL queries from handlers into `src/data/<entity>.rs` files. Each handler is one task, one commit.

## Architecture

```
src/
  data/
    mod.rs              # pub mod for each entity
    campaign_pages.rs   # pub struct CampaignPageRow + FromRow, free functions
    producers.rs
    songs.rs
    ...
```

Each `data/<entity>.rs` file:
- `#[derive(FromRow)] struct EntityRow { ... }` — moved from handler
- `impl From<EntityRow> for Entity` — moved from handler
- Free async functions: `pub async fn list(pool: &PgPool) -> Result<Vec<Entity>, sqlx::Error>`
- Functions accept `&PgPool`, return domain models from `crate::models`

## Per-Handler Inventory

### TIER 1 — GREEN (Simple CRUD, no joins, no transactions)

#### 1. campaign_pages — `data/campaign_pages.rs`
- **Queries**: 2 (index, show)
- **Row structs**: 1 (`CampaignPageRow`)
- **Complexity**: Trivial — read-only, no params beyond ID
- **Tests**: 4 existing tests

#### 2. producers — `data/producers.rs`
- **Queries**: 3 (index, create, update)
- **Row structs**: 1 (`ProducerRow`)
- **Complexity**: Basic CRUD, no joins
- **Tests**: 5 existing tests

#### 3. songs — `data/songs.rs`
- **Queries**: 3 (index, show, create)
- **Row structs**: 1 (`SongRow`)
- **Complexity**: Basic CRUD, no joins
- **Tests**: 5 existing tests

#### 4. albums — `data/albums.rs`
- **Queries**: 3 (index, show, create)
- **Row structs**: 1 (`AlbumRow`)
- **Complexity**: Basic CRUD, no joins
- **Tests**: 5 existing tests

#### 5. permissions — `data/permissions.rs`
- **Queries**: 4 (index, show, create, update)
- **Row structs**: 1 (`PermissionRow`)
- **Complexity**: Basic CRUD, no joins
- **Tests**: 5 existing tests

#### 6. configs — `data/configs.rs`
- **Queries**: 5 (index, show, create, update, destroy)
- **Row structs**: 1 (`ConfigRow`)
- **Complexity**: Basic CRUD, has soft-delete pattern
- **Tests**: 5 existing tests

---

### TIER 2 — YELLOW (Joins, multiple Row structs, moderate complexity)

#### 7. comments — `data/comments.rs`
- **Queries**: 4 (show, index-all, index-by-commentable, create, create-reply)
- **Row structs**: 1 (`CommentRow` with `username` from LEFT JOIN users)
- **Complexity**: LEFT JOIN users for username, CTE in create/create_reply
- **Tests**: 8 existing tests

#### 8. dashboard — `data/dashboard.rs`
- **Queries**: 1 (subscribed_artists)
- **Row structs**: 1 (`DashboardRow` with JOIN artists+users)
- **Complexity**: JOIN across artists/users, S3 calls remain in handler
- **Tests**: 1 existing test

#### 9. event_attendees — `data/event_attendees.rs`
- **Queries**: 4 (qr_scan, attendees_for_event, create, update)
- **Row structs**: 1 (`EventAttendeeRow`)
- **Complexity**: Transaction in create (INSERT + SELECT FOR UPDATE)
- **Tests**: 5 existing tests

#### 10. merchandise — `data/merchandise.rs`
- **Queries**: 6 (index, show, by_artist, create, update, destroy, cache_update)
- **Row structs**: 2 (`MerchandiseRow`, `MerchandiseCacheRow`)
- **Complexity**: LEFT JOIN, Shopify cache integration
- **Tests**: 5 existing tests

#### 11. mailing — `data/mailing.rs`
- **Queries**: 8 (index, index_artist_subscribers, artist_mail_subscriber, add_mail_subscriber_with_user, add_mail_subscriber, unsubscribe, request_unsubscribe, process_unsubscribe)
- **Row structs**: 1 (`MailingRow`)
- **Complexity**: Mailchimp integration, multiple query variants
- **Tests**: 5 existing tests

#### 12. playlists — `data/playlists.rs`
- **Queries**: 10 (index_admin, show_admin, destroy_admin, dashboard_index, dashboard_show, dashboard_create, dashboard_update, dashboard_destroy, dashboard_add_news, dashboard_reorder, dashboard_remove_news)
- **Row structs**: 1 (`PlaylistRow`)
- **Complexity**: Reordering logic, news associations, admin/dashboard split
- **Tests**: 5 existing tests

#### 13. data_api — `data/data_api.rs`
- **Queries**: 3 (last_logins, last_login_by_id, event_attendees_present)
- **Row structs**: 3 (`LastLoginRow`, `LastLoginByIdRow`, `EventAttendeePresentRow`)
- **Complexity**: DISTINCT ON, JOIN mail_subscribers
- **Tests**: 5 existing tests

#### 14. webhook — `data/webhook.rs`
- **Queries**: 3 (update_progress: campaign_page lookup, campaign lookup, campaign update)
- **Row structs**: 2 (`CampaignPageRow`, `CampaignRow`)
- **Complexity**: Mostly stubs, update_progress has business logic
- **Tests**: 6 existing tests

---

### TIER 3 — RED (Transactions, complex business logic, multiple entities)

#### 15. auth — `data/auth.rs`
- **Queries**: 2 (login: user lookup, session: user lookup)
- **Row structs**: 2 (`UserRow`, `UserRowNoPassword`)
- **Complexity**: bcrypt, JWT, timing jitter, cookie handling
- **Tests**: 10 existing tests

#### 16. reset_trigger — `data/reset_trigger.rs`
- **Queries**: 3 (create: user lookup + insert, update: SELECT FOR UPDATE + UPDATE users + DELETE trigger)
- **Row structs**: 1 (`ResetTriggerRow`)
- **Complexity**: Transaction with FOR UPDATE, bcrypt verification, token_version increment
- **Tests**: 10 existing tests

#### 17. sign_up_trigger — `data/sign_up_trigger.rs`
- **Queries**: 3 (create: EXISTS check + UPDATE + INSERT, show: lookup by token)
- **Row structs**: 1 (`SignUpTriggerRow`)
- **Complexity**: EXISTS check, UPDATE to expire old triggers, INSERT
- **Tests**: 7 existing tests

#### 18. users — `data/users.rs`
- **Queries**: 4 (index, show, create-with-tx, update)
- **Row structs**: 1 (`UserRow`)
- **Complexity**: Transaction with FOR UPDATE on sign_up_triggers, email normalization, password hashing, token_version
- **Tests**: 15 existing tests

#### 19. news — `data/news.rs`
- **Queries**: 6 (index, show, create-with-dup-check, update, toggle_comments, add_to_playlist)
- **Row structs**: 1 (`NewsRow`)
- **Complexity**: OG tags fetch, Safe Browsing, duplicate URL check, playlist integration
- **Tests**: 12 existing tests

#### 20. events — `data/events.rs`
- **Queries**: 5 (index, show, index_by_user, create, update) + batch comment fetch
- **Row structs**: 1 (`KbrEventRow`)
- **Complexity**: Batch comment fetching with JOIN, S3 integration, Safe Browsing
- **Tests**: 10 existing tests

#### 21. artists — `data/artists.rs`
- **Queries**: 8 (index, show, create, sign_up-with-tx, update, add_artist_links, delete_artist_links, available_link_types)
- **Row structs**: 1 (`ArtistRow`)
- **Complexity**: Transaction for sign_up, S3 uploads, artist links batch fetch, multipart
- **Tests**: 10 existing tests

#### 22. campaigns — `data/campaigns.rs`
- **Queries**: 8 (index, index_by_user, active_campaigns, show, create, update, destroy, activate_campaign)
- **Row structs**: 2 (`CampaignRow`, `CampaignPageRow`)
- **Complexity**: Shopify GraphQL, activation flow, campaign pages
- **Tests**: 10 existing tests

---

### SKIP (no SQL to extract)

- **health** — no DB access
- **storage** — all DB access goes through `storage_service`

---

## Execution Order

1. `campaign_pages` — simplest, read-only
2. `producers` — basic CRUD
3. `songs` — basic CRUD
4. `albums` — basic CRUD
5. `permissions` — basic CRUD
6. `configs` — basic CRUD with soft-delete
7. `comments` — first with JOIN
8. `dashboard` — JOIN + S3
9. `event_attendees` — first with transaction
10. `merchandise` — LEFT JOIN + Shopify
11. `mailing` — multiple query variants
12. `playlists` — reordering logic
13. `data_api` — multiple Row structs
14. `webhook` — mostly stubs
15. `auth` — bcrypt + JWT
16. `reset_trigger` — transaction with FOR UPDATE
17. `sign_up_trigger` — EXISTS + UPDATE + INSERT
18. `users` — complex transaction
19. `news` — OG tags + Safe Browsing
20. `events` — batch comments + S3
21. `artists` — transaction + S3 + multipart
22. `campaigns` — Shopify GraphQL

## Workflow Per Handler

1. **Read** the handler, identify all `sqlx::query_as` / `sqlx::query` / `sqlx::query_scalar` calls
2. **Create** `src/data/<entity>.rs` with `*Row` struct + `From` impl + free functions
3. **Update** handler to call `data::<entity>::function(&state.db, ...)`
4. **Run** `cargo test --lib` to verify
5. **Commit** with message: `refactor: extract <entity> SQL into data/<entity>.rs`
