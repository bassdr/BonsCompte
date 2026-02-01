# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

BonsCompte is a financial coordination tool for shared living situations (roommates, cooperatives, collectives). It tracks shared expenses with a weight-based fairness system and supports recurring payments with future debt projection.

## Tech Stack

- **Backend**: Rust with Axum 0.8, SQLite (sqlx), JWT auth (jsonwebtoken), Argon2 password hashing
- **Frontend**: SvelteKit 2.48, Svelte 5 (runes syntax: `$state`, `$derived`, `$effect`), TypeScript, Vite 7

## Common Commands

**Important:** Always use `cd` or run commands from the correct directory. The working directory may change during a session.

### Backend (run from `/backend`)
```bash
cd /home/david/src/BonsCompte/backend && cargo run      # Start server on localhost:8000
cd /home/david/src/BonsCompte/backend && cargo check    # Type check without building
cd /home/david/src/BonsCompte/backend && cargo build --release  # Production build
cd /home/david/src/BonsCompte/backend && cargo test     # Run all unit tests
```

### Frontend (run from `/frontend`)
```bash
cd /home/david/src/BonsCompte/frontend && npm run dev   # Start dev server on localhost:5173
cd /home/david/src/BonsCompte/frontend && npm run build # Production build
cd /home/david/src/BonsCompte/frontend && npm run check # TypeScript + Svelte type checking
cd /home/david/src/BonsCompte/frontend && npm run format # Auto-format with ESLint + Prettier
cd /home/david/src/BonsCompte/frontend && npm run format:check # Check format
cd /home/david/src/BonsCompte/frontend && npm test      # Run unit tests with Vitest
```

### Root (run from `/`)
```bash
cd /home/david/src/BonsCompte && npm test               # Run all tests (backend + frontend)
cd /home/david/src/BonsCompte && npm run test:backend   # Run backend tests only
cd /home/david/src/BonsCompte && npm run test:frontend  # Run frontend tests only
```

## Database

The SQLite database is located at `backend/data/bonscompte.db`. The database schema is defined and migrated automatically by `backend/src/db.rs` on server startup.

**Important notes:**
- The actual database file is `backend/data/bonscompte.db`, not `backend/data.db`
- Migrations run automatically when the server starts
- The database uses WAL (Write-Ahead Logging) mode for better concurrency
- Foreign keys are enabled by default

## Architecture

### Backend Structure

```
backend/src/
├── main.rs              # Axum router setup, middleware, server start
├── config.rs            # Environment config (DATABASE_URL, JWT_SECRET, HOST, PORT, RATE_LIMIT_ENABLED)
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
├── services/
│   └── debt_calculator.rs  # Balance calculation, recurring payment expansion
└── bin/                 # Additional binary utilities
    ├── admin.rs         # Admin CLI tool (cargo run --bin bonscompte-admin)
    └── recalculate_contributions.rs  # Recalculate contributions with 4-decimal precision
```

### Key Backend Patterns

- **Path extraction**: Axum 0.8 uses `{param}` syntax. Multi-param routes need struct extractors:
  ```rust
  #[derive(Deserialize)]
  struct ParticipantPath { id: i64, participant_id: i64 }
  ```

- **Auth middleware**: `AuthUser` extractor for authenticated routes, `ProjectMember` for project-scoped routes (validates membership + extracts role)

- **Contribution amounts**: Stored with 4-decimal precision to avoid rounding errors. When splitting a payment by weight, each contribution is calculated as `(amount * weight / total_weight * 10000.0).round() / 10000.0`. The frontend formats to 2 decimals for display. To recalculate existing contributions after precision changes, run `cargo run --bin recalculate-contributions`.

- **Migrations**: Inline SQL in `db.rs`, runs `.ok()` on ALTER TABLE to handle re-runs

### Security Middleware (in `main.rs`)

The backend includes several security layers:

