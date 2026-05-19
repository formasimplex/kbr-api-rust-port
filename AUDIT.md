# Security Audit Remediation Plan — kbr-api-rust

**Date:** May 15, 2026
**Scope:** `/Users/ws/formasimplex/kbr-api-rust/src`
**Stack:** Rust 1.95 / actix-web 4.x / sqlx 0.8.x / jsonwebtoken 9.x / bcrypt 0.17.x / lettre 0.11 / qrcode 0.14 / image 0.25
**Methodology:** OWASP Top 10:2025

---

## Executive Summary

46 findings across 4 severity levels. Remediation is organized into 5 phases ordered by effort-to-impact ratio. Phase 1 targets quick wins (<1 hour total). Phase 2 closes the remaining critical/auth gaps. Phase 3 hardens crypto and token lifecycle. Phase 4 addresses the email subsystem. Phase 5 covers operational hardening and backlog items.

| Phase | Name | Effort | Findings |
|-------|------|--------|----------|
| **Phase 1** | Quick Wins | ~1 hour | C-1, H-2, H-3, H-7, M-9, I-2 |
| **Phase 2** | Auth & Authorization Core | ~4 hours | C-2, C-3, C-4, C-5, C-6, C-7, C-8, H-5, H-6, H-9 |
| **Phase 3** | Crypto & Token Lifecycle | ~3 hours | H-1, H-10, M-1, M-7 |
| **Phase 4** | Email Subsystem | ~4 hours | H-12, H-13, M-10, M-11, M-12, M-13, L-6 |
| **Phase 5** | Hardening & Operational | ~6 hours | H-4, H-8, H-11, M-2, M-3, M-4, M-5, M-6, M-8, L-1, L-2, L-3, L-4, L-7, I-1, I-3, I-4, I-5 |

---

## Phase 1 — Quick Wins (~1 hour)

Small, isolated changes with immediate security impact. No dependencies on other phases.

### [x] C-1: Hardcoded JWT Secret Fallback — Authentication Bypass
- **Severity:** Critical | **Category:** A04
- **Location:** `src/auth/middleware.rs:7-10`
- **Status:** ✅ Fixed — `get_jwt_secret()` now returns `Result<String, AppError>` with no fallback. `from_request` rejects the request when the secret is unavailable.

### [ ] H-2: JWT Algorithm Not Explicitly Restricted
- **Severity:** High | **Category:** A04
- **Location:** `src/auth/jwt.rs:41`
- **Why quick win:** One line addition to validation config.
- **Remediation:** `validation.algorithms = vec![Algorithm::HS256];`

### [ ] H-3: Database Error Details Leaked to Clients
- **Severity:** High | **Category:** A10
- **Location:** `src/error.rs:43-66`
- **Why quick win:** One-line change in the `Database` arm of the error response.
- **Remediation:** `Self::Database(_) => "A database error occurred"`.

### [ ] H-7: Reset Token Exposed in API Response
- **Severity:** High | **Category:** A04
- **Location:** `src/models/reset_trigger.rs:34-41`
- **Why quick win:** Remove `token` field from response struct.
- **Remediation:** Remove `token` from `ResetTriggerResponse`; communicate via email only.

### [ ] M-9: Server Version Information Leakage
- **Severity:** Medium | **Category:** A02
- **Location:** `src/main.rs` (actix-web default)
- **Why quick win:** Override default `Server` header in actix config.
- **Remediation:** Remove/override `Server` header.

### [ ] I-2: Unused `actix-session` Dependency
- **Severity:** Informational | **Category:** A03
- **Location:** `Cargo.toml`
- **Why quick win:** Remove from `Cargo.toml`. Reduces attack surface.

---

## Phase 2 — Auth & Authorization Core (~4 hours)

Closes all remaining Critical findings and systemic authorization gaps. These are the ship-blocking issues.

### [ ] C-2: Password Reset Never Persists the New Password
- **Severity:** Critical | **Category:** A06
- **Location:** `src/handlers/reset_trigger.rs:93-121`
- **Description:** `let _new_hash = hash_password(&body.password)?` is computed but never written.
- **Remediation:** Add `UPDATE users SET password_digest = $1 WHERE id = $2`, then invalidate the used reset token.

### [ ] C-3: Sign-up Token Not Validated Against Database
- **Severity:** Critical | **Category:** A06
- **Location:** `src/handlers/users.rs:85-90`
- **Description:** Only checks `token.is_empty()`. Never queries `sign_up_triggers`.
- **Remediation:** Query `sign_up_triggers WHERE token = $1 AND expires_at > NOW()`, verify email match, consume token.

