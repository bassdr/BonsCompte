# BonsCompte

[![Tests](https://github.com/bassdr/BonsCompte/actions/workflows/test.yml/badge.svg)](https://github.com/bassdr/BonsCompte/actions/workflows/test.yml)

BonsCompte is a lightweight financial coordination tool designed for shared living situations such as housemates, cooperatives, or small collectives.
It helps groups track recurring shared bills, future expenses, and fairness over time through a simple weighting system.

The project includes:
- Rust backend (Axum + SQLite + JWT authentication)
- SvelteKit frontend (Svelte 5 with runes)
- SQLite database with automatic migrations

## Purpose

The goal of BonsCompte is to make it easy for a group of people to fairly manage shared expenses without spreadsheets or manual tracking.

## Common Use Cases

- **Roommates or cooperative households**
  Track who paid what, ensure fair distribution of recurring and one-off expenses, and keep a clear history for everyone.

- **Shared bank accounts with multiple owners**
  The app helps determine which part of the shared balance belongs to each participant over time.
  Even if all operations go through a common account, the system tracks individual contributions and consumption
  so the implicit "ownership" of the account remains balanced and transparent.

- **Groups with irregular participation**
  Some participants join only for certain bills or activities. Per-bill weight overrides make this effortless.

- **Co-parenting or alternating household setups**
  Handle situations where different people are present at different times, with recurring or seasonal expenses.

## Core Features

### 1. User Authentication & Project Management
- Secure user registration and login with JWT tokens
- Password hashing with Argon2
- Human-approved password recovery (no email required) - see [Password Recovery](docs/PASSWORD_RECOVERY.md)
- Multi-project support with role-based access (owner/editor/viewer)
- Invite system for adding members to projects
- Rate limiting on auth endpoints (prevents brute-force attacks)

### 2. Recurring Shared Payments
The system keeps track of expenses that repeat:
- One-time or recurring payments (daily, weekly, monthly, yearly)
- Flexible recurrence: "every X periods" or "X times per period"
- Payment date picker with calendar support
- Receipt image storage (Base64 encoded)
- Optional notes/descriptions
- Per-payment participant weights for fine-grained control
- Edit existing payments with automatic recalculation

### 3. Debt Calculation with Future Projection
BonsCompte generates a projection of upcoming and past shared expenses:
- View debts as of any date (past, present, or future)
- See all recurring payments and their future occurrences
- Smart date navigation that jumps to actual payment dates
- Relative date labels ("Today", "Tomorrow", "in X days", "X days ago")
- Settlement calculations to determine who owes whom

### 4. Weight System (Fairness Calculation)
Every payment has a **default weight**, representing how much each person participates in that expense.

Examples:
- Weight `1.0` → normal shared participation
- Weight `0.5` → someone benefits less (e.g., part-time presence)
- Weight `2.0` → someone occupies more space and pays a higher share
- Weight `0` → excluded from this payment

The system can use these weights to divide the cost fairly.

### 5. Participant Management
- Create participants with default weights
- Participants may or may not have associated user accounts
- Two account types:
  - **User accounts**: Regular participants who pay and owe
  - **Pool accounts**: Shared account per project for tracking collective funds (e.g., house fund, shared bank account)
- Track contributions by participant across all payments

### 6. Transfer System
BonsCompte supports multiple payment types for flexible debt settlement:

- **External expenses**: Regular shared bills (groceries, rent, utilities)
  - Money leaves the system
  - Affects settlements between participants

- **Direct transfers**: Payments between participants to settle debts
  - Use the "Pay back" button to transfer money to a specific person
  - Reduces debt without creating new expenses
  - Perfect for settling partial debts

- **Pool deposits/withdrawals**: Move money to/from a shared account
  - Deposit: "Pay to pool" via the "Deposit" button
  - Only affects pool ownership, not user-to-user settlements
  - Useful for managing collective funds

### 7. Smart Payment Interface
- View all payments (recurring and one-time) with type indicators
- Edit recent payments with full recalculation
- Quick action buttons: "Pay back" (direct transfer to person) or "Deposit" (transfer to pool)
- Include all default participants or customize per-payment weights
- Future date indication with "from" vs "on" preposition
- Past/future badges for date context
- Receipt image storage and viewing

### 8. Security Hardening
The backend includes multiple security layers for production deployment:
- **Rate limiting**: Stricter limits on auth routes (5 req/sec) vs general API (100 req/sec)
- **Scan path blocking**: Silently rejects common scanner probes (/.git, /wp-admin, .php files, etc.)
- **Error sanitization**: Internal errors logged server-side, generic messages returned to clients
- **Non-root containers**: Docker images run as unprivileged users

## Tech Stack

| Component  | Technology |
|-----------|------------|
| Backend   | Rust (Axum 0.8, Tokio, SQLx, JWT) |
| Frontend  | Svelte 5 + SvelteKit 2.48 + Vite 7 + TypeScript |
| Database  | SQLite with automatic migrations |
| Auth      | JWT tokens + Argon2 password hashing |
| Deployment | Docker / Docker Compose |

## Getting Started

### Backend (from `/backend`)
```sh
cargo run                    # Start server on localhost:8000
cargo check                  # Type check without building
cargo build --release        # Production build
```

### Frontend (from `/frontend`)
```sh
npm run dev                  # Start dev server on localhost:5173
npm run build                # Production build
npm run check                # TypeScript + Svelte type checking
npm run lint                 # ESLint + Prettier check
npm run format               # Auto-format with Prettier
```

## Architecture

### Data Model
- **Project**: Owned by a user, contains participants and members
- **Participant**: Entity that pays/owes (may or may not be a user); has account type (user/pool)
- **Member**: User's membership in a project with role (owner/editor/viewer)
- **Payment**: Has payer, amount, date, optional recurrence, contributions, optional receiver (for transfers)
- **Contribution**: Links payment to participant with weight

### Key Features

**Transfer Types**: Flexible payment handling via optional receiver account
- **External expense** (no receiver): Normal shared bills, affects settlements
- **User-to-user transfer** (receiver = user): Direct payment to settle debts
- **Pool transfer** (receiver = pool): Deposits/withdrawals from shared account
  - Only affects pool ownership tracking, not user-to-user settlements
  - Transparent record of who contributed to/withdrew from shared funds

**Weight System**: Fairness through proportional distribution
- Default weight applies to new payments
- Per-payment overrides for specific scenarios
- Zero weight excludes from calculations

**Recurring Payments**: Automatic future projection
- Types: daily, weekly, monthly, yearly
- Either "every X periods" OR "X times per period"
- Optional end date
- Debt calculator expands occurrences for future date queries

**Settlement Calculation**: Smart matching for debt resolution
- Pool accounts excluded from user-to-user settlements
- Greedy algorithm matches debtors with creditors
- Direct mode available for settlements involving only direct transactors
- Automatic recalculation on all changes

**Date Handling**: Timezone-safe with local dates
- Avoids UTC offset issues with ISO strings
- Smart navigation jumps to actual payment dates
- Relative date labels for context

## Deployment

BonsCompte supports three deployment scenarios:

| Scenario | Frontend | Backend | API Base |
|----------|----------|---------|----------|
| **Development** | `localhost:5173` | `localhost:8000` | `http://localhost:8000` |
| **Docker Local** | `localhost:3000` | `localhost:8000` | `http://localhost:8000` |
| **Production** | Behind NGINX | Behind NGINX | `/api` (via reverse proxy) |

### 1. Development Mode (without Docker)

Run the backend and frontend separately for rapid development:

**Terminal 1 - Backend:**
```sh
cd backend
cargo run  # Starts on localhost:8000
```

**Terminal 2 - Frontend:**
```sh
cd frontend
npm run dev  # Starts on localhost:5173
```

No configuration needed - the frontend defaults to `http://localhost:8000` for API calls.

### 2. Docker Local Development

Build and run locally using Docker Compose:

```sh
git clone https://github.com/bassdr/BonsCompte.git
cd BonsCompte
docker compose up --build -d
```

Access the application at:
- Frontend: `http://localhost:3000`
- Backend API: `http://localhost:8000`

The `docker-compose.override.yml` file is automatically merged and builds images with `VITE_API_BASE=http://localhost:8000` for direct backend access.

### 3. Production with NGINX

For production, use the pre-built Docker images behind an NGINX reverse proxy.

#### Quick Start

1. Create a directory and download config:
```sh
mkdir -p /srv/bonscompte
cd /srv/bonscompte
wget https://raw.githubusercontent.com/bassdr/BonsCompte/main/docker-compose.yml
```

2. Create a `.env` file:
```sh
cat > .env << EOF
JWT_SECRET=your-very-secret-key-change-this-in-production
EOF
```

3. Update ports to only bind locally (add to docker-compose.yml or create override):
```yaml
services:
  backend:
    ports:
      - "127.0.0.1:8000:8000"
  frontend:
    ports:
      - "127.0.0.1:3000:3000"
```

4. Start the services:
```sh
docker compose up -d
```

5. Configure NGINX (see [docs/NGINX_CONFIGURATION.md](docs/NGINX_CONFIGURATION.md) for full details):
```nginx
server {
    server_name example.com;
    listen 443 ssl;
    # ... SSL configuration ...

    # Frontend
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # Backend API (strips /api prefix)
    location /api/ {
        proxy_pass http://127.0.0.1:8000/;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

The pre-built frontend image uses `VITE_API_BASE=/api`, so all API calls go through `/api/*` which NGINX forwards to the backend (stripping the prefix).

#### Update to Latest Version

```sh
docker compose pull
docker compose up -d
```

### Docker Images

Multi-platform images are published to GitHub Container Registry for `linux/amd64` and `linux/arm64`:

- `ghcr.io/bassdr/bonscompte-backend:latest`
- `ghcr.io/bassdr/bonscompte-frontend:latest`

Tags include: `latest`, branch name, commit SHA, semantic versions (e.g., `v1.0.0`).

### Environment Variables

**Backend** (`.env` or environment):
```
DATABASE_URL=sqlite:///data/bonscompte.db
JWT_SECRET=your-secret-key
RUST_LOG=info,bonscompte_backend=debug
HOST=0.0.0.0
PORT=8000
```

**Frontend** (build-time via `VITE_API_BASE`):
- Unset: Defaults to `http://localhost:8000` (development)
- `/api`: Relative path for NGINX reverse proxy (production)

### Persistent Data

SQLite database is stored in a Docker volume `sqlite_data`. To use a host directory:

```yaml
services:
  backend:
    volumes:
      - /path/to/local/data:/data
```

### Docker Security & User Management

Both BonsCompte containers follow Docker security best practices by running as **non-root users**.

**Default Setup:**

- **Backend**: UID 1000:GID 1000 (standard app user ID)
- **Frontend**: UID 1001:GID 1001 (Alpine-safe alternative, avoids GID conflicts)

Both run as `appuser`, which is the standard unprivileged user ID for containers.

**Benefits:**
- Limits damage if the application is compromised
- Prevents container escape attacks from escalating to root
- Follows principle of least privilege
- Works out-of-the-box on most systems

**Customizing User for Your Environment:**

You can override the user in `docker-compose.yml` if you need different UIDs:

```yaml
services:
  backend:
    user: "1000:1000"        # Default (recommended) - backend image default
    # OR
    user: "${UID}:${GID}"    # Current host user (development)
    # OR
    user: "nobody"           # System nobody user
    # OR
    user: ""                 # Reset to image default

  frontend:
    user: "1001:1001"        # Default (recommended) - frontend image default
    # OR
    user: "${UID}:${GID}"    # Current host user (development)
    # OR
    user: ""                 # Reset to image default
```

**Volume Permissions:**

When mounting host directories, ensure the container user can access them. Adjust the UID/GID in commands below to match your container's UID.

**Option 1: Adjust host directory to match container user (recommended)**

For backend (UID 1000):
```sh
mkdir -p /path/to/data
chown 1000:1000 /path/to/data
chmod 755 /path/to/data
```

For frontend or custom UID:
```sh
# Replace CONTAINER_UID with your container's actual UID (e.g., 1001 for frontend)
mkdir -p /path/to/data
chown $CONTAINER_UID:$CONTAINER_UID /path/to/data
chmod 755 /path/to/data
```

**Option 2: Use Docker named volumes (simplest)**
```yaml
volumes:
  sqlite_data:
    driver: local
```
No permission setup needed - Docker manages ownership automatically.

**Option 3: Run as current user (development only)**
```yaml
services:
  backend:
    user: "${UID}:${GID}"
    volumes:
      - ./data:/data  # Works with current user's ownership
```

**Option 4: Make directory world-writable (least secure)**
```sh
chmod 777 /path/to/data
```

**Finding Your UID:GID:**
```sh
id
# Output: uid=1000(username) gid=1000(username) ...
```