- **Rate limiting** (tower_governor) - **disabled by default**:
  - Only enable with `RATE_LIMIT_ENABLED=true` if backend is directly exposed to internet (no reverse proxy)
  - When behind nginx/Caddy/etc, keep disabled - the proxy handles rate limiting with real client IPs
  - Backend sees all proxied requests as coming from one IP (e.g., 127.0.0.1), breaking per-user limits
  - If enabled: Auth routes get 5 req/60s, General API gets 100 req/s

- **Scan path blocking**: Silently returns 404 for common scanner probes:
  - Prefixes: `/.git`, `/.env`, `/wp-admin`, `/phpmyadmin`, `/cgi-bin`, etc.
  - Extensions: `.php`, `.asp`, `.sql`, `.bak`, etc.
  - Runs before logging to reduce noise

- **Error sanitization** (`error.rs`): Internal errors (database, JWT) are logged server-side but return generic messages to clients

### Frontend Structure

```
frontend/src/
├── lib/
│   ├── api.ts              # All API calls + TypeScript interfaces
│   ├── auth.ts             # Token storage, login state, User type
│   ├── i18n/translations/  # English and French translations
│   └── stores/             # Svelte stores
└── routes/
    ├── +layout.svelte      # Global layout with auth state
    ├── +page.svelte        # Project list (home)
    ├── login/              # Login form
    ├── join/               # Form to join a project
    ├── register/           # Create user form
    └── projects/[id]/
        ├── +layout.svelte       # Project nav tabs
        ├── +page.svelte         # Redirects to overview
        ├── overview/            # Settlements/Ownership summary with date picker + optional range for graph view
        ├── transactions/[mode]  # internal (transfer), outgoing (payments) and incoming (deposit) transactions payments form + list of transactions with filters and edit mode
        └── settings/            # Project settings, including participant and member/role management
```

### Key Frontend Patterns

- **Svelte 5 runes**: Use `$state()`, `$derived()`, `$effect()` - NOT legacy `$:` syntax
- **API calls**: All through `lib/api.ts` using `authFetch()` wrapper that handles token + 401 redirect
- **Routing**: SvelteKit file-based with `[id]` for dynamic segments

### Internationalization (i18n)

The app supports multiple languages (EN/FR) with user preferences for formatting.

**Key Concepts:**
- **UI Language** (i18n): Text translations via `svelte-i18n`, stored in JSON files
- **Formatting Preferences**: Date/currency/number formatting via user settings, NOT tied to language

**File Structure:**
```
frontend/src/lib/
├── i18n/
│   ├── index.ts              # i18n setup, locale management
│   └── translations/
│       ├── en.json           # English translations (flat keys)
│       └── fr.json           # French translations
├── format/
│   ├── date.ts               # formatDate(), formatDateWithWeekday()
│   └── currency.ts           # formatCurrency(), formatNumber()
└── stores/
    └── preferences.ts        # User preferences store
```

**Adding a New Language:**
1. Create `frontend/src/lib/i18n/translations/{lang}.json`
2. Add to `register()` in `frontend/src/lib/i18n/index.ts`
3. Add to `supportedLanguages` array
4. Add to backend validation in `backend/src/routes/users.rs`

**Translation Keys:** Use flat dot-notation (e.g., `auth.login`, `nav.projects`). No logic in translation files.

**Formatting:** Always use helpers from `lib/format/` instead of hardcoded formatting:
- `formatDate(dateStr)` - respects user's date_format preference
- `formatCurrency(amount)` - respects user's currency_symbol, position, decimal_separator

**Language Precedence (highest wins):**
1. Authenticated user preference (from backend)
2. localStorage `bonscompte_language`
3. Browser language
4. Default: 'en'

## Data Model

- **Project**: Has participants, members (users), and payments
- **Participant**: Entity that pays/owes (may or may not be linked to a user); has `account_type` ('user' or 'pool')
- **Member**: User's membership in a project with role (owner/editor/viewer)
- **Payment**: Has payer, amount, contributions, optional recurrence, and optional `receiver_account_id` for transfers
- **Contribution**: Links payment to participant with weight (determines share)

### Participant Types

- **User account** (`account_type = 'user'`): Regular participant who can owe/be owed money
- **Pool account** (`account_type = 'pool'`): Shared account (max one per project) for tracking collective funds
  - Excluded from settlement calculations
  - Has ownership tracking (who contributed/consumed from pool)
  - Has per-pool warning settings:
    - `warning_horizon_account`: When to warn if pool balance drops below expected minimum
    - `warning_horizon_users`: When to warn if individual user's pool share drops below their expected minimum
    - Either can be NULL to disable that warning type

### Dual Ledger System (Pool Expected Minimum)

Pools support a "dual ledger" system that tracks both actual balance AND expected minimum separately. This enables features like:
- Setting contribution rules that define expected deposits without moving money
- Tracking when users are behind on their expected contributions
- Visualizing the gap between actual balance and expected minimum

**Payment Flags:**

Each payment has three boolean flags controlling how it affects the ledgers:

| Flag | Default | Description |
|------|---------|-------------|
| `affects_balance` | `true` | Transaction moves actual money (affects pool balance) |
| `affects_payer_expectation` | `false` | When payer is a pool, reduces pool's expected minimum |
| `affects_receiver_expectation` | `false` | When receiver is a pool, increases pool's expected minimum |

**Transaction Labels (UI):**

Based on these flags, transactions show badges in the UI:

| Label | Condition | Description |
|-------|-----------|-------------|
| **Rule** | `affects_balance === false` | Sets expected minimum without moving money |
| **Approved** | `affects_payer_expectation === true` | Approved expense from pool (reduces expected min) |
| **Earmarked** | `affects_receiver_expectation === true` | Earmarked deposit to pool (increases expected min) |
| **Draft** | `is_final === false` | Not yet finalized (can show with other labels) |

**Common Scenarios:**

1. **Monthly contribution rule** (affects_balance=false, affects_receiver_expectation=true):
   - Creates recurring expected deposits without moving money
   - Shows as "Rule" badge
   - Increases pool's expected minimum each month

2. **Earmarked deposit** (affects_balance=true, affects_receiver_expectation=true):
   - User deposits money AND it counts toward their expected contribution
   - Shows as "Earmarked" badge
   - Increases both balance and expected minimum

3. **Approved pool expense** (affects_balance=true, affects_payer_expectation=true):
   - Pool pays expense AND reduces expected minimum proportionally
   - Shows as "Approved" badge
   - Decreases both balance and expected minimum

4. **Regular pool expense** (affects_balance=true, affects_payer_expectation=false):
   - Pool pays but expected minimum unchanged (e.g., unexpected expense)
   - No special badge
   - Only decreases balance

**Overview Page Features:**

In horizon mode, the overview page shows:
- **Expected minimum stats**: Current, Projected, Max, and Min (with date) expected minimum
- **Chart visualization**: Red dashed line for expected minimum, with red shaded area filling between expected minimum and balance when balance drops below
- **Warnings**: Triggered when balance goes below expected minimum (not just negative)

### Payment Types

Determined by `payer_id` and `receiver_account_id`:

- **External expense** (`payer_id` = user, `receiver_account_id = NULL`): Normal expense, money leaves the system
  - Affects settlements: payer's "paid" increases, contributors' "owed" increases

- **User → User transfer** (`receiver_account_id` = another user): Direct payment between users
  - Affects settlements: payer's "paid" increases, receiver's "owed" increases
  - Used for "Pay back" functionality to settle debts

