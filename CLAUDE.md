# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working on this codebase. IMPORTANT: these instructions OVERRIDE any default behavior and you MUST follow them exactly as written.

## Project Overview

Lab Inventory is an open-source Laboratory Inventory Management System for multi-laboratory environments. Each laboratory deploys its own instance (separate server, separate database). Instances communicate through a **federation API** using HMAC-SHA256 signed requests to query remote laboratory assets and inventory. Supports asset/devices/parts tracking, inventory management, cross-laboratory borrowing with approval workflows, maintenance records, and audit logging. Full requirements are in `reference/需求文档.md` (Chinese).

**Tech stack:** Rust + Actix Web backend, React 19 + Ant Design 6 frontend, PostgreSQL + Redis, deployable as a browser SPA or Tauri desktop app.

## Repository Structure

```
backend/              # Rust API server (Actix Web)
  Cargo.toml
  configuration/      # YAML configs (base.yaml, local.yaml, production.yaml)
  migrations/         # SQLx time-stamped migrations
  scripts/            # init_db.sh/.ps1, init_redis.sh/.ps1 (Docker-based)
  src/
    main.rs           # Entry: tokio runtime → config → Application::build → run
    lib.rs            # Module declarations
    startup.rs        # Application struct, PgPool, CORS, Redis sessions, all routes under /api/v1
    configuration.rs  # Settings struct + YAML loading (config crate, APP_ENVIRONMENT)
    telemetry.rs      # tracing subscriber (Bunyan JSON)
    authentication/   # Session auth (Argon2id), role checks (admin/user), middleware
    federation.rs     # HMAC-SHA256 signed federation protocol (verify + sign)
    routes/           # Route modules (CRUD + actions for each entity)
      local_lab/      # Local laboratory CRUD (renamed from laboratories/)
      remote_laboratories/  # Remote lab CRUD (admin management)
      federation_api.rs     # Federation public API + proxy routes
      assets/, inventory_items/, asset_categories/, locations/, ...
    audit.rs          # Audit log recording
    idempotency/      # Idempotency key middleware
  tests/api/          # Integration tests (helpers.rs with TestApp/spawn_app)
frontend/             # React SPA + Tauri desktop
  src/
    main.tsx          # ReactDOM entry → BrowserRouter → AppProviders → App
    app/              # App.tsx (routes), RootRoute.tsx (auth check), AppShell.tsx (sidebar layout), providers.tsx
    features/         # Feature modules: auth/, admin/, dashboard/, settings/, server-settings/
      admin/          # Admin pages: local labs, users, remote labs
    shared/           # api/ (httpClient, backendConfig), test/, ui/ (AppChrome, EntryShell)
  src-tauri/          # Tauri 2 desktop app wrapper
  tests/e2e/          # Playwright E2E tests
```

## Commands

### Prerequisites

- **PostgreSQL** and **Redis** must be running locally (or use `backend/scripts/init_db.ps1` and `init_redis.ps1` for Docker)
- Set `DATABASE_URL` env var for sqlx (see `.env` file): `postgres://postgres:password@127.0.0.1:5432/lab_inventory`
- On Windows, use `.ps1` scripts; on Linux/macOS, use `.sh` scripts

### Backend (run from `backend/` directory)

```bash
# Start the API server (default port 8000)
cargo run

# Run all integration tests
cargo test

# Run a single test module
cargo test --test api auth

# Run a specific test
cargo test --test api auth::login_returns_200

# Database migrations
sqlx database create
sqlx migrate run
```

### Frontend (run from `frontend/` directory)

```bash
# Install dependencies
npm install

# Start dev server (http://127.0.0.1:5173)
npm run dev

# Typecheck
npm run typecheck

# Build for production
npm run build

# Run unit tests (Vitest + JSDOM + MSW)
npm run test

# Run tests in watch mode
npm run test:watch

# Run E2E tests (Playwright, 3 viewport projects)
npm run test:e2e

# Tauri desktop dev
npm run tauri:dev

# Tauri desktop build
npm run tauri:build
```

### Development Tips

- Test-Driven Development (TDD) with comprehensive unit and integration tests. Backend tests use Rust's built-in test framework with SQLx's `cargo sqlx` for database setup. Frontend tests use Vitest with JSDOM for unit tests and Playwright for E2E tests.

## Architecture

### Deployment Model

Each laboratory runs its own server instance with its own database. An instance can manage multiple sub-laboratories (e.g., departments within a university) stored in `local_laboratories` table. To access other laboratories' data, configure `remote_laboratories` entries with their API URL and shared secret. Queries to remote labs are **proxied in real-time** through the local API — no caching of remote data locally.

### Backend Architecture