### [ ] C-4: Webhook Endpoint Allows Arbitrary Campaign Manipulation
- **Severity:** Critical | **Category:** A01
- **Location:** `src/handlers/webhook.rs:48-116`
- **Description:** Zero auth on `POST /v1/webhook/update_progress`. Modifies `vinyl_sold_count` and `progress`.
- **Remediation:** Add HMAC-SHA256 signature verification (`X-Shopify-Hmac-Sha256` header) and/or IP allowlisting.

### [ ] C-5: User IDOR — Any Authenticated User Can Modify/Delete Any User
- **Severity:** Critical | **Category:** A01
- **Location:** `src/handlers/users.rs:55-113`
- **Description:** `show`, `update`, `delete` accept `CurrentUser` but never check ownership.
- **Remediation:**
  ```rust
  if target_id != user.id && !user.is_admin() {
      return Err(AppError::Forbidden("Not Authorized".to_string()));
  }
  ```

### [ ] C-6: Tenant Configs — Any User Can Modify/Delete Any Config
- **Severity:** Critical | **Category:** A01
- **Location:** `src/handlers/configs.rs:101-230`
- **Description:** `_user: CurrentUser` (underscore = ignored) on `create`, `update`, `destroy`.
- **Remediation:** Remove underscore prefix, enforce `user.is_admin()` check.

### [ ] C-7: Data API Exposes User PII Without Authentication
- **Severity:** Critical | **Category:** A01
- **Location:** `src/handlers/data_api.rs:40-114`
- **Description:** Three endpoints expose user data with zero auth: `last_logins`, `last_login_by_id`, `event_attendees_present`.
- **Remediation:** Add `CurrentUser` guard + `user.is_admin()` check to all three handlers.

### [ ] C-8: Sign-Up Trigger Tokens Exposed Without Authentication
- **Severity:** Critical | **Category:** A01
- **Location:** `src/handlers/sign_up_trigger.rs:64-84`
- **Description:** `show` reveals trigger details (email, role, token) for any token without auth.
- **Remediation:** Require admin authentication; never expose raw tokens in responses.

### [ ] H-5: Systemic Missing Ownership Checks Across CRUD Handlers
- **Severity:** High | **Category:** A01
- **Location:** `src/handlers/producers.rs`, `albums.rs`, `events.rs`, `merchandise.rs`, `campaigns.rs`, `artists.rs`
- **Description:** Multiple handlers accept `_user: CurrentUser` (ignored) on `update`/`destroy`.
- **Remediation:** Enforce `user.is_admin()` or ownership check on every write operation. Consider a macro or helper to reduce boilerplate.

### [ ] H-6: Artist Update — Any Artist+ Can Modify Any Artist Record
- **Severity:** High | **Category:** A01
- **Location:** `src/handlers/artists.rs:121-169`
- **Description:** Checks `is_artist_or_above(&user.role)` but doesn't verify ownership.
- **Remediation:** Fetch the artist record first, verify `artist.user_id == user.id`.

### [ ] H-9: Unauthenticated User Registration
- **Severity:** High | **Category:** A01
- **Location:** `src/handlers/users.rs:79-113`
- **Description:** `POST /v1/users` has no `CurrentUser` guard and doesn't validate sign-up tokens.
- **Remediation:** Covered by C-3 remediation (token validation against DB).

---

## Phase 3 — Crypto & Token Lifecycle (~3 hours)

Hardens random number generation, bcrypt cost, and token expiration semantics.

### [ ] H-1: Non-Cryptographic PRNG for Security Tokens
- **Severity:** High | **Category:** A04
- **Location:** `src/services/auth_service.rs:37-44`, `src/models/reset_trigger.rs:43-50`, `src/models/sign_up_trigger.rs:46-53`, `src/models/mail_subscriber.rs:56-63`
- **Description:** All security tokens use `rand::thread_rng()` with `gen_range(b'a'..=b'z')` — only 26 chars per position.
- **Remediation:**
  ```rust
  use getrandom::getrandom;
  pub fn generate_token() -> String {
      let mut buf = [0u8; 32];
      getrandom(&mut buf).expect("Failed to generate random bytes");
      hex::encode(buf)
  }
  ```

### [ ] H-10: Reset Token Never Checked for Expiration or Single-Use
- **Severity:** High | **Category:** A06
- **Location:** `src/handlers/reset_trigger.rs:93-121`
- **Description:** `expires_at` never validated; token not invalidated after use.
- **Remediation:** Check `expires_at > NOW()` and delete token after successful password change.