- **User → Pool transfer** (`receiver_account_id` = pool): Deposit to shared account
  - Only affects pool ownership (payer's ownership increases)
  - Does NOT affect user-to-user settlements

- **Pool → User transfer** (payer = pool): Withdrawal from shared account
  - Only affects pool ownership (receiver's ownership decreases)
  - Does NOT affect user-to-user settlements

- **External funds to User** (`payer_id = NULL`, `receiver_account_id` = user): Income/refund received by a user
  - Example: Bank refund of $1000 to User A, split among A, B, C
  - Receiver is debited full amount (holds money for others)
  - Contributors are credited their share (owed their portion)
  - Result: Receiver owes other contributors their share
  - Test case: `test_external_inflow_bank_refund_split_three_ways`

- **External funds to Pool** (`payer_id = NULL`, `receiver_account_id` = pool): Income/grant to shared account
  - Example: Government grant or interest deposited to pool
  - Contributors' pool ownership increases by their share
  - Does NOT affect user-to-user settlements

### Weight System

- `weight > 0`: Normal participation (proportional share)
- `weight = 0`: Excluded from this payment
- Default weight on participant applies to new payments

### Recurring Payments

- Types: daily, weekly, monthly, yearly
- Supports "every X periods"
- Supports multiple times per period for days and months
- Debt calculator expands occurrences up to target date for projection

### Settlement Calculation

Located in `backend/src/services/debt_calculator.rs`:

1. **Balance calculation**: For each participant, compute `net_balance = total_paid - total_owed`
2. **Pool exclusion**: Pool accounts are excluded from settlement generation
3. **Greedy matching**: Debtors (negative balance) are matched with creditors (positive balance)
4. **Direct mode**: Optional mode that only settles between participants who directly transacted

#### Critical Invariants (FRAGILE LOGIC)

The settlement calculation has subtle interactions between pool accounts and user accounts. **Any changes to `debt_calculator.rs` must be validated carefully.**

**Key rules that must be preserved:**

1. **Pool transfers are excluded from user settlements**: When `receiver_account_id` points to a pool OR when `payer_id` is a pool, the occurrence is skipped in `paid_map`/`owed_map` calculations. Pool transfers only affect pool ownership tracking.

2. **Pool-paid external expenses do NOT add to user `owed_map`**: When pool pays an external expense (e.g., pool pays $300 insurance split among users), the contributors' shares are NOT added to `owed_map`. This debt is tracked separately in pool ownership. Adding these to `owed_map` would create an imbalance since pool is excluded from settlements.

3. **User-to-user transfers affect both parties**: Payer's "paid" increases, receiver's "owed" increases. This is how "Pay back" functionality settles debts.

4. **External funds to user**: When `payer_id` is NULL and `receiver_account_id` is a user, the receiver holds money for the group. The receiver is DEBITED (owed_map += amount) and contributors are CREDITED (paid_map += share). This ensures the receiver owes others their share.

5. **External funds to pool**: When `payer_id` is NULL and `receiver_account_id` is a pool, contributors' pool ownership increases by their share. Does not affect user-to-user settlements.

6. **Pairwise tracking excludes pool relationships**: `pairwise_map` only tracks user-to-user relationships for the direct settlements view.

**Before modifying this file:**
- Run `cargo test` in the backend directory and ensure all tests pass
- If changing behavior, update the tests FIRST to reflect expected new behavior
- Test manually with scenarios involving: pool + users, transfers, and external expenses

## Environment Variables

Backend (`.env`):
```
DATABASE_URL=data/bonscompte.db
JWT_SECRET=your-secret-key
HOST=0.0.0.0
PORT=8000
RATE_LIMIT_ENABLED=false  # Default. Only set to true if NOT behind a reverse proxy
MAX_PROJECTS_PER_USER=3  # Optional: Limit projects per user (empty/0 = unlimited)
```

Frontend (`.env` or inline):
```
VITE_API_BASE=http://localhost:8000  # Development (optional, auto-detected)
VITE_API_BASE=/api                   # Production with NGINX proxy (optional, auto-detected)
```

**Note:** The frontend automatically detects production (HTTPS) and uses `/api` as the base URL. You only need to set `VITE_API_BASE` for custom deployment configurations.

## Development Guidelines

### Testing

The project has comprehensive unit tests for both backend and frontend.

#### Backend Tests

Located in `backend/tests/`, using Rust's built-in test framework with `axum-test` for HTTP testing.

**Authentication tests** (`backend/tests/auth_tests.rs`):
- Register and login success flow
- Invalid credentials handling
- Wrong password handling
- Duplicate username validation
- Password strength requirements
- User preferences returned in login response

Run tests:
```bash
cd backend
cargo test                   # Run all tests
cargo test auth_tests        # Run auth tests only
cargo test -- --nocapture    # Run with output
```

#### Frontend Tests

Located in `frontend/src/tests/`, using Vitest with jsdom environment.

**API tests** (`frontend/src/tests/api.test.ts`):
- Login with valid credentials
- Login with invalid credentials
- Network error handling
- Malformed JSON responses
- User registration
- Username exists error handling
- Password validation errors

**Production NGINX proxy tests** (`frontend/src/tests/api-config.test.ts`):
- API base URL selection logic (HTTPS → `/api`, HTTP → `localhost:8000`)
- NGINX reverse proxy path mapping validation
- Production deployment checklist
- API endpoint path validation
- Backend CORS requirements documentation

Run tests:
```bash
cd frontend
npm test                     # Run all tests
npm run test:watch           # Watch mode for development
npm run test:ui              # Interactive UI
```

#### Running All Tests

From the project root:
```bash
npm test                     # Runs both backend and frontend tests
```

### Pre-commit Hooks

The project uses Husky to automatically run tests before each commit. The pre-commit hook (`.husky/pre-commit`) will:

1. Auto-fix format (currently only frontend, `npm run format`)
2. Run all backend tests (`cargo test`)
3. Run all frontend tests (`npm test`)
4. Block the commit if any tests fail

This ensures that broken code is never committed to the repository.

To bypass the hook in exceptional cases (not recommended):
```bash
git commit --no-verify -m "message"
```

### Continuous Integration

GitHub Actions automatically runs tests on every push and pull request to `main` and `develop` branches.

The workflow (`.github/workflows/test.yml`) includes:

**Backend CI**:
- Rust toolchain setup
- Cargo dependency caching
- Unit tests (`cargo test`)
- Code formatting check (`cargo fmt --check`)
- Linting with Clippy (`cargo clippy`)

**Frontend CI**:
- Node.js 20 setup
- npm dependency caching
- Unit tests (`npm test`)
- Type checking (`npm run check`)
- Linting (`npm run lint`)

### Branch Protection (Required Status Checks)

To prevent merging untested code, configure GitHub branch protection rules:

1. Go to repository **Settings** → **Branches**
2. Add protection rule for `main` (and optionally `develop`)
3. Enable "Require status checks to pass before merging"
4. Select required checks:
   - `Backend Tests`
   - `Frontend Tests`

See [`docs/GITHUB_BRANCH_PROTECTION.md`](docs/GITHUB_BRANCH_PROTECTION.md) for detailed setup instructions.

Once configured, pull requests **cannot be merged** until all tests pass.

### Before Committing Changes

**Automated checks** (via pre-commit hook):
- Backend unit tests will run automatically
- Frontend unit tests will run automatically
- Commit will be blocked if tests fail

**Manual checks for critical changes**:
1. **Financial calculations**: Run `cargo test` and verify debt_calculator tests pass
2. **Type safety**: Run `npm run check` for TypeScript validation
3. **Test edge cases manually** when modifying:
   - Settlement calculations (pool + user combinations)
   - Recurring payment expansion
   - Transfer functionality (user-to-user, user-to-pool, pool-to-user)

### Critical Files

Files that require extra care when modifying:

- `backend/src/services/debt_calculator.rs` - Complex financial logic with subtle pool/user interactions. Has comprehensive unit tests that MUST pass.