- **Framework:** Actix Web 4 REST API on `/api/v1`
- **Auth (local users):** Redis-backed session cookies (24h TTL, SameSite Lax, HttpOnly). Passwords hashed with Argon2id. Two roles: `admin` (full access) and `user` (laboratory-scoped). `reject_anonymous_users` middleware wraps all authenticated routes.
- **Auth (federation):** HMAC-SHA256 signed requests between instances. Each `remote_laboratories` row stores `key_id` and `shared_secret`. Signing string: `"{method}\n{path_and_query}\n{timestamp}\n{nonce}\n{body_sha256_hex}"`. Headers: `X-Lab-Id`, `X-Key-Id`, `X-Timestamp`, `X-Nonce`, `X-Signature`. Timestamp tolerance ±900s. Nonce dedup via `federation_nonces` table.
- **Routing:** All routes in `startup.rs` function `api_routes()`:
  - **Public routes:** `/health_check`, `/auth/login`, `/auth/logout`
  - **Federation routes** (no session, HMAC-signed): `/federation/info`, `/federation/laboratories/{id}/assets`, `/federation/laboratories/{id}/inventory-items`
  - **Authenticated routes** (session cookie): `/local-lab/*`, `/remote-laboratories/*`, `/assets/*`, `/inventory-items/*`, etc.
- **DB:** PostgreSQL via SQLx 0.8 (compile-time query checking, `cargo sqlx` for migrations). 15+ tables, UUIDs as PKs, `pg_trgm` for text search.
- **Config:** YAML hierarchy via `config` crate. `APP_ENVIRONMENT` env var selects `local.yaml` or `production.yaml` to overlay `base.yaml`. Env vars with `APP_` prefix and `__` separator override YAML values.
- **Audit:** Every mutation writes to `audit_logs` table. `record_audit()` helper in `audit.rs`.
- **Federation protocol:** `federation.rs` module handles signature verification (`verify_federation_request`) and outgoing request signing (`signed_headers`).

### Federation API Design

The federation routes (`/federation/*`) return **desensitized** data — no `internal_notes`, `serial_number`, `batch_number`, or other sensitive fields. Federation responses are trimmed to public fields only: name, model, status, laboratory info, quantity available, public notes.

Proxy routes (`/remote-laboratories/{id}/assets`, `/remote-laboratories/{id}/inventory-items`) are behind session auth. They call the remote federation API using signed requests and return the response to the local user.

### Frontend Architecture

- **Routing:** React Router DOM 7. `App.tsx` defines all routes: `/` (RootRoute → redirect), `/login`, `/dashboard`, `/admin/*`, `/settings/*`, `/server-settings`.
- **Auth flow:** `RootRoute` checks for auth token, redirects to `/login` if not authenticated. Login stores session cookie via backend.
- **State management:** TanStack React Query for all server state. No Redux or Context for data.
- **API layer:** `shared/api/httpClient.ts` — thin fetch wrapper with `ApiError` class. `shared/api/backendConfig.tsx` — API base URL stored in localStorage, configurable via Server Settings page.
- **UI:** Ant Design 6 with Chinese locale (`zh_CN`). `shared/ui/AppChrome.tsx` — authenticated layout (header, sidebar, content). `shared/ui/EntryShell.tsx` — public pages layout.
- **Validation:** Zod 4 schemas for runtime API response validation.
- **All UI text is in Simplified Chinese.**

### Frontend Testing

- **Unit tests:** Vitest with JSDOM. MSW for API mocking. Test setup in `src/shared/test/setup.ts`. Render helper in `src/shared/test/render.tsx`.
- **E2E tests:** Playwright with 3 viewport projects (desktop, iPad Mini, Pixel 5). Tests in `tests/e2e/`. Routes API calls through `page.route()` for mocking.

### Current Feature Status

- **Complete:** Auth (login/logout/password change), Admin management (local labs, users, remote labs), Dashboard, Backend API (inventory, exports, audit)
- **Partial:** User settings (profile/password working, preferences is placeholder)
- **Not implemented:** Frontend UI for inventory management, borrow requests, maintenance, alerts (sidebar items are disabled)

## Key Conventions

- **Rust edition 2024**, strict TypeScript (`strict: true`, `noUnusedLocals`, `noUnusedParameters`)
- No ESLint/Prettier — TypeScript strictness is the primary linting gate
- Backend uses `thiserror` for error types, `anyhow` for application errors
- All UUIDs use the `uuid` crate with `v4` and `serde` features
- API base URL: `http://127.0.0.1:8000/api/v1` (default)
- Backend config key: `labInventory.apiBaseUrl` in localStorage
- Route modules follow sub-module pattern: `mod.rs` + `create.rs`/`list.rs`/`get.rs`/`update.rs`/`delete.rs`/`helpers.rs`/`model.rs`
- Model types use `Row` → `Response` pattern with `from_row(row, &actor)` for row-level visibility