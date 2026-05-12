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
  auth/
    jwt.rs         — Claims, encode/decode, get_jwt_secret
    roles.rs       — Role enum, guards, PermissionResource, RESOURCES
    middleware.rs  — CurrentUser extractor, FromRequest
  db/
    pool.rs        — PgPool connection setup
  handlers/        — HTTP handlers (one file per resource)
  models/          — DB models (sqlx types)
  responses/       — response serializers
  services/        — business logic
```

## Commands
- `cargo test --lib` — run all unit tests
- `cargo build` — build binary
- `cargo test` — run all tests (including integration)

## Reference
- Rails source: `/Users/ws/formasimplex/kbr-api`
- Rust patterns: `/Users/ws/formasimplex/tenant_daemon`
