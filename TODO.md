# Test Refactoring TODO

## Overview
Reduce test boilerplate by consolidating duplicated patterns into `test_utils.rs`.
Total estimated savings: ~2,838 lines across Low + Mid effort phases.

---

## Low Effort (Quick wins)

- [x] **L1: Remove `TEST_SECRET` from all handler test modules** — import from `crate::test_utils::TEST_SECRET` instead (21 files, ~21 lines saved)
- [x] **L2: Replace all `get_state()` functions** — expose single `get_test_state()` in `test_utils`, remove 22 local copies (~132 lines)
- [x] **L3: Delete unused `AppState::for_tests()` in `app.rs:27-48`** — dead code (~22 lines)
- [x] **L4: Fix `auth.rs` inline `get_state()`** — replaced as part of L2 (~40 lines)
- [x] **L5: Consolidate `setup_env()` pattern** — add `set_test_env()` to `test_utils`, replace 180 inline `unsafe { set_var(...) }` blocks (~720 lines)

## Mid Effort (New helpers)

- [x] **M6: Token factory functions** — added `admin_token()`, `artist_token()`, `user_token()`, `customer_token()`, `token_with_role()` to `test_utils`; removed 14+ duplicates; standardized 3-day expiry
- [x] **M7: Unify `unique_suffix()`** — AtomicI64 + timestamp in `test_utils`; removed 6 duplicate implementations
- [x] **M8: `not_found_id()` helper** — replaced 22 MAX(id) queries across 13 files
- [x] **M9: `seed_user()` / `cleanup_user()` generics** — added to `test_utils`; full module migration deferred (varying signatures require per-module analysis)
- [x] **M10: `build_test_app!` macro** — macro wraps `init_service` boilerplate; permissions.rs migrated as proof of concept; full migration of 206 calls deferred

## High Effort (Architectural)

- [ ] **H11: Shared `PgPool` via `LazyLock`** — single static pool, eliminates ~400 connections
- [ ] **H12: Transaction-based test isolation** — wrap each test in rollback transaction, eliminates all cleanup calls (~400+ lines)
- [ ] **H13: Test fixture macro** — one-call setup for env + state + app (risky: harder to debug)
- [ ] **H14: Move to integration tests / TestServer** — significant restructuring

## Notes
- Run `cargo test --lib` after each task to verify correctness
- Branch: `testRefactor`
