# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

BonsCompte is a financial coordination tool for shared living situations (roommates, cooperatives, collectives). It tracks shared expenses with a weight-based fairness system and supports recurring payments with future debt projection.

## Tech Stack

- **Backend**: Rust with Axum 0.8, SQLite (sqlx), JWT auth (jsonwebtoken), Argon2 password hashing
- **Frontend**: SvelteKit 2.48, Svelte 5 (runes syntax: `$state`, `$derived`, `$effect`), TypeScript, Vite 7

## Common Commands

### Backend (from `/backend`)
```bash
cargo run                    # Start server on localhost:8000
cargo check                  # Type check without building
cargo build --release        # Production build
```

### Frontend (from `/frontend`)
```bash
npm run dev                  # Start dev server on localhost:5173
npm run build                # Production build
npm run check                # TypeScript + Svelte type checking
npm run lint                 # ESLint + Prettier check
npm run format               # Auto-format with Prettier
```

## Architecture

### Backend Structure

```
backend/src/
├── main.rs              # Axum router setup, middleware, server start
├── config.rs            # Environment config (DATABASE_URL, JWT_SECRET, HOST, PORT)
├── db.rs                # SQLite pool init + migrations (run at startup)
├── error.rs             # AppError enum with Into<Response>
├── auth/
│   ├── handlers.rs      # Login/register endpoints
│   ├── jwt.rs           # Token create/validate
│   ├── middleware.rs    # AuthUser + ProjectMember extractors
│   └── password.rs      # Argon2 hash/verify
├── models/              # SQLx FromRow structs + Create* input structs
├── routes/              # Axum handlers grouped by resource
│   ├── auth.rs          # POST /auth/login, /auth/register
│   ├── projects.rs      # CRUD + invite system
│   ├── participants.rs  # Project participants (can exist without user)
│   ├── members.rs       # User-project membership with roles
│   ├── payments.rs      # Payments with contributions + recurrence
│   └── debts.rs         # GET with ?date= for future projection
└── services/
    └── debt_calculator.rs  # Balance calculation, recurring payment expansion
```

### Key Backend Patterns

- **Path extraction**: Axum 0.8 uses `{param}` syntax. Multi-param routes need struct extractors:
  ```rust
  #[derive(Deserialize)]
  struct ParticipantPath { id: i64, participant_id: i64 }
  ```

- **Auth middleware**: `AuthUser` extractor for authenticated routes, `ProjectMember` for project-scoped routes (validates membership + extracts role)

- **Migrations**: Inline SQL in `db.rs`, runs `.ok()` on ALTER TABLE to handle re-runs

### Frontend Structure

```
frontend/src/
├── lib/
│   ├── api.ts           # All API calls + TypeScript interfaces
│   ├── auth.ts          # Token storage, login state, User type
│   └── stores/          # Svelte stores
└── routes/
    ├── +layout.svelte   # Global layout with auth state
    ├── +page.svelte     # Project list (home)
    ├── login/           # Login form
    ├── register/        # Registration form
    └── projects/[id]/
        ├── +layout.svelte   # Project nav tabs
        ├── +page.svelte     # Payments list + add form
        ├── participants/    # Participant management
        ├── members/         # Member/role management
        ├── debts/           # Debt summary with date picker
        └── settings/        # Project settings
```

### Key Frontend Patterns

- **Svelte 5 runes**: Use `$state()`, `$derived()`, `$effect()` - NOT legacy `$:` syntax
- **API calls**: All through `lib/api.ts` using `authFetch()` wrapper that handles token + 401 redirect
- **Routing**: SvelteKit file-based with `[id]` for dynamic segments

## Data Model

- **Project**: Has participants, members (users), and payments
- **Participant**: Entity that pays/owes (may or may not be linked to a user)
- **Member**: User's membership in a project with role (owner/editor/viewer)
- **Payment**: Has payer, amount, contributions (who owes what), optional recurrence
- **Contribution**: Links payment to participant with weight (determines share)

### Weight System

- `weight > 0`: Normal participation (proportional share)
- `weight = 0`: Excluded from this payment
- Default weight on participant applies to new payments

### Recurring Payments

- Types: daily, weekly, monthly, yearly
- Either "every X periods" OR "X times per period"
- Debt calculator expands occurrences up to target date for projection

## Environment Variables

Backend (`.env`):
```
DATABASE_URL=data.db
JWT_SECRET=your-secret-key
HOST=0.0.0.0
PORT=8000
```

Frontend (`.env` or inline):
```
VITE_API_BASE=http://localhost:8000
```
