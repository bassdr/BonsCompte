# Phase 1 Design: Accounts with Interest Projection

Goal (from `ROADMAP.md`): BonsCompte reproduces the "Avoirs" tab of the personal
finance workbook — a weekly multi-account net-worth projection to 2032 — within an
agreed tolerance. This document reverse-engineers the spreadsheet's exact model and
proposes the schema and engine changes.

## 1. What the spreadsheet actually computes (reverse-engineered 2026-07-05)

Five accounts, each a column pair (balance, cumulative interest), stepped weekly.
Parameters per account: starting capital, weekly contribution ("Apport", sourced from
the Dépenses tab), annual rate, and one-time Solver-optimized extra payments.

| Account | Interest credited | Rate | Special rules |
|---|---|---|---|
| Compte courant (checking / margin) | Monthly | 5.25% | Interest charged **only when balance < 0** (it's a credit line); flat **7.95 $/month fee when balance < 5 000 $** |
| REER | Annually (Jan 1) | ≈ 7.61% (weighted avg of two funds) | First year prorated (DAYS360); **+20% of each contribution** credited immediately (models the RRSP tax refund) |
| CELI | Annually (Jan 1) | ≈ 3.07% (weighted avg of three vehicles) | First year prorated |
| Hypothèque | **Semi-annually** (Canadian mortgage convention) | 1.84% until **2026-05-23 renewal**, then `prime × 75%` (= checking rate × 0.75) | Balance clamped at 0 (paid off = stops) |
| Tesla prêt | Monthly | 3.34% until **2027-05-27 renewal**, then `prime × 75%` | Balance clamped at 0 |

Structural rules:

- **Checking is the residual account**: every other account's contribution flows
  through it (weekly net budget surplus/deficit from Dépenses, mortgage and loan
  payments, transfers to REER/CELI). This is just "recurring transfers between
  accounts" — BonsCompte's existing payment model.
- **Rollups**: Avoirs (net worth) = accounts + property value (fixed 780 k$);
  Dette = negative balances; Placements = positive investment balances; Crédit
  disponible = 80% of property value + loan balances.
- **Solver variables** ("Paiement supplémentaire") are one-time reallocations
  (e.g., −26 000 $ from CELI, −147 980 $ extra on the mortgage, 73 990 $ down
  payment from checking). In BonsCompte these are ordinary one-time transfer
  payments — the *optimization* stays in the spreadsheet (see ROADMAP: no Solver
  rebuild; scenario comparison later).

## 2. Proposed data model

New table (keeps `participants` narrow; supports rate changes like renewals):

```sql
CREATE TABLE IF NOT EXISTS account_interest_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    participant_id INTEGER NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
    start_date TEXT NOT NULL,              -- rule applies from this date (inclusive)
    end_date TEXT,                         -- NULL = open-ended; renewal = close old rule, add new
    annual_rate REAL NOT NULL,             -- e.g. 0.0761
    period TEXT NOT NULL DEFAULT 'monthly' -- 'monthly' | 'semiannual' | 'annual'
        CHECK (period IN ('monthly','semiannual','annual')),
    only_when_negative INTEGER NOT NULL DEFAULT 0,  -- credit-line style
    monthly_fee REAL,                      -- flat fee, charged with the period tick
    fee_threshold REAL                     -- fee applies when balance < threshold
);
```

Plus one column on `participants`:

```sql
ALTER TABLE participants ADD COLUMN initial_balance REAL NOT NULL DEFAULT 0;
-- balance as of the participant's creation; projections start from here,
-- so users don't need to backfill years of history
```

Deliberately **not** modeled (v1): the REER +20% refund (model it as a separate
recurring deposit rule — it is literally "for every 740 $ contribution, 148 $
extra arrives"; a recurring payment covers it); clamp-at-zero for loans (v1
projects past zero and the chart shows it — acceptable, revisit); weighted-average
rate formulas (user enters the resulting number).

## 3. Engine changes (`debt_calculator.rs` — critical file, test-first)

Today the calculator expands recurring payments into dated occurrences and the
frontend integrates running balances. Interest depends on the running balance, so
it must be computed **chronologically in the backend**:

1. Collect all real occurrences up to the target date (existing logic, unchanged).
2. For each account with interest rules: walk time from projection start, maintain
   running balance (initial_balance + flows), and at each period boundary
   (month/half-year/year change, matching the spreadsheet's `MONTH()>MONTH()`
   convention) emit a **synthetic interest occurrence**:
   `amount = balance_at_boundary × rate / periods_per_year` (+ fee if applicable),
   respecting `only_when_negative` and the rule's date window.
3. Synthetic occurrences are flagged (`is_interest: true`) so the UI can style
   them and settlements can handle attribution.

**Attribution** (the settlement-invariant question): interest on a **pool** account
is credited/charged to ownership **proportionally to each participant's current
ownership share** — same treatment as "External funds to pool". Interest never
enters user-to-user `paid_map`/`owed_map`. For the personal-finance case (single
participant) this degenerates to "it's all yours", and for the condo reserve fund
it does the legally correct thing. This respects existing invariants #1 and #5 in
CLAUDE.md.

First-year proration: the spreadsheet prorates with DAYS360 from `today()`. We
prorate from the projection start date using actual day counts. This is one known
divergence source (see tolerance).

## 4. Acceptance test

Fixture project mirroring the workbook: 5 accounts with the table above, weekly
recurring transfers matching the Apport column, one-time transfers for the Solver
amounts. Assert the projected per-account balances against the spreadsheet's
computed values (extracted to a CSV fixture) at monthly checkpoints 2026→2032.

**Tolerance target: within 0.1% per account at every checkpoint** (DAYS360 vs
actual/365 and weekday-stepping differences make cent-exact unrealistic; 0.1% on
a 400 k$ mortgage is ~400 $ over six years — reviewable, and tightenable later).

## 5. Open questions for David before implementation

1. **Tolerance**: is 0.1% at monthly checkpoints acceptable for the "trust it over
   Excel" milestone, or do you want the engine to replicate DAYS360/weekday
   conventions exactly?
2. **Clamp-at-zero for loans** in v1 or defer? (Affects the mortgage end-game
   ~2040s, not the 2026–2032 window, since it doesn't pay off in range.)
3. **Where accounts live**: these personal accounts are `pool`-type participants in
   a single-member project (gets ownership tracking + charts for free) — confirm,
   or should a new `account_type = 'asset'` be introduced to keep them out of
   pool-specific UI (warnings, expected minimum)?
4. **Interest occurrence granularity in the UI**: shown as regular transactions in
   the lists (auditable, noisy) or only folded into the chart (clean, less
   auditable)? Proposal: shown, with an "Interest" badge, filterable.
