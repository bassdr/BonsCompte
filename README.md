# BonsCompte

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
- Multi-project support with role-based access (owner/editor/viewer)
- Invite system for adding members to projects

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
- Track contributions by participant across all payments

### 6. Smart Payment Interface
- View all payments (recurring and one-time) with tags
- Edit recent payments with full recalculation
- Quick action buttons: "Pay back" (single participant) or "Include all" (default participants)
- Future date indication with "from" vs "on" preposition
- Past/future badges for date context

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
- **Participant**: Entity that pays/owes (may or may not be a user)
- **Member**: User's membership in a project with role (owner/editor/viewer)
- **Payment**: Has payer, amount, date, optional recurrence, contributions
- **Contribution**: Links payment to participant with weight

### Key Features

**Weight System**: Fairness through proportional distribution
- Default weight applies to new payments
- Per-payment overrides for specific scenarios
- Zero weight excludes from calculations

**Recurring Payments**: Automatic future projection
- Types: daily, weekly, monthly, yearly
- Either "every X periods" OR "X times per period"
- Optional end date
- Debt calculator expands occurrences for future date queries

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
