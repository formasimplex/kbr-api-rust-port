# Security Audit Report — kbr-api-rust

**Date:** May 13, 2026
**Scope:** `/Users/ws/formasimplex/kbr-api-rust/src`
**Stack:** Rust 1.95 / actix-web 4.x / sqlx 0.8.x / jsonwebtoken 9.x / bcrypt 0.17.x
**Methodology:** OWASP Top 10:2025

---

## Executive Summary

The codebase has **systemic security deficiencies** that must be addressed before production deployment. The most severe issues are a hardcoded JWT secret fallback enabling complete authentication bypass, a password reset flow that never persists the new password, sign-up tokens that aren't validated against the database, and widespread missing authorization checks allowing any authenticated user to modify any resource.

| Severity | Count | OWASP Category |
|----------|-------|----------------|
| **Critical** | 8 | A01, A04, A06 |
| **High** | 11 | A01, A04, A05, A06, A07, A10 |
| **Medium** | 9 | A02, A04, A06, A07, A09 |
| **Low** | 5 | A02, A03, A05 |
| **Informational** | 5 | A03, A09 |
| **Total** | **38** | |

---

## CRITICAL (8)

### C-1: Hardcoded JWT Secret Fallback — Authentication Bypass
- **Severity:** Critical
- **Category:** A04 Cryptographic Failures
- **Location:** `src/auth/middleware.rs:8-10`
- **Description:** `get_jwt_secret()` falls back to `"default-dev-secret"` when `JWT_SECRET` env var is unset. If deployed without the env var, anyone can forge valid JWTs.
- **Impact:** Complete authentication bypass — attacker can impersonate any user including admins.
- **Remediation:**
  ```rust
  pub fn get_jwt_secret() -> String {
      std::env::var(JWT_SECRET_ENV).unwrap_or_else(|_| {
          tracing::error!("FATAL: JWT_SECRET not set. Refusing to start.");
          std::process::exit(1);
      })
  }
  ```

### C-2: Password Reset Never Persists the New Password
- **Severity:** Critical
- **Category:** A06 Insecure Design
- **Location:** `src/handlers/reset_trigger.rs:93-121`
- **Description:** The `update` handler computes `let _new_hash = hash_password(&body.password)?` but **never writes it to the database**. The underscore prefix indicates the value is intentionally discarded.
- **Impact:** Password reset is completely non-functional. Users receive a success message but their password never changes.
- **Remediation:** Add `UPDATE users SET password_digest = $1 WHERE id = $2`, then invalidate the used reset token.

### C-3: Sign-up Token Not Validated Against Database
- **Severity:** Critical
- **Category:** A06 Insecure Design
- **Location:** `src/handlers/users.rs:85-90`
- **Description:** User registration only checks if `token.is_empty()`. It never queries `sign_up_triggers` to verify the token exists, matches the email, or hasn't expired.
- **Impact:** Anyone can register with any arbitrary string as a token. The invitation system is completely bypassed.
- **Remediation:** Query `sign_up_triggers WHERE token = $1 AND expires_at > NOW()`, verify email match, consume the token after use.

### C-4: Webhook Endpoint Allows Arbitrary Campaign Manipulation
- **Severity:** Critical
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/webhook.rs:48-116`
- **Description:** `POST /v1/webhook/update_progress` accepts requests with zero authentication and directly modifies `vinyl_sold_count` and `progress` in the campaigns table.
- **Impact:** Anyone on the internet can falsify sales data and corrupt business metrics.
- **Remediation:** Add HMAC-SHA256 signature verification (`X-Shopify-Hmac-Sha256` header) and/or IP allowlisting.

### C-5: User IDOR — Any Authenticated User Can Modify/Delete Any User
- **Severity:** Critical
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/users.rs:55-113`
- **Description:** `show`, `update`, and `delete` accept `CurrentUser` but never check if `target_id == user.id` or if the user is admin. Any authenticated user can read, modify, or delete any user record.
- **Impact:** Full horizontal privilege escalation — any user can escalate their own role to admin, change others' emails, or delete accounts.
- **Remediation:**
  ```rust
  if target_id != user.id && !user.is_admin() {
      return Err(AppError::Forbidden("Not Authorized".to_string()));
  }
  ```

