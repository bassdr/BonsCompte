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

### Docker (Multi-Platform)

BonsCompte is automatically built and published to GitHub Container Registry for both `linux/amd64` and `linux/arm64` architectures (including Raspberry Pi 4).

Images are built on every push to `main` and tagged with:
- `latest` for the default branch
- Git branch name
- Git commit SHA
- Semantic version tags (e.g., `v1.0.0`)

#### Quick Start on Raspberry Pi 4 or Any Server

1. Create a directory for BonsCompte:
```sh
mkdir -p /srv/bonscompte
cd /srv/bonscompte
```

2. Download the docker-compose file:
```sh
wget https://raw.githubusercontent.com/bassdr/BonsCompte/main/docker-compose.yml
```

3. Create a `.env` file with your configuration:
```sh
cat > .env << EOF
JWT_SECRET=your-very-secret-key-change-this-in-production
EOF
```

4. Start the services:
```sh
docker compose up -d
```

The application will be available at:
- Frontend: `http://localhost:3000`
- Backend API: `http://localhost:8000`

#### Development with Local Build

To build and run locally (useful for development):

```sh
git clone https://github.com/bassdr/BonsCompte.git
cd BonsCompte
docker compose up --build -d
```

Then access the application at:
- Frontend: `http://localhost:3000`
- Backend API: `http://localhost:8000`

The `docker-compose.override.yml` file is automatically merged with `docker-compose.yml` and enables the build sections for local development. The frontend automatically uses relative paths (`/api`) to communicate with the backend, so it works seamlessly in development without additional configuration.

#### Update to Latest Version

```sh
docker compose pull
docker compose up -d
```

### Environment Variables

Backend (`.env`):
```
DATABASE_URL=sqlite:///data/bonscompte.db
JWT_SECRET=your-secret-key
RUST_LOG=info,bonscompte_backend=debug
HOST=0.0.0.0
PORT=8000
```

Frontend automatically uses `/api` as the base path for all API calls, which works correctly when deployed behind a reverse proxy (like Nginx). The frontend is served from the same domain and the reverse proxy routes `/api/*` to the backend.

### Persistent Data

SQLite database is stored in a Docker volume `sqlite_data` by default. To use a host directory instead:

```yaml
volumes:
  backend:
    volumes:
      - /path/to/local/data:/data  # Use host directory
```

### Reverse Proxy Setup (Production)

For production deployments with SSL/TLS, use a reverse proxy like Nginx or Traefik to:
1. Terminate SSL/TLS
2. Route frontend and backend through a single domain
3. Keep backend only accessible locally

**Docker Compose (with reverse proxy):**
```yaml
services:
  frontend:
    ports:
      - "127.0.0.1:3000:3000"  # Only localhost
  backend:
    ports:
      - "127.0.0.1:8000:8000"  # Only localhost
```

**Example Nginx configuration:**
```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection $connection_upgrade;
}

location /api/ {
    proxy_pass http://127.0.0.1:8000/;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Real-IP $remote_addr;
}
```

The frontend automatically routes all API calls to `/api/*`, which your reverse proxy forwards to the backend.