### [ ] M-1: bcrypt Cost Factor Below OWASP Recommendation
- **Severity:** Medium | **Category:** A04
- **Location:** `src/services/auth_service.rs:28`
- **Description:** Uses `bcrypt::DEFAULT_COST` (10), below OWASP minimum of 12.
- **Remediation:** `const BCRYPT_COST: u32 = 12;`

### [ ] M-7: `is_expired()` Never Checks Actual Time
- **Severity:** Medium | **Category:** A06
- **Location:** `src/models/sign_up_trigger.rs`
- **Description:** Only checks `self.expires_at.is_none()`, never compares against current time.
- **Remediation:** Compare `expires_at` against `chrono::Utc::now()`.

---

## Phase 4 — Email Subsystem (~4 hours)

Addresses all findings in the email queue, QR code, and SMTP transport code.

### [x] H-12: HTML Injection via `text_copy` (Stored XSS in Email)
- **Severity:** High | **Category:** A03
- **Location:** `src/jobs/email.rs:151`, `src/templates/text_copy_email.rs:11`
- **Status:** ✅ Fixed — `text_copy` sanitized with `ammonia::clean()` before template rendering.

### [ ] H-13: Insecure SMTP Transport — `builder_dangerous` Without TLS Enforcement
- **Severity:** High | **Category:** A02
- **Location:** `src/services/email_service.rs:53`
- **Description:** `builder_dangerous()` skips hostname verification and certificate validation.
- **Remediation:**
  ```rust
  let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(host)
      .unwrap()
      .port(port)
      .credentials(creds)
      .build();
  ```

### [x] M-10: HTML Injection via Database-Sourced Fields
- **Severity:** Medium | **Category:** A03
- **Location:** `src/jobs/email.rs:79-89`, `src/templates/qr_code_email.rs`
- **Status:** ✅ Fixed — `escape_html()` applied to `event_name`, `full_name`, `event_description` before template interpolation.

### [x] M-11: Email Header Injection via Subject Line
- **Severity:** Medium | **Category:** A03
- **Location:** `src/jobs/email.rs:64-66`, `src/services/email_service.rs:94`
- **Status:** ✅ Fixed — `strip_newlines()` applied to `event_name` in all subject line constructions.

### [ ] M-12: Unbounded Email Blast via Job Queue
- **Severity:** Medium | **Category:** A04
- **Location:** `src/handlers/event_attendees.rs:220-228`
- **Description:** One email job per attendee with no batch size cap.
- **Remediation:**
  ```rust
  const MAX_EMAIL_BATCH: usize = 500;
  for attendee in &attendees[..MAX_EMAIL_BATCH.min(attendees.len())] {
      // enqueue job
  }
  ```

### [ ] M-13: Unsubscribe URL Uses Email Address Instead of Token
- **Severity:** Medium | **Category:** A01
- **Location:** `src/jobs/email.rs:206-209`
- **Description:** Unsubscribe URL embeds raw email address instead of a signed token.
- **Remediation:** Use HMAC-SHA256 or short-lived JWT as unsubscribe identifier.

### [x] L-6: Insecure URL Encoding for Non-ASCII Characters
- **Severity:** Low | **Category:** A04
- **Location:** `src/jobs/email.rs:212-219`
- **Status:** ✅ Fixed — `urlencoding` now uses `percent-encoding` crate with RFC 3986 encoding.

---

## Phase 5 — Hardening & Operational (~6 hours)

Broader security posture improvements, operational concerns, and backlog items.

### [ ] H-4: No Rate Limiting on Authentication
- **Severity:** High | **Category:** A07
- **Location:** `src/handlers/auth.rs:50-87`
- **Description:** Login endpoint has no rate limiting, account lockout, or CAPTCHA.
- **Remediation:** Add `actix-rate-limit` middleware or Redis-backed rate limiter.

### [ ] H-8: No Security Headers Configured
- **Severity:** High | **Category:** A02
- **Location:** `src/main.rs`
- **Description:** Zero security headers: no CORS, CSP, HSTS, X-Frame-Options, X-Content-Type-Options.
- **Remediation:** Add `actix-cors` and security headers middleware.

### [ ] H-11: No TLS Enforcement
- **Severity:** High | **Category:** A02
- **Location:** `src/main.rs`
- **Description:** Server binds to plain TCP with no TLS.
- **Remediation:** Terminate TLS at reverse proxy (nginx/Caddy). Infra change, not code.

### [ ] M-2: Weak Password Policy
- **Severity:** Medium | **Category:** A07
- **Location:** `src/models/user.rs:66-68`
- **Description:** Only enforces 6 characters minimum, no complexity requirements.
- **Remediation:** Enforce 8+ chars, require 2 of: uppercase, lowercase, digit, special char.