### C-6: Tenant Configs — Any User Can Modify/Delete Any Config
- **Severity:** Critical
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/configs.rs:101-230`
- **Description:** `create`, `update`, `destroy` accept `_user: CurrentUser` (underscore = intentionally ignored). Any authenticated user can modify tenant branding, contact emails, and social media links.
- **Impact:** Defacement, phishing via contact email takeover, denial of service via soft-delete.
- **Remediation:** Remove underscore prefix, enforce `user.is_admin()` check.

### C-7: Data API Exposes User PII Without Authentication
- **Severity:** Critical
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/data_api.rs:40-114`
- **Description:** Three endpoints expose user data with zero authentication: `last_logins` (usernames), `last_login_by_id` (email addresses), `event_attendees_present` (full names + emails).
- **Impact:** Complete user enumeration and PII harvesting.
- **Remediation:** Add `CurrentUser` guard + `user.is_admin()` check to all three handlers.

### C-8: Sign-Up Trigger Tokens Exposed Without Authentication
- **Severity:** Critical
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/sign_up_trigger.rs:64-84`
- **Description:** `show` endpoint reveals trigger details (email, role, token) for any token without authentication.
- **Impact:** Attacker can enumerate valid invitations, discover intended user roles, and hijack pending registrations.
- **Remediation:** Require admin authentication; never expose raw tokens in responses.

---

## HIGH (11)

### H-1: Non-Cryptographic PRNG for Security Tokens
- **Severity:** High
- **Category:** A04 Cryptographic Failures
- **Location:** `src/services/auth_service.rs:37-44`, `src/models/reset_trigger.rs:43-50`, `src/models/sign_up_trigger.rs:46-53`, `src/models/mail_subscriber.rs:56-63`
- **Description:** All security tokens use `rand::thread_rng()` with `gen_range(b'a'..=b'z')` — only 26 possible characters per position.
- **Impact:** Reduced entropy makes tokens more susceptible to brute-force. `thread_rng()` is not guaranteed CSPRNG-quality.
- **Remediation:**
  ```rust
  use getrandom::getrandom;
  pub fn generate_token() -> String {
      let mut buf = [0u8; 32];
      getrandom(&mut buf).expect("Failed to generate random bytes");
      hex::encode(buf)  // 64-char hex, 256 bits of entropy
  }
  ```

### H-2: JWT Algorithm Not Explicitly Restricted
- **Severity:** High
- **Category:** A04 Cryptographic Failures
- **Location:** `src/auth/jwt.rs:41`
- **Description:** `Validation::default()` is used without explicitly setting `validation.algorithms`. Relies on library defaults which could change.
- **Impact:** Potential algorithm confusion attacks if library defaults change.
- **Remediation:** `validation.algorithms = vec![Algorithm::HS256];`

### H-3: Database Error Details Leaked to Clients
- **Severity:** High
- **Category:** A10 Mishandling of Exceptional Conditions
- **Location:** `src/error.rs:43-66`
- **Description:** `AppError::Database(_)` returns `self.to_string()` which includes raw sqlx error messages containing table names, column names, and constraints.
- **Impact:** Schema reconnaissance enabling further attacks.
- **Remediation:** Sanitize: `Self::Database(_) => "A database error occurred"`.

### H-4: No Rate Limiting on Authentication
- **Severity:** High
- **Category:** A07 Authentication Failures
- **Location:** `src/handlers/auth.rs:50-87`
- **Description:** Login endpoint has no rate limiting, account lockout, or CAPTCHA.
- **Impact:** Unlimited brute-force and credential stuffing attacks.
- **Remediation:** Add `actix-rate-limit` middleware or Redis-backed rate limiter.

### H-5: Systemic Missing Ownership Checks Across CRUD Handlers
- **Severity:** High
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/producers.rs`, `albums.rs`, `events.rs`, `merchandise.rs`, `campaigns.rs`, `artists.rs`
- **Description:** Multiple handlers accept `_user: CurrentUser` (underscore = ignored) on `update`/`destroy` endpoints. Any authenticated user can modify/delete any record.
- **Impact:** Horizontal privilege escalation across all resource types.
- **Remediation:** Enforce `user.is_admin()` or ownership check on every write operation.

### H-6: Artist Update — Any Artist+ Can Modify Any Artist Record
- **Severity:** High
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/artists.rs:121-169`
- **Description:** Checks `is_artist_or_above(&user.role)` but doesn't verify the user owns the target artist record.
- **Impact:** One artist can modify another artist's profile, bio, and links.
- **Remediation:** Fetch the artist record first, verify `artist.user_id == user.id`.

### H-7: Reset Token Exposed in API Response
- **Severity:** High
- **Category:** A04 Cryptographic Failures
- **Location:** `src/models/reset_trigger.rs:34-41`
- **Description:** `ResetTriggerResponse` includes the `token` field, returned in API responses.
- **Impact:** Tokens leaked through browser history, logs, or API monitoring tools.
- **Remediation:** Remove `token` from response struct; communicate via email only.

### H-8: No Security Headers Configured
- **Severity:** High
- **Category:** A02 Security Misconfiguration
- **Location:** `src/main.rs`
- **Description:** Zero security headers: no CORS, no CSP, no HSTS, no X-Frame-Options, no X-Content-Type-Options.
- **Impact:** Vulnerable to clickjacking, MIME sniffing, and cross-origin attacks.
- **Remediation:** Add `actix-cors` and security headers middleware.

### H-9: Unauthenticated User Registration
- **Severity:** High
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/users.rs:79-113`
- **Description:** `POST /v1/users` has no `CurrentUser` guard and doesn't validate sign-up tokens against DB.
- **Impact:** Uncontrolled account creation, invitation system bypass.
- **Remediation:** See C-3 remediation.

### H-10: Reset Token Never Checked for Expiration or Single-Use
- **Severity:** High
- **Category:** A06 Insecure Design
- **Location:** `src/handlers/reset_trigger.rs:93-121`
- **Description:** `expires_at` is never validated, and token is not invalidated after use.
- **Impact:** Reset tokens usable indefinitely and infinitely reusable.
- **Remediation:** Check `expires_at > NOW()` and delete token after successful password change.

### H-11: No TLS Enforcement
- **Severity:** High
- **Category:** A02 Security Misconfiguration
- **Location:** `src/main.rs`
- **Description:** Server binds to plain TCP with no TLS. JWTs, passwords, and PII transmitted in plaintext.
- **Impact:** MITM attacks, token interception, credential theft.
- **Remediation:** Terminate TLS at reverse proxy (nginx/Caddy) or integrate rustls.

---

## MEDIUM (9)

### M-1: bcrypt Cost Factor Below OWASP Recommendation
- **Severity:** Medium
- **Category:** A04 Cryptographic Failures
- **Location:** `src/services/auth_service.rs:28`
- **Description:** Uses `bcrypt::DEFAULT_COST` (10), below OWASP minimum of 12.
- **Remediation:** `const BCRYPT_COST: u32 = 12;`

### M-2: Weak Password Policy
- **Severity:** Medium
- **Category:** A07 Authentication Failures
- **Location:** `src/models/user.rs:66-68`
- **Description:** Only enforces 6 characters minimum, no complexity requirements.
- **Remediation:** Enforce 8+ chars, require 2 of: uppercase, lowercase, digit, special char.

### M-3: Unsubscribe Token Generated But Never Stored
- **Severity:** Medium
- **Category:** A06 Insecure Design
- **Location:** `src/handlers/mailing.rs:270-295`
- **Description:** `request_unsubscribe` generates a token but never saves it to the database.
- **Impact:** Unsubscribe flow is non-functional.
- **Remediation:** `UPDATE mail_subscribers SET unsubscribe_token = $1 WHERE email = $2`.

### M-4: Email Enumeration via Unsubscribe Endpoint
- **Severity:** Medium
- **Category:** A01 Broken Access Control
- **Location:** `src/handlers/mailing.rs:270-295`
- **Description:** Returns distinct messages for existing vs non-existing emails.
- **Remediation:** Always return "If your email is registered, an unsubscribe link has been sent."

