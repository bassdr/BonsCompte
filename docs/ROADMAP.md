# Roadmap: Replace the Spreadsheets

## North star

Two real users, two real spreadsheets to beat — no hypothetical users this time:

1. **David's personal finance workbook** ("Dépenses" / "Avoirs"): budget categories
   normalized across frequencies, plus a weekly multi-account net-worth projection to
   2032 with per-account interest rates and Solver-optimized surplus allocation.
2. **The brother's condominium Excel**: syndicate finances — condo fees by unit share,
   operating expenses, reserve fund (fonds de prévoyance), owner statements.

"Better, period" decomposes into three promises, in order of importance:

- **Trust**: never loses data, numbers provably match the spreadsheet, everything
  exportable so it can always be audited in Excel.
- **Less work**: bank import kills manual entry — the reason every previous attempt
  (chalet, personal pivot) died.
- **Things Excel can't do**: budget vs. actual reconciliation, multi-user access with
  roles, read-only share links, no sync-conflict sheets.

## Why evolve BonsCompte instead of contributing to an existing app

| App | What it covers | Why it can't be the vehicle |
|---|---|---|
| Actual Budget | Personal budgeting, bank import | Backward-looking, local-first single-user core; projection and multi-party accounting are architecturally alien |
| Firefly III | Personal finance, recurring txns, CSV import | Single-user, weak projection, no group/weight/pool model; PHP |
| Spliit / IHateMoney / Cospend | Group expense splitting | No accounting, no projection, no budgets, no pools |
| Wave / Akaunting | Small-business accounting | Wrong shape for both use cases; Wave is closed |

No existing project covers **projection + pools + weighted splitting + budgets** — that
combination is BonsCompte's existing core. The engine already expands recurring payments
to arbitrary future dates (`debt_calculator`), models pools with expected minimums, and
handles weighted shares. What we take from the others instead of code: their conventions
(OFX/CSV import formats, categorization-rules UX) so data is portable in both directions
and there is no lock-in even to BonsCompte itself.

## Phases

Effort sizes are relative (S = a weekend or two, M = several weekends, L = a month+ of
spare time). Each phase ends with an acceptance test, not a feature list.

### Phase 0 — Trust foundation (S–M)

The brother left because of early bugs; the brother-in-law lesson is that people trust
what they can audit. Trust ships before features.

- ~~Automated backups (#68)~~ **Done 2026-07-05**: daily 3-2-1 borg backup (local
  snapshots + gentoo-tv.local + tau offsite), restore tested end-to-end. See
  `docs/BACKUPS.md` and `scripts/backup/`.
- ~~Export everything~~ **Done 2026-07-05 (CSV)**: transactions (respecting active
  filters), balances, settlements, and per-pool ownership export to CSV (UTF-8 BOM,
  Excel-safe). XLSX can come later if CSV proves insufficient.
- Bug triage: collect what the brother hit during his condo attempt; fix or close each.
- History validation improvements (#91) fold in here.

**Done when:** a full project can be exported, the DB restored from backup, and the
brother's old bug list is empty.

### Phase 1 — Replace "Avoirs": accounts with interest projection (L)

The engine gap. Everything else in Avoirs is already what `debt_calculator` does.

- First-class financial accounts (checking, REER, CELI, mortgage, loans) — likely an
  extension of participants/pools with `balance` + `annual_rate` + compounding rule.
- Interest applied during projection expansion in `debt_calculator.rs`. **Test-first**;
  this is the critical file. Validate against the spreadsheet's own numbers.
- Net-worth projection view: multi-account chart to an arbitrary horizon (the existing
  horizon/chart machinery on the cashflow page is the starting point), with
  assets/debt/available rollups like the Avoirs summary columns.
- Scenario comparison (later, optional): "extra $500/mo to mortgage vs. CELI" as two
  overlaid projection curves. Deliberately **not** a Solver rebuild — OpenSolver stays
  for the rare true optimization run.

**Done when:** BonsCompte reproduces the Avoirs chart within rounding over the full
2026–2032 range, verified by parallel-running both for one month.

### Phase 2 — Replace "Dépenses": budget view (S)

Cheap once Phase 1 exists — it is arithmetic over recurring payments.

- Budget category on payments (or promote the existing description conventions).
- Normalization table: each category at annual / bi-annual / monthly / per-pay / weekly /
  % of income, computed from recurring payment definitions.
- Income vs. total spending summary line (the sheet's "Total" row).

**Done when:** the Dépenses tab stops being edited.

### Phase 3 — Actuals: bank import + budget vs. actual (M)

The feature that makes the app *better* than the spreadsheet instead of just prettier —
and the one that killed every previous attempt by its absence.

- CSV + OFX/QFX drag-drop import (RBC exports both). Dedupe by hash of
  date+amount+description so overlapping re-imports are safe.
- Rules engine for auto-categorization (`PROVIGO.* → Épicerie`), learning from manual
  corrections.
- Budget vs. actual view: per category, budgeted (Phase 2) against imported actuals,
  monthly and year-to-date.
- Later, optional: zero-touch sync via SimpleFIN Bridge (~$1.50 USD/mo, covers RBC), or
  parsing RBC transaction alert emails. The monthly-CSV workflow is the supported core;
  sync is sugar.

**Done when:** the monthly ritual is "download two files, import, correct a few
categories" — ten minutes or less — for three consecutive months.

### Phase 4 — Condo mode: win the brother back (M)

Structurally, BonsCompte already *is* condo software: owners = participants with
weights (unit shares), condo fees = recurring contribution rules with expected minimums,
reserve fund = pool. Gaps:

- **Multiple pools per project** (operating fund + reserve fund). The one-pool limit is
  documented convention, not enforced in the backend — audit `debt_calculator.rs`,
  ownership tracking, and the frontend for single-pool assumptions, then lift it.
- Owner statement report: per-owner annual/quarterly statement (contributions, share of
  expenses, pool ownership), exportable to XLSX/PDF. This is the auditability feature
  for co-owners who trust Excel.
- Read-only share links: a co-owner sees balances and statements without an account.
  Solves the "my sister won't log in" class of problem forever.
- Import (Phase 3) pointed at the syndicate's bank account.

**Done when:** the brother runs a full quarter of the condominium on BonsCompte without
reopening his Excel.

### Phase 5 — Hardening for a public instance (M)

Only after both spreadsheets are dead:

- DB encryption (#225), account-creation control on the public instance (#28).
- Dark theme (#99), remaining polish.
- Docs + demo instance with seeded example projects (a coloc, a condo, a personal
  budget) so "a couple people said they'd try it" can actually try it in 30 seconds.

## Existing issues, re-homed

| Issue | Phase |
|---|---|
| #68 Setup Backups | 0 |
| #91 Improve history validation | 0 |
| #188 Rename payment → transaction | opportunistic, during Phase 1 model work |
| #98 Cashflow idea from budget spreadsheet | superseded by Phases 1–3 (this document) |
| #225 Encrypt DB | 5 |
| #28 Control account creation | 5 |
| #99 Dark theme | 5 |

## What this plan deliberately does not include

- **Small-business accounting** (invoicing, GST/QST): a second full product; Wave is
  free while the need proves itself.
- **A Solver/optimizer**: scenario comparison covers 90% at 10% of the cost.
- **Marketing for adoption**: the only two users that matter through Phase 4 are David
  and the brother. Both have proven they show up when the tool works — as did the
  coloc, the coffee pool, and the pre-Splitwise versions. General adoption is a Phase 5+
  question, answered only if the first two users are happy.
