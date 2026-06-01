# Test Refactoring TODO

## Overview
Reduce test boilerplate by consolidating duplicated patterns into `test_utils.rs`.
Total estimated savings: ~2,838 lines across Low + Mid effort phases.

---

## Low Effort (Quick wins)

- [x] **L1: Remove `TEST_SECRET` from all handler test modules** — import from `crate::test_utils::TEST_SECRET` instead (21 files, ~21 lines saved)
- [ ] **L2: Replace all `get_state()` functions** — expose single `get_test_state()` in `test_utils`, remove 22 local copies (~132 lines)
- [ ] **L3: Delete unused `AppState::for_tests()` in `app.rs:27-48`** — dead code (~22 lines)
- [ ] **L4: Fix `auth.rs` inline `get_state()`** — replace 40-line duplicate with `build_test_state` call (~40 lines)
- [ ] **L5: Consolidate `setup_env()` pattern** — add `set_test_env()` to `test_utils`, replace 180 inline `unsafe { set_var(...) }` blocks (~720 lines)

## Mid Effort (New helpers)

- [ ] **M6: Token factory functions** — add `admin_token()`, `artist_token()`, `user_token()`, `customer_token()` to `test_utils` (~48 lines)
- [ ] **M7: Unify `unique_suffix()`** — pick AtomicI64 + timestamp approach, remove 25+ duplicated functions (~125 lines)
- [ ] **M8: `not_found_id()` helper** — generic helper for MAX(id) pattern, replace 22 queries (~110 lines)
- [ ] **M9: `seed_user()` / `cleanup_user()` generics** — parameterized versions in `test_utils` (~300 lines)
- [ ] **M10: `build_test_app()` wrapper** — wrap `test::init_service` boilerplate (~1,320 lines)

## High Effort (Architectural)

- [ ] **H11: Shared `PgPool` via `LazyLock`** — single static pool, eliminates ~400 connections
- [ ] **H12: Transaction-based test isolation** — wrap each test in rollback transaction, eliminates all cleanup calls (~400+ lines)
- [ ] **H13: Test fixture macro** — one-call setup for env + state + app (risky: harder to debug)
- [ ] **H14: Move to integration tests / TestServer** — significant restructuring

## Notes
- Run `cargo test --lib` after each task to verify correctness
- Branch: `testRefactor`