### M-5: User Index Returns All Users Without Pagination
- **Severity:** Medium
- **Category:** A02 Security Misconfiguration
- **Location:** `src/handlers/users.rs:37-53`
- **Description:** `fetch_all()` with no LIMIT clause.
- **Impact:** Memory exhaustion with large user bases.
- **Remediation:** Add offset/limit pagination.

### M-6: Inconsistent Email Validation
- **Severity:** Medium
- **Category:** A06 Insecure Design
- **Location:** `src/models/user.rs:55-64` vs `src/models/mail_subscriber.rs:52-54`
- **Description:** User validation checks for `@` + `.` in domain; mail subscriber only checks `@`.
- **Remediation:** Use one consistent validation function or `email_address` crate.

### M-7: `is_expired()` Never Checks Actual Time
- **Severity:** Medium
- **Category:** A06 Insecure Design
- **Location:** `src/models/sign_up_trigger.rs`
- **Description:** `is_expired()` only checks `self.expires_at.is_none()`, never compares against current time.
- **Remediation:** Compare `expires_at` against `chrono::Utc::now()`.

### M-8: No Security Event Logging
- **Severity:** Medium
- **Category:** A09 Security Logging Failures
- **Location:** Entire codebase
- **Description:** No logging of failed logins, admin actions, token generation, or permission changes.
- **Remediation:** Add `tracing::warn!` for auth failures, `tracing::info!` for admin actions.

### M-9: Server Version Information Leakage
- **Severity:** Medium
- **Category:** A02 Security Misconfiguration
- **Location:** `src/main.rs` (actix-web default)
- **Description:** `Server: actix-web` header reveals framework; health endpoint exposes service name.
- **Remediation:** Remove/override `Server` header.

---

## LOW (5)

| ID | Category | Description |
|----|----------|-------------|
| L-1 | A05 | No request body size limits configured |
| L-2 | A02 | Debug logging configurable via `RUST_LOG` env var |
| L-3 | A04 | Hardcoded credentials in `.env` file |
| L-4 | A02 | Hardcoded DB URLs in test code |
| L-5 | A05 | No CSRF protection (acceptable for pure JWT auth) |

---

## INFORMATIONAL (5)

| ID | Category | Description |
|----|----------|-------------|
| I-1 | A03 | Dependencies generally up-to-date, no known critical CVEs |
| I-2 | A03 | `actix-session` declared but unused — remove to reduce attack surface |
| I-3 | A01 | `playlists.rs` and `permissions.rs` demonstrate correct ownership patterns — use as reference |
| I-4 | A01 | `CurrentUser` extractor is correctly implemented — the auth infrastructure is sound |
| I-5 | A05 | `.env` properly in `.gitignore` — good secret hygiene |

---

## Security Strengths

1. **Parameterized SQL exclusively** — zero SQL injection risk in production code
2. **bcrypt password hashing** — passwords never stored in plaintext
3. **UserResponse sanitization** — `password_digest` and `session_token` excluded from API responses
4. **Uniform login error messages** — prevents email enumeration
5. **Role-based access control** — admin checks exist on some endpoints
6. **Single error type** — `AppError` provides structured error handling

---

## Remediation Priority Matrix

| Priority | Findings | Effort | Impact |
|----------|----------|--------|--------|
| **P0 — Ship Blocker** | C-1, C-2, C-3, C-4 | ~2 hours | Eliminates auth bypass, broken password reset, open registration, unauthenticated webhooks |
| **P1 — This Sprint** | C-5, C-6, C-7, C-8, H-1, H-2, H-3, H-4 | ~4-6 hours | Closes IDOR, PII exposure, weak crypto, brute-force vectors |
| **P2 — Next Sprint** | H-5, H-6, H-7, H-8, H-9, H-10, H-11, M-1–M-4 | ~6-8 hours | Systemic ownership checks, security headers, TLS, token lifecycle |
| **P3 — Backlog** | M-5–M-9, L-1–L-5, I-1–I-5 | ~4 hours | Pagination, logging, password policy, dependency cleanup |
