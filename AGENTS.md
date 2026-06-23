# AGENTS.md — kbr-api-rust

## Overview
Rust replacement for the `kbr-api` Rails backend. Shares the same PostgreSQL schema during a migration transition period.

## Stack
- Rust 1.95.0, edition 2024
- `actix-web` 4.x (HTTP framework)
- `sqlx` 0.8.x (async PostgreSQL)
- `jsonwebtoken` (JWT auth)
- `bcrypt` (password hashing)
- `tokio` (async runtime)
- `serde` / `serde_json` (serialization)
- `tracing` (logging)

## Key Conventions
- All async tests must use `#[tokio::test(flavor = "current_thread")]`
- `std::env::{set_var, remove_var}` are `unsafe` in edition 2024 — wrap in `unsafe {}`
- Avoid string literals for permission resources — use `PermissionResource` / `RESOURCES`
- `AppError` is the single error type; implement `ResponseError` for HTTP responses
- `AppState` holds shared state (`PgPool`); routes borrow via `web::Data<AppState>`

## Directory Structure
```
src/
  main.rs          — server bootstrap, routes, AppState
  lib.rs           — module re-exports
  error.rs         — AppError enum + ResponseError impl
  bin/
    kbr_migrate.rs — migration CLI (migrate, rollback, status, check)
  auth/
    jwt.rs         — Claims, encode/decode, get_jwt_secret
    roles.rs       — Role enum
    permissions.rs — PermissionResource, has_permission, is_admin, is_artist_or_above, is_customer_or_above
    resources.rs   — RESOURCES constant, ResourceNames
    middleware.rs  — CurrentUser extractor, FromRequest
  db/
    pool.rs        — PgPool connection setup
    migrate.rs     — sqlx::migrate runner, health check
  handlers/        — HTTP handlers (one file per resource)
  models/          — DB models (sqlx types)
  responses/       — response serializers
  services/        — business logic
```

## Commands
- `cargo test --lib` — run all unit tests
- `cargo build` — build binary
- `cargo test` — run all tests (including integration)
- `cargo run --bin kbr-migrate -- migrate` — run pending migrations
- `cargo run --bin kbr-migrate -- check` — schema health check

## Reference
- Rails source: `/Users/ws/formasimplex/kbr-api`
- Rust patterns: `/Users/ws/formasimplex/tenant_daemon`
