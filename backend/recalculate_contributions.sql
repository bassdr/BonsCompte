-- Recalculate all contribution amounts with 4-decimal precision
-- This fixes rounding errors from the old 2-decimal calculation

-- We need to do this in a Rust script since SQLite can't easily do this calculation
-- This SQL file documents what needs to happen, but we'll use a Rust utility instead

-- For each payment:
-- 1. Get total weight: SUM(weight) for all contributions
-- 2. For each contribution: new_amount = ROUND(payment.amount * contribution.weight / total_weight * 10000) / 10000
-- 3. Update the contribution amount

-- See recalculate_contributions.rs for the actual implementation