### [ ] M-3: Unsubscribe Token Generated But Never Stored
- **Severity:** Medium | **Category:** A06
- **Location:** `src/handlers/mailing.rs:270-295`
- **Description:** Token generated but never saved to database.
- **Remediation:** `UPDATE mail_subscribers SET unsubscribe_token = $1 WHERE email = $2`.

### [ ] M-4: Email Enumeration via Unsubscribe Endpoint
- **Severity:** Medium | **Category:** A01
- **Location:** `src/handlers/mailing.rs:270-295`
- **Description:** Returns distinct messages for existing vs non-existing emails.
- **Remediation:** Always return "If your email is registered, an unsubscribe link has been sent."

### [ ] M-5: User Index Returns All Users Without Pagination
- **Severity:** Medium | **Category:** A02
- **Location:** `src/handlers/users.rs:37-53`
- **Description:** `fetch_all()` with no LIMIT clause.
- **Remediation:** Add offset/limit pagination.

### [ ] M-6: Inconsistent Email Validation
- **Severity:** Medium | **Category:** A06
- **Location:** `src/models/user.rs:55-64` vs `src/models/mail_subscriber.rs:52-54`
- **Description:** User checks `@` + `.` in domain; mail subscriber only checks `@`.
- **Remediation:** Use one consistent validation function or `email_address` crate.

### [ ] M-8: No Security Event Logging
- **Severity:** Medium | **Category:** A09
- **Location:** Entire codebase
- **Description:** No logging of failed logins, admin actions, token generation, or permission changes.
- **Remediation:** Add `tracing::warn!` for auth failures, `tracing::info!` for admin actions.

### [ ] L-1: No Request Body Size Limits
- **Severity:** Low | **Category:** A05
- **Remediation:** Configure `actix-web` extract limits.

### [ ] L-2: Debug Logging Configurable via `RUST_LOG`
- **Severity:** Low | **Category:** A02
- **Remediation:** Document that `RUST_LOG=debug` must never be used in production.

### [ ] L-3: Hardcoded Credentials in `.env` File
- **Severity:** Low | **Category:** A04
- **Remediation:** Ensure `.env` is never committed; use secret manager in production.

### [ ] L-4: Hardcoded DB URLs in Test Code
- **Severity:** Low | **Category:** A02
- **Remediation:** Use environment variables or `.env.test` for test DB connections.

### [ ] L-7: SMTP Credentials in Memory Without Rotation Support
- **Severity:** Low | **Category:** A07
- **Location:** `src/services/email_service.rs:35-36, 52`
- **Remediation:** Document limitation. Consider `/v1/reload-config` endpoint for rotation without restart.

### [ ] L-5: No CSRF Protection
- **Severity:** Low | **Category:** A05
- **Status:** Acceptable — pure JWT auth with no cookies mitigates CSRF risk.

### [ ] I-1: Dependencies Generally Up-to-Date
- **Severity:** Informational
- **Status:** No action needed. Review periodically.

### [ ] I-3: `playlists.rs` and `permissions.rs` — Correct Ownership Patterns
- **Severity:** Informational
- **Status:** Use as reference implementation for Phase 2 ownership checks.

### [ ] I-4: `CurrentUser` Extractor Correctly Implemented
- **Severity:** Informational
- **Status:** Auth infrastructure is sound. The problem is missing guards, not the extractor itself.

### [ ] I-5: `.env` Properly in `.gitignore`
- **Severity:** Informational
- **Status:** Good secret hygiene. No action needed.

---

## Security Strengths (No Action Required)

1. **Parameterized SQL exclusively** — zero SQL injection risk in production code
2. **bcrypt password hashing** — passwords never stored in plaintext
3. **UserResponse sanitization** — `password_digest` and `session_token` excluded from API responses
4. **Uniform login error messages** — prevents email enumeration
5. **Role-based access control** — admin checks exist on some endpoints
6. **Single error type** — `AppError` provides structured error handling
7. **Optional email client** — fail-open design prevents server startup failure when SMTP is misconfigured

---

## Progress Tracker

| Phase | Status | Findings | Estimate |
|-------|--------|----------|----------|
| Phase 1 — Quick Wins | ⬜ Not Started | 6 | ~1 hour |
| Phase 2 — Auth & Authorization Core | ⬜ Not Started | 10 | ~4 hours |
| Phase 3 — Crypto & Token Lifecycle | ⬜ Not Started | 4 | ~3 hours |
| Phase 4 — Email Subsystem | ⬜ Not Started | 7 | ~4 hours |
| Phase 5 — Hardening & Operational | ⬜ Not Started | 18 | ~6 hours |
| **Total** | | **46** | **~18 hours** |
