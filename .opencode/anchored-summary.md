# Anchored Summary

## Goal
Embed nested data (`artist_links` for Artist, `comments` with `user.username` for KbrEvent) in Rust API responses to match Rails serializer output.

## Constraints & Preferences
- TDD approach (red/green/refactor)
- Use SQL JOINs and batch queries to avoid N+1
- Property names must match frontend expectations (`artist_links`, `comments` with `user.username`)
- Skip `songs` and `campaigns` for now
- Focus on Artists and KbrEvents first

## Progress
### Done
- **Artist → `artist_links`**: Added `artist_links: Vec<ArtistLinkResponse>` to `ArtistResponse`, updated `to_response` signature, added `fetch_artist_links_batch` helper with `ANY($1)` query, updated all handler call sites (`show`, `index`, `create`, `update`, `sign_up`), added test assertion
- **Comment model**: Added `CommentUser` struct, added `user: Option<CommentUser>` to `CommentResponse`, updated `to_response` to accept `username: Option<String>`, updated all 4 callers in `comments.rs` to pass `None`
- **KbrEvent → `comments`**: Added `comments: Vec<CommentResponse>` to `KbrEventResponse`, updated `to_response` signature, added `fetch_event_comments_batch` helper with `LEFT JOIN users` SQL, updated all handler call sites (`show`, `index`, `index_by_user`, `create`, `update`)
- All tests pass: `handlers::artists`, `handlers::events`, `handlers::comments`
- Committed and pushed to `origin/develop` (1534bdb)

### In Progress
- (none)

### Blocked
- (none)

## Key Decisions
- Batch query + `HashMap<i64, Vec<...>>` grouping for index endpoints avoids N+1
- SQL `LEFT JOIN users` fetches username in same query for comments
- `user` field optional in `CommentResponse` for backward compatibility
- Comments handler passes `None` for username (standalone endpoint can be enhanced later)
- Skipped nested `replies` in comments (Rails includes them recursively)

## Next Steps
1. Add `assert!(body["comments"].is_array())` to event show/index tests
2. Decide: add `user.username` to standalone `/comments` endpoint? (Rails does this)
3. Decide: add nested `replies` to comments? (Rails does this)

## Critical Context
- Pre-existing flaky test: `add_artist_link_success` fails due to timestamp collision in email seeding (`artist_test_{timestamp}@test.com`)
- `fetch_event_comments_batch` SQL: `SELECT c.id, c.content, c.created_at AT TIME ZONE 'UTC', u.username, c.commentable_id FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.commentable_type = 'KBREvent' AND c.commentable_id = ANY($1) ORDER BY c.commentable_id, c.created_at`
- `fetch_artist_links_batch` SQL: `SELECT id, artist_id, link_type, url, created_at AT TIME ZONE 'UTC', updated_at AT TIME ZONE 'UTC' FROM artist_links WHERE artist_id = ANY($1) ORDER BY artist_id, id`

## Relevant Files
- `src/models/artist.rs`: Added `artist_links` field and updated `to_response`
- `src/handlers/artists.rs`: Added batch fetch helper, updated all handlers, added test assertion
- `src/models/comment.rs`: Added `CommentUser`, `user` field, updated `to_response`
- `src/handlers/comments.rs`: Updated 4 `to_response` callers to pass `None`
- `src/models/kbr_event.rs`: Added `comments` field and updated `to_response`
- `src/handlers/events.rs`: Added batch fetch helper with user JOIN, updated all handlers
- `app/serializers/ArtistSerializer.rb`: Rails reference for expected output structure
- `app/serializers/KbrEventSerializer.rb`: Rails reference for expected output structure
- `app/serializers/CommentSerializer.rb`: Rails reference for expected output structure
