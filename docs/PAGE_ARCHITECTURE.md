# BonsCompte — Page Architecture

This document defines the core pages of BonsCompte, their purpose, and their data sources.

The system is layered:

Transactions → Cashflow → Budget → Resolution → Reconciliation

Each page answers a different class of question.

---

## 1. Cashflow

### Purpose
Liquidity and balance dynamics over time.

### Answers
- What is my balance now?
- What will it be in the future?
- When is the minimum?
- What happens in draft mode?
- What happens if I focus on one user?

### Data Sources
- Transactions (confirmed + draft)
- Recurring transaction schedules
- Account definitions
- Focus mode filter
- Draft mode flag

### Contains
- Horizon balance graph
- Min balance detection
- Insolvency warnings
- Time navigation (past / future)
- Optional: transfer solver summary (read-only indicator)

### Does NOT contain
- Category normalization
- Reconciliation logic UI

---

## 2. Transactions

### Purpose
Ground truth ledger editing.

### Answers
- What happened?
- What is scheduled?
- What is draft?

### Existing
- Transaction list page
- Add transaction page
- Edit transaction page

### Data Sources
- Transaction table
- Recurrence definitions

### Should Add
- categoryId (nullable)
- Bulk categorization
- Classification tools:
  - includeInBudget (bool)
  - recurrence override
  - merge tool
- % categorized indicator

This page remains factual only.
No averages. No projections.

---

## 3. Budget

### Purpose
Steady-state normalization of finances.

### Answers
- What is my yearly income?
- What are my yearly expenses by category?
- What is my cost per month / week / paycheck / hour?
- What % of my lifestyle does each category represent?

### Data Sources
- Categorized transactions
- Recurring transactions
- Classification flags (includeInBudget)
- Optional manual adjustments (future)

### Should Create
- Category entity (global)
- Budget aggregation engine
- Yearly canonical normalization
- Derived columns:
  - Monthly
  - Weekly
  - Per paycheck
  - Hourly
- Category breakdown table
- Observed vs projected delta

Budget is horizon-agnostic.
It collapses time into yearly equivalents.

---

## 4. Resolution

### Purpose
Constraint solving and stabilization.

### Answers
- How much must I transfer every X weeks to avoid insolvency?
- What steady inflow balances this account?
- What buffer do I need?

### Data Sources
- Cashflow projection engine
- Recurring transactions
- Draft mode (optional)
- User-defined constraints:
  - cadence
  - buffer target
  - horizon window

### Should Create
- Transfer solver engine
- Constraint UI
- Sensitivity preview
- "Apply as recurring transfer" option

Resolution bridges Budget and Cashflow.
It solves instead of describes.

---

## 5. Reconciliation

### Purpose
Equity and settlement tracking between members.

### Answers
- Who owes what?
- What will reconciliation look like at date X?
- What happens after future scheduled transactions?

### Data Sources
- Transactions
- Pool weights / split rules
- Date cutoff selector

### Should Improve
- Future reconciliation preview
- Settlement history log
- Suggested settlement transactions
- Link to Cashflow impact view

Reconciliation is about fairness.
Cashflow is about liquidity.

They use the same data but answer different questions.

---

## 6. History

### Purpose
Validate the data has not been altered by another user or hacked.

### Answers
- Who did what?
- Was it legitimate?
- Recover from human or threat situations.

### Data Sources
- Unalterable separate table in DB with hash and anti-hack mechanisms

### Status
- Keep as is for now, can be improved later.

---

## 7. Project Settings

### Purpose
Configure the project.

### Answers
- What is the purpose of the project?
- What is the level of security on the project?

### Status
- Keep as is for now.

---

## Summary

| Page           | Responsibility              |
|----------------|-----------------------------|
| Transactions   | Facts                       |
| Cashflow       | Liquidity dynamics          |
| Budget         | Steady-state normalization  |
| Resolution     | Constraint solving          |
| Reconciliation | Equity settlement           |

Each page has a single responsibility.
No page mixes liquidity, normalization, and fairness.
