# BonsCompte

BonsCompte is a lightweight financial coordination tool designed for shared living situations such as housemates, cooperatives, or small collectives.
It helps groups track recurring shared bills, future expenses, and fairness over time through a simple weighting system.

The project includes:
- Rust backend (Axum + MongoDB)
- SvelteKit frontend
- MongoDB database

## Purpose

The goal of BonsCompte is to make it easy for a group of people to fairly manage shared expenses without spreadsheets or manual tracking.

## Common Use Cases

- **Roommates or cooperative households**  
  Track who paid what, ensure fair distribution of recurring and one-off expenses, and keep a clear history for everyone.

- **Shared bank accounts with multiple owners**  
  The app helps determine which part of the shared balance belongs to each participant over time.
  Even if all operations go through a common account, the system tracks individual contributions and consumption 
  so the implicit “ownership” of the account remains balanced and transparent.

- **Groups with irregular participation**  
  Some participants join only for certain bills or activities. Per-bill weight overrides make this effortless.

- **Co-parenting or alternating household setups**  
  Handle situations where different people are present at different times, with recurring or seasonal expenses.

## Core Features

### 1. Recurring Shared Bills
The system keeps track of expenses that repeat:
- Rent
- Electricity
- Internet
- Any periodic shared cost

Each bill includes:
- Name
- Amount
- Frequency (once, weekly, monthly, yearly…)
- First expected payment date
- Participants with weights
- _Optional_ Images of receipts or invoices
- _Optional_ text notes

The backend automatically computes future occurrences.

### 2. Future Payment Timeline
BonsCompte generates a projection of upcoming shared expenses.
This helps the group anticipate:
- Cash flow
- Who is responsible for which payments
- How costs accumulate over the next days, weeks, or months

### 3. Weight System (Fairness Calculation)
Every bill has a **default weight**, representing how much each person participates in that expense.

Examples:
- Weight `1.0` → normal shared participation
- Weight `0.5` → someone benefits less (e.g., part-time presence)
- Weight `2.0` → someone occupies more space and pays a higher share

The system can use these weights to divide the cost fairly.

### 4. Weight of Zero

A bill with **weight = 0** is still stored, but it is excluded from fairness and cost-sharing calculations.

This is useful for scenarios such as:
- Adding participants who never contribute to regular shared expenses (e.g., a shared bank account used only as a ledger participant).
- Adding participants who normally do not pay common bills but may still be included selectively through the bill-specific weight override system.

## Tech Stack

| Component  | Technology |
|-----------|------------|
| Backend   | Rust (Axum, Tokio, MongoDB driver) |
| Frontend  | Svelte 5 + SvelteKit 2.48 + Vite 7 |
| Database  | MongoDB |
| Deployment | Docker / Docker Compose |

## Running the Project

```sh
docker compose up --build
```
