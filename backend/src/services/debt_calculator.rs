use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::error::AppResult;
use crate::models::Payment;

#[derive(Debug, Serialize)]
pub struct ParticipantBalance {
    pub participant_id: i64,
    pub participant_name: String,
    pub total_paid: f64,
    pub total_owed: f64,
    pub net_balance: f64,
}

#[derive(Debug, Serialize)]
pub struct Debt {
    pub from_participant_id: i64,
    pub from_participant_name: String,
    pub to_participant_id: i64,
    pub to_participant_name: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairwisePaymentBreakdown {
    pub payment_id: i64,
    pub description: String,
    pub occurrence_date: String,
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct PairwiseBalance {
    pub participant_id: i64,
    pub participant_name: String,
    pub other_participant_id: i64,
    pub other_participant_name: String,
    pub amount_paid_for: f64, // Amount this participant paid for other
    pub amount_owed_by: f64,  // Amount other paid for this participant
    pub net: f64,             // paid_for - owed_by (positive = they owe you)
    pub paid_for_breakdown: Vec<PairwisePaymentBreakdown>, // Details of what we paid for them
    pub owed_by_breakdown: Vec<PairwisePaymentBreakdown>, // Details of what they paid for us
}

#[derive(Debug, Serialize)]
pub struct PoolOwnershipEntry {
    pub participant_id: i64,
    pub participant_name: String,
    pub contributed: f64, // Total deposited to pool
    pub consumed: f64,    // Total share of pool-paid expenses
    pub ownership: f64,   // contributed - consumed
    pub contributed_breakdown: Vec<PairwisePaymentBreakdown>, // Details of contributions
    pub consumed_breakdown: Vec<PairwisePaymentBreakdown>, // Details of consumption
}

#[derive(Debug, Serialize)]
pub struct PoolOwnership {
    pub pool_id: i64,
    pub pool_name: String,
    pub entries: Vec<PoolOwnershipEntry>,
    pub total_balance: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentOccurrence {
    pub payment_id: i64,
    pub description: String,
    pub amount: f64,
    pub occurrence_date: String,
    pub payer_id: Option<i64>,
    pub is_recurring: bool,
    // Internal transfer support
    // None = external expense (money leaves system, affects settlements)
    // Some = internal transfer (money moves between accounts, only affects pool ownership)
    pub receiver_account_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct DebtSummary {
    pub balances: Vec<ParticipantBalance>,
    pub settlements: Vec<Debt>,
    pub direct_settlements: Vec<Debt>,
    pub target_date: String,
    pub occurrences: Vec<PaymentOccurrence>,
    pub pairwise_balances: Vec<PairwiseBalance>,
    pub pool_ownerships: Vec<PoolOwnership>,
}

// Cashflow Planning Structures

#[derive(Debug, Serialize)]
pub struct MonthlyBalance {
    pub month: String, // "2026-01" format
    pub participant_id: i64,
    pub participant_name: String,
    pub net_balance: f64, // Cumulative balance at end of month
}

#[derive(Debug, Serialize)]
pub struct RecurringContributionRecommendation {
    pub participant_id: i64,
    pub participant_name: String,
    pub recommended_amount: f64, // Amount per payment at chosen frequency
    pub frequency_type: String,  // 'weekly', 'monthly', 'yearly'
    pub frequency_interval: i32, // Every X periods (e.g., 2 for bi-weekly)
    pub current_trend: f64,      // Monthly rate of change
    pub calculation_method: String, // "simple_average" or "linear_regression"
}

#[derive(Debug, Serialize)]
pub struct PoolOwnershipSnapshot {
    pub participant_id: i64,
    pub participant_name: String,
    pub ownership: f64,
}

#[derive(Debug, Serialize)]
pub struct PoolMonthlyBalance {
    pub month: String,
    pub total_balance: f64,
    pub participant_ownerships: Vec<PoolOwnershipSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct PoolEvolution {
    pub pool_id: i64,
    pub pool_name: String,
    pub monthly_balances: Vec<PoolMonthlyBalance>,
}

#[derive(Debug, Serialize)]
pub struct RecurringPaymentToConsolidate {
    pub payment_id: i64,
    pub description: String,
    pub amount: f64,
    pub payer_id: i64,
    pub payer_name: String,
    pub recurrence_type: String,
    pub recurrence_interval: i32,
}

#[derive(Debug, Serialize)]
pub struct CashflowProjection {
    pub start_date: String,
    pub end_date: String,
    pub months: Vec<String>, // ["2026-01", "2026-02", ...]
    pub monthly_balances: Vec<MonthlyBalance>,
    pub recommendations: Vec<RecurringContributionRecommendation>,
    pub pool_evolutions: Vec<PoolEvolution>,
    pub consolidate_mode: bool,
    pub payments_to_consolidate: Vec<RecurringPaymentToConsolidate>, // Empty if consolidate_mode=false
}

/// Calculate debts as of today
pub async fn calculate_debts(pool: &SqlitePool, project_id: i64) -> AppResult<DebtSummary> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    calculate_debts_at_date(pool, project_id, &today).await
}

/// Calculate debts as of a specific target date
pub async fn calculate_debts_at_date(
    pool: &SqlitePool,
    project_id: i64,
    target_date: &str,
) -> AppResult<DebtSummary> {
    let target = parse_date(target_date).unwrap_or_else(|| chrono::Utc::now().date_naive());

    // Get all participants for this project (including account_type)
    let participants: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, account_type FROM participants WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(pool)
            .await?;

    let participant_map: HashMap<i64, String> = participants
        .iter()
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();

    // Track which participants are pool accounts (excluded from settlements)
    let pool_participants: std::collections::HashSet<i64> = participants
        .iter()
        .filter(|(_, _, account_type)| account_type == "pool")
        .map(|(id, _, _)| *id)
        .collect();

    // Get all payments for this project
    let payments: Vec<Payment> = sqlx::query_as("SELECT * FROM payments WHERE project_id = ?")
        .bind(project_id)
        .fetch_all(pool)
        .await?;

    // Generate all payment occurrences (including recurring expansions)
    let mut all_occurrences: Vec<PaymentOccurrence> = Vec::new();

    for payment in &payments {
        let occurrences = generate_payment_occurrences(payment, target);
        all_occurrences.extend(occurrences);
    }

    // Get contributions for each payment
    let contributions: Vec<(i64, i64, f64)> = sqlx::query_as(
        "SELECT c.payment_id, c.participant_id, c.amount
         FROM contributions c
         JOIN payments p ON c.payment_id = p.id
         WHERE p.project_id = ?",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // Build contribution map: payment_id -> [(participant_id, amount)]
    let mut contribution_map: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
    for (payment_id, participant_id, amount) in contributions {
        contribution_map
            .entry(payment_id)
            .or_default()
            .push((participant_id, amount));
    }

    // Calculate total paid and owed based on occurrences
    // Also track pairwise amounts: (payer_id, contributor_id) -> (total_amount, breakdown)
    //
    // Transfer types:
    // 1. External expense (receiver_account_id IS NULL): Normal expense, affects settlements
    // 2. User → User transfer: Direct payment, affects settlements (reduces debt)
    // 3. User → Pool transfer: Only affects pool ownership, NOT settlements
    // 4. Pool → User transfer: Only affects pool ownership, NOT settlements
    let mut paid_map: HashMap<i64, f64> = HashMap::new();
    let mut owed_map: HashMap<i64, f64> = HashMap::new();
    let mut pairwise_map: HashMap<(i64, i64), (f64, Vec<PairwisePaymentBreakdown>)> =
        HashMap::new();

    for occurrence in &all_occurrences {
        // Check if this is a pool-related transfer (should not affect settlements)
        if let Some(receiver_id) = occurrence.receiver_account_id {
            let receiver_is_pool = pool_participants.contains(&receiver_id);
            let payer_is_pool = occurrence
                .payer_id
                .map(|id| pool_participants.contains(&id))
                .unwrap_or(false);

            if receiver_is_pool || payer_is_pool {
                // Pool transfer - skip for settlement calculations (handled in pool ownership)
                continue;
            }

            // User-to-user transfer: treat as direct payment
            // Payer gives money directly to receiver, reducing payer's debt to receiver
            if let Some(payer_id) = occurrence.payer_id {
                // Add to payer's "paid" total
                *paid_map.entry(payer_id).or_insert(0.0) += occurrence.amount;

                // Add to receiver's "owed" total (they received the money)
                *owed_map.entry(receiver_id).or_insert(0.0) += occurrence.amount;

                // Track pairwise: payer paid this amount directly to receiver
                let entry = pairwise_map
                    .entry((payer_id, receiver_id))
                    .or_insert((0.0, Vec::new()));
                entry.0 += occurrence.amount;
                entry.1.push(PairwisePaymentBreakdown {
                    payment_id: occurrence.payment_id,
                    description: occurrence.description.clone(),
                    occurrence_date: occurrence.occurrence_date.clone(),
                    amount: occurrence.amount,
                });
            }
            continue;
        }

        // External expense (receiver_account_id IS NULL)
        // Add to paid total for payer
        let payer_is_pool = occurrence
            .payer_id
            .map(|id| pool_participants.contains(&id))
            .unwrap_or(false);

        if let Some(payer_id) = occurrence.payer_id {
            *paid_map.entry(payer_id).or_insert(0.0) += occurrence.amount;

            // Track pairwise amounts: how much payer paid for each contributor
            // Skip if payer is pool (pool relationships are tracked in pool ownership)
            if !payer_is_pool {
                if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                    for (contributor_id, amount) in contribs {
                        // payer paid this amount for contributor
                        let entry = pairwise_map
                            .entry((payer_id, *contributor_id))
                            .or_insert((0.0, Vec::new()));
                        entry.0 += amount;
                        entry.1.push(PairwisePaymentBreakdown {
                            payment_id: occurrence.payment_id,
                            description: occurrence.description.clone(),
                            occurrence_date: occurrence.occurrence_date.clone(),
                            amount: *amount,
                        });
                    }
                }
            }
        }

        // Add to owed totals from contributions
        // IMPORTANT: Only add to owed if the payer is a USER (not pool)
        // When pool pays for expenses, the debt is owed TO the pool, which is
        // tracked separately in pool ownership. Including pool-paid debts in
        // owed_map would create an imbalance in user-to-user settlements since
        // pool is excluded from settlement calculations.
        if !payer_is_pool {
            if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                for (participant_id, amount) in contribs {
                    *owed_map.entry(*participant_id).or_insert(0.0) += amount;
                }
            }
        }
    }

    // Calculate balances
    let mut balances: Vec<ParticipantBalance> = participants
        .iter()
        .map(|(id, name, _)| {
            let total_paid = paid_map.get(id).copied().unwrap_or(0.0);
            let total_owed = owed_map.get(id).copied().unwrap_or(0.0);
            ParticipantBalance {
                participant_id: *id,
                participant_name: name.clone(),
                total_paid,
                total_owed,
                net_balance: total_paid - total_owed,
            }
        })
        .collect();

    // Sort by net balance for settlement calculation
    balances.sort_by(|a, b| a.net_balance.partial_cmp(&b.net_balance).unwrap());

    // Calculate optimal settlements (greedy algorithm)
    // Exclude pool accounts from settlements
    let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

    // Sort occurrences by date
    all_occurrences.sort_by(|a, b| a.occurrence_date.cmp(&b.occurrence_date));

    // Build pairwise balances from the pairwise_map
    // For each participant, show their relationship with every other participant
    let mut pairwise_balances: Vec<PairwiseBalance> = Vec::new();
    for (id, name, _) in &participants {
        for (other_id, other_name, _) in &participants {
            if id == other_id {
                continue; // Skip self
            }

            // How much did 'id' pay for 'other_id'?
            let (amount_paid_for, paid_for_breakdown): (f64, Vec<PairwisePaymentBreakdown>) =
                if let Some((amt, breakdown)) = pairwise_map.get(&(*id, *other_id)) {
                    (*amt, breakdown.clone())
                } else {
                    (0.0, Vec::new())
                };
            // How much did 'other_id' pay for 'id'?
            let (amount_owed_by, owed_by_breakdown): (f64, Vec<PairwisePaymentBreakdown>) =
                if let Some((amt, breakdown)) = pairwise_map.get(&(*other_id, *id)) {
                    (*amt, breakdown.clone())
                } else {
                    (0.0, Vec::new())
                };

            // Only include if there's any relationship
            if amount_paid_for > 0.01 || amount_owed_by > 0.01 {
                pairwise_balances.push(PairwiseBalance {
                    participant_id: *id,
                    participant_name: name.clone(),
                    other_participant_id: *other_id,
                    other_participant_name: other_name.clone(),
                    amount_paid_for,
                    amount_owed_by,
                    net: amount_paid_for - amount_owed_by,
                    paid_for_breakdown,
                    owed_by_breakdown,
                });
            }
        }
    }

    // Calculate pool ownership for all pool accounts
    let pool_account_list: Vec<(i64, String)> = participants
        .iter()
        .filter(|(_, _, account_type)| account_type == "pool")
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();

    let mut pool_ownerships: Vec<PoolOwnership> = Vec::new();

    for (pool_id, pool_name) in pool_account_list {
        // Track contributions to pool (deposited) and consumption from pool (expenses)
        //
        // Pool ownership is affected by:
        // 1. EXTERNAL expenses where pool is payer: decreases contributor ownership (consumption)
        // 2. EXTERNAL expenses where pool is contributor: increases payer's ownership
        // 3. INTERNAL transfers TO pool: increases sender's ownership (deposit)
        // 4. INTERNAL transfers FROM pool: decreases receiver's ownership (withdrawal)

        // Track ownership amounts and transaction breakdowns
        // HashMap<participant_id, (contributed_total, consumed_total, contributed_breakdown, consumed_breakdown)>
        let mut ownership_map: HashMap<
            i64,
            (
                f64,
                f64,
                Vec<PairwisePaymentBreakdown>,
                Vec<PairwisePaymentBreakdown>,
            ),
        > = HashMap::new();

        for occurrence in &all_occurrences {
            // Handle internal transfers (receiver_account_id IS NOT NULL)
            if let Some(receiver_id) = occurrence.receiver_account_id {
                if let Some(payer_id) = occurrence.payer_id {
                    if receiver_id == pool_id && payer_id != pool_id {
                        // Internal transfer TO pool: payer's ownership increases
                        let entry = ownership_map.entry(payer_id).or_insert((
                            0.0,
                            0.0,
                            Vec::new(),
                            Vec::new(),
                        ));
                        entry.0 += occurrence.amount; // contributed (deposited to pool)
                        entry.2.push(PairwisePaymentBreakdown {
                            payment_id: occurrence.payment_id,
                            description: occurrence.description.clone(),
                            occurrence_date: occurrence.occurrence_date.clone(),
                            amount: occurrence.amount,
                        });
                    } else if payer_id == pool_id && receiver_id != pool_id {
                        // Internal transfer FROM pool: receiver's ownership decreases
                        let entry = ownership_map.entry(receiver_id).or_insert((
                            0.0,
                            0.0,
                            Vec::new(),
                            Vec::new(),
                        ));
                        entry.1 += occurrence.amount; // consumed (withdrawn from pool)
                        entry.3.push(PairwisePaymentBreakdown {
                            payment_id: occurrence.payment_id,
                            description: occurrence.description.clone(),
                            occurrence_date: occurrence.occurrence_date.clone(),
                            amount: occurrence.amount,
                        });
                    }
                }
                continue; // Skip contribution-based logic for internal transfers
            }

            // Handle external expenses (receiver_account_id IS NULL)
            if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                // Check if pool is involved in this payment
                let pool_contrib = contribs.iter().find(|(pid, _)| *pid == pool_id);

                if let Some(payer_id) = occurrence.payer_id {
                    if payer_id == pool_id {
                        // Pool is the payer: each contributor's ownership decreases
                        for (contributor_id, amount) in contribs {
                            if *contributor_id != pool_id {
                                let entry = ownership_map.entry(*contributor_id).or_insert((
                                    0.0,
                                    0.0,
                                    Vec::new(),
                                    Vec::new(),
                                ));
                                entry.1 += amount; // consumed
                                entry.3.push(PairwisePaymentBreakdown {
                                    payment_id: occurrence.payment_id,
                                    description: occurrence.description.clone(),
                                    occurrence_date: occurrence.occurrence_date.clone(),
                                    amount: *amount,
                                });
                            }
                        }
                    } else if let Some((_, pool_amount)) = pool_contrib {
                        // Pool is a contributor: the payer's ownership increases
                        let entry = ownership_map.entry(payer_id).or_insert((
                            0.0,
                            0.0,
                            Vec::new(),
                            Vec::new(),
                        ));
                        entry.0 += pool_amount; // contributed
                        entry.2.push(PairwisePaymentBreakdown {
                            payment_id: occurrence.payment_id,
                            description: occurrence.description.clone(),
                            occurrence_date: occurrence.occurrence_date.clone(),
                            amount: *pool_amount,
                        });
                    }
                }
            }
        }

        // Build ownership entries for non-pool participants
        let mut entries: Vec<PoolOwnershipEntry> = participants
            .iter()
            .filter(|(id, _, account_type)| *id != pool_id && account_type != "pool")
            .filter_map(|(id, name, _)| {
                let (contributed, consumed, contributed_breakdown, consumed_breakdown) =
                    ownership_map
                        .get(id)
                        .map(|(c, con, cb, conb)| (*c, *con, cb.clone(), conb.clone()))
                        .unwrap_or((0.0, 0.0, Vec::new(), Vec::new()));
                if contributed > 0.01 || consumed > 0.01 {
                    Some(PoolOwnershipEntry {
                        participant_id: *id,
                        participant_name: name.clone(),
                        contributed,
                        consumed,
                        ownership: contributed - consumed,
                        contributed_breakdown,
                        consumed_breakdown,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by ownership descending
        entries.sort_by(|a, b| {
            b.ownership
                .partial_cmp(&a.ownership)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_balance: f64 = entries.iter().map(|e| e.ownership).sum();

        pool_ownerships.push(PoolOwnership {
            pool_id,
            pool_name,
            entries,
            total_balance,
        });
    }

    // Calculate direct-only settlements based on pairwise relationships
    // Pass balances to ensure we respect net balance constraints
    let direct_settlements =
        calculate_direct_settlements(&pairwise_balances, &balances, &pool_participants);

    Ok(DebtSummary {
        balances,
        settlements,
        direct_settlements,
        target_date: target_date.to_string(),
        occurrences: all_occurrences,
        pairwise_balances,
        pool_ownerships,
    })
}

/// Generate all occurrences of a payment up to target_date
fn generate_payment_occurrences(
    payment: &Payment,
    target_date: NaiveDate,
) -> Vec<PaymentOccurrence> {
    let mut occurrences = Vec::new();

    let start_date = match parse_date(&payment.payment_date) {
        Some(d) => d,
        None => return occurrences,
    };

    // If start date is after target, no occurrences
    if start_date > target_date {
        return occurrences;
    }

    if !payment.is_recurring {
        // Single payment - just one occurrence
        occurrences.push(PaymentOccurrence {
            payment_id: payment.id,
            description: payment.description.clone(),
            amount: payment.amount,
            occurrence_date: payment.payment_date.clone(),
            payer_id: payment.payer_id,
            is_recurring: false,
            receiver_account_id: payment.receiver_account_id,
        });
        return occurrences;
    }

    // Recurring payment - generate occurrences
    let recurrence_type = payment.recurrence_type.as_deref().unwrap_or("monthly");
    let interval = payment.recurrence_interval.unwrap_or(1) as u32;
    let end_date = payment
        .recurrence_end_date
        .as_ref()
        .and_then(|d| parse_date(d))
        .unwrap_or(target_date);
    let effective_end = end_date.min(target_date);

    // Handle enhanced recurrence patterns
    match recurrence_type {
        "weekly" => {
            // Check for weekday selection pattern
            if let Some(weekdays_json) = &payment.recurrence_weekdays {
                return generate_weekly_with_weekdays(
                    payment,
                    start_date,
                    effective_end,
                    interval,
                    weekdays_json,
                );
            }
        }
        "monthly" => {
            // Check for monthday selection pattern (only valid when interval = 1)
            if interval == 1 {
                if let Some(monthdays_json) = &payment.recurrence_monthdays {
                    return generate_monthly_with_monthdays(
                        payment,
                        start_date,
                        effective_end,
                        monthdays_json,
                    );
                }
            }
        }
        "yearly" => {
            // Check for month selection pattern (only valid when interval = 1)
            if interval == 1 {
                if let Some(months_json) = &payment.recurrence_months {
                    return generate_yearly_with_months(
                        payment,
                        start_date,
                        effective_end,
                        months_json,
                    );
                }
            }
        }
        _ => {}
    }

    // Legacy/simple recurrence - every X periods
    // Also handle deprecated times_per for backwards compatibility
    let times_per = payment.recurrence_times_per;
    let (effective_interval, effective_type) = if let Some(times) = times_per {
        calculate_times_per_interval(recurrence_type, interval, times as u32)
    } else {
        (interval, recurrence_type.to_string())
    };

    let mut current = start_date;
    while current <= effective_end {
        occurrences.push(PaymentOccurrence {
            payment_id: payment.id,
            description: payment.description.clone(),
            amount: payment.amount,
            occurrence_date: current.format("%Y-%m-%d").to_string(),
            payer_id: payment.payer_id,
            is_recurring: true,
            receiver_account_id: payment.receiver_account_id,
        });

        current = match add_interval(current, &effective_type, effective_interval) {
            Some(d) => d,
            None => break,
        };
    }

    occurrences
}

/// Generate weekly occurrences with specific weekday selections
/// weekdays_json format: [[1,3],[5]] - array of arrays, one per week in cycle
/// Each inner array contains weekday numbers (0=Sun, 1=Mon, ..., 6=Sat)
fn generate_weekly_with_weekdays(
    payment: &Payment,
    start_date: NaiveDate,
    end_date: NaiveDate,
    interval: u32,
    weekdays_json: &str,
) -> Vec<PaymentOccurrence> {
    let mut occurrences = Vec::new();

    // Parse weekdays JSON - array of arrays
    let week_patterns: Vec<Vec<u32>> = match serde_json::from_str(weekdays_json) {
        Ok(p) => p,
        Err(_) => return occurrences, // Invalid JSON, return empty
    };

    if week_patterns.is_empty() {
        return occurrences;
    }

    // Calculate the start of the week containing start_date
    let start_weekday = start_date.weekday().num_days_from_sunday();
    let week_start = start_date - chrono::Duration::days(start_weekday as i64);

    // Total weeks in the pattern cycle
    let cycle_weeks = interval;

    // Iterate through weeks from start
    let mut cycle_week = 0u32; // Which week in the cycle (0-indexed)
    let mut current_week_start = week_start;

    // Maximum iterations to prevent infinite loops (10 years of weeks)
    let max_iterations = 52 * 10;
    let mut iterations = 0;

    while current_week_start <= end_date && iterations < max_iterations {
        iterations += 1;

        // Get the pattern for this week in the cycle
        let pattern_idx = (cycle_week % week_patterns.len() as u32) as usize;
        let weekdays = &week_patterns[pattern_idx];

        // Generate occurrences for each selected weekday
        for &weekday in weekdays {
            let occurrence_date = current_week_start + chrono::Duration::days(weekday as i64);

            // Must be >= start_date and <= end_date
            if occurrence_date >= start_date && occurrence_date <= end_date {
                occurrences.push(PaymentOccurrence {
                    payment_id: payment.id,
                    description: payment.description.clone(),
                    amount: payment.amount,
                    occurrence_date: occurrence_date.format("%Y-%m-%d").to_string(),
                    payer_id: payment.payer_id,
                    is_recurring: true,
                    receiver_account_id: payment.receiver_account_id,
                });
            }
        }

        // Move to next week
        cycle_week += 1;

        // After completing the cycle, skip weeks based on interval
        if cycle_week >= cycle_weeks {
            cycle_week = 0;
            // Move forward by cycle_weeks weeks (already processed them)
        }

        current_week_start += chrono::Duration::weeks(1);
    }

    // Sort by date
    occurrences.sort_by(|a, b| a.occurrence_date.cmp(&b.occurrence_date));
    occurrences
}

/// Generate monthly occurrences on specific days
/// monthdays_json format: [1, 15, 28] - array of day numbers
fn generate_monthly_with_monthdays(
    payment: &Payment,
    start_date: NaiveDate,
    end_date: NaiveDate,
    monthdays_json: &str,
) -> Vec<PaymentOccurrence> {
    let mut occurrences = Vec::new();

    // Parse monthdays JSON - array of day numbers
    let monthdays: Vec<u32> = match serde_json::from_str(monthdays_json) {
        Ok(d) => d,
        Err(_) => return occurrences,
    };

    if monthdays.is_empty() {
        return occurrences;
    }

    // Start from the month of start_date
    let mut current_year = start_date.year();
    let mut current_month = start_date.month();

    // Maximum iterations (20 years of months)
    let max_iterations = 12 * 20;
    let mut iterations = 0;

    while iterations < max_iterations {
        iterations += 1;

        // Get the last day of current month
        let days_in_month = get_days_in_month(current_year, current_month);

        // Generate occurrences for each selected day
        for &day in &monthdays {
            // Clamp day to valid range (handle dates > 28 for short months)
            let actual_day = day.min(days_in_month);

            if let Some(occurrence_date) =
                NaiveDate::from_ymd_opt(current_year, current_month, actual_day)
            {
                // Must be >= start_date and <= end_date
                if occurrence_date >= start_date && occurrence_date <= end_date {
                    occurrences.push(PaymentOccurrence {
                        payment_id: payment.id,
                        description: payment.description.clone(),
                        amount: payment.amount,
                        occurrence_date: occurrence_date.format("%Y-%m-%d").to_string(),
                        payer_id: payment.payer_id,
                        is_recurring: true,
                        receiver_account_id: payment.receiver_account_id,
                    });
                }
            }
        }

        // Check if we've passed end_date
        if let Some(first_of_next) = NaiveDate::from_ymd_opt(current_year, current_month, 1)
            .and_then(|d| d.checked_add_months(Months::new(1)))
        {
            if first_of_next > end_date {
                break;
            }
            current_year = first_of_next.year();
            current_month = first_of_next.month();
        } else {
            break;
        }
    }

    // Sort by date
    occurrences.sort_by(|a, b| a.occurrence_date.cmp(&b.occurrence_date));
    occurrences
}

/// Generate yearly occurrences in specific months
/// months_json format: [1, 6, 12] - array of month numbers (1-12)
fn generate_yearly_with_months(
    payment: &Payment,
    start_date: NaiveDate,
    end_date: NaiveDate,
    months_json: &str,
) -> Vec<PaymentOccurrence> {
    let mut occurrences = Vec::new();

    // Parse months JSON - array of month numbers
    let months: Vec<u32> = match serde_json::from_str(months_json) {
        Ok(m) => m,
        Err(_) => return occurrences,
    };

    if months.is_empty() {
        return occurrences;
    }

    // Get the day from start_date to use for all occurrences
    let base_day = start_date.day();

    // Start from the year of start_date
    let mut current_year = start_date.year();

    // Maximum iterations (50 years)
    let max_iterations = 50;
    let mut iterations = 0;

    while iterations < max_iterations {
        iterations += 1;

        // Generate occurrences for each selected month
        for &month in &months {
            if !(1..=12).contains(&month) {
                continue;
            }

            // Get the last day of this month
            let days_in_month = get_days_in_month(current_year, month);
            let actual_day = base_day.min(days_in_month);

            if let Some(occurrence_date) = NaiveDate::from_ymd_opt(current_year, month, actual_day)
            {
                // Must be >= start_date and <= end_date
                if occurrence_date >= start_date && occurrence_date <= end_date {
                    occurrences.push(PaymentOccurrence {
                        payment_id: payment.id,
                        description: payment.description.clone(),
                        amount: payment.amount,
                        occurrence_date: occurrence_date.format("%Y-%m-%d").to_string(),
                        payer_id: payment.payer_id,
                        is_recurring: true,
                        receiver_account_id: payment.receiver_account_id,
                    });
                }
            }
        }

        // Move to next year
        current_year += 1;

        // Check if we've passed end_date
        if let Some(jan_first) = NaiveDate::from_ymd_opt(current_year, 1, 1) {
            if jan_first > end_date {
                break;
            }
        } else {
            break;
        }
    }

    // Sort by date
    occurrences.sort_by(|a, b| a.occurrence_date.cmp(&b.occurrence_date));
    occurrences
}

/// Get the number of days in a month
fn get_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29 // Leap year
            } else {
                28
            }
        }
        _ => 30, // Invalid month, default to 30
    }
}

/// Calculate effective interval for "X times per period"
fn calculate_times_per_interval(
    recurrence_type: &str,
    interval: u32,
    times_per: u32,
) -> (u32, String) {
    if times_per == 0 {
        return (interval, recurrence_type.to_string());
    }

    // Convert "3 times per month" to "every ~10 days"
    match recurrence_type {
        "daily" => {
            // X times per day doesn't make sense, treat as every day
            (1, "daily".to_string())
        }
        "weekly" => {
            // X times per week = every (7 / X) days
            let days = (7 * interval) / times_per;
            (days.max(1), "daily".to_string())
        }
        "monthly" => {
            // X times per month = every (30 / X) days approximately
            let days = (30 * interval) / times_per;
            (days.max(1), "daily".to_string())
        }
        "yearly" => {
            // X times per year = every (365 / X) days approximately
            let days = (365 * interval) / times_per;
            (days.max(1), "daily".to_string())
        }
        _ => (interval, recurrence_type.to_string()),
    }
}

/// Add interval to a date
fn add_interval(date: NaiveDate, recurrence_type: &str, interval: u32) -> Option<NaiveDate> {
    match recurrence_type {
        "daily" => date.checked_add_days(chrono::Days::new(interval as u64)),
        "weekly" => date.checked_add_days(chrono::Days::new((interval * 7) as u64)),
        "monthly" => date.checked_add_months(Months::new(interval)),
        "yearly" => date.checked_add_months(Months::new(interval * 12)),
        _ => None,
    }
}

/// Parse date string to NaiveDate
fn parse_date(date_str: &str) -> Option<NaiveDate> {
    // Try common formats
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S").ok())
}

fn calculate_settlements(
    balances: &[ParticipantBalance],
    participant_map: &HashMap<i64, String>,
    pool_participants: &std::collections::HashSet<i64>,
) -> Vec<Debt> {
    let mut settlements = Vec::new();

    // Separate into debtors (negative balance) and creditors (positive balance)
    // Exclude pool accounts from settlements
    let mut debtors: Vec<(i64, f64)> = balances
        .iter()
        .filter(|b| b.net_balance < -0.01 && !pool_participants.contains(&b.participant_id))
        .map(|b| (b.participant_id, -b.net_balance)) // Convert to positive amount owed
        .collect();

    let mut creditors: Vec<(i64, f64)> = balances
        .iter()
        .filter(|b| b.net_balance > 0.01 && !pool_participants.contains(&b.participant_id))
        .map(|b| (b.participant_id, b.net_balance))
        .collect();

    // Greedy matching: match largest debtor with largest creditor
    debtors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    creditors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut d_idx = 0;
    let mut c_idx = 0;

    while d_idx < debtors.len() && c_idx < creditors.len() {
        let (debtor_id, debtor_amount) = &mut debtors[d_idx];
        let (creditor_id, creditor_amount) = &mut creditors[c_idx];

        let transfer = debtor_amount.min(*creditor_amount);

        if transfer > 0.01 {
            settlements.push(Debt {
                from_participant_id: *debtor_id,
                from_participant_name: participant_map.get(debtor_id).cloned().unwrap_or_default(),
                to_participant_id: *creditor_id,
                to_participant_name: participant_map
                    .get(creditor_id)
                    .cloned()
                    .unwrap_or_default(),
                amount: (transfer * 100.0).round() / 100.0,
            });
        }

        *debtor_amount -= transfer;
        *creditor_amount -= transfer;

        if *debtor_amount < 0.01 {
            d_idx += 1;
        }
        if *creditor_amount < 0.01 {
            c_idx += 1;
        }
    }

    settlements
}

/// Calculate direct-only settlements based on pairwise relationships
/// Shows ALL pairwise debts directly without intermediary optimization
/// This means if A owes B based on their direct transactions, A pays B directly,
/// even if A has a positive overall balance
fn calculate_direct_settlements(
    pairwise_balances: &[PairwiseBalance],
    _balances: &[ParticipantBalance],
    pool_participants: &std::collections::HashSet<i64>,
) -> Vec<Debt> {
    let mut settlements = Vec::new();

    for pw in pairwise_balances {
        // Skip if either participant is a pool account
        if pool_participants.contains(&pw.participant_id)
            || pool_participants.contains(&pw.other_participant_id)
        {
            continue;
        }

        // Only process each unique pair once (avoid duplicates)
        // Since pairwise_balances contains both (A, B) and (B, A),
        // we only process when participant_id < other_participant_id
        if pw.participant_id >= pw.other_participant_id {
            continue;
        }

        // Create settlement based on pairwise net debt
        // pw.net = amount_paid_for - amount_owed_by
        // If net > 0, other owes this participant
        // If net < 0, this participant owes other
        //
        // In direct-only mode, we show ALL pairwise debts without filtering
        // by overall balance. This shows the "natural" flow of money based
        // on who actually paid for whom.

        if pw.net > 0.01 {
            // other_participant owes participant
            settlements.push(Debt {
                from_participant_id: pw.other_participant_id,
                from_participant_name: pw.other_participant_name.clone(),
                to_participant_id: pw.participant_id,
                to_participant_name: pw.participant_name.clone(),
                amount: (pw.net * 100.0).round() / 100.0,
            });
        } else if pw.net < -0.01 {
            // participant owes other_participant
            settlements.push(Debt {
                from_participant_id: pw.participant_id,
                from_participant_name: pw.participant_name.clone(),
                to_participant_id: pw.other_participant_id,
                to_participant_name: pw.other_participant_name.clone(),
                amount: ((-pw.net) * 100.0).round() / 100.0,
            });
        }
    }

    // Sort by amount descending for consistency
    settlements.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    settlements
}

/// Calculate cashflow projection over a future time horizon
pub async fn calculate_cashflow_projection(
    pool: &SqlitePool,
    project_id: i64,
    horizon_months: u32,
    recommendation_mode: &str,
    frequency_type: &str,
    frequency_interval: i32,
    consolidate_mode: bool,
) -> AppResult<CashflowProjection> {
    let today = chrono::Utc::now().date_naive();
    let end_date = today + Months::new(horizon_months);

    // Get all participants
    let participants: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, account_type FROM participants WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(pool)
            .await?;

    let participant_map: HashMap<i64, String> = participants
        .iter()
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();

    let pool_participants: std::collections::HashSet<i64> = participants
        .iter()
        .filter(|(_, _, account_type)| account_type == "pool")
        .map(|(id, _, _)| *id)
        .collect();

    // Get all payments
    let payments: Vec<Payment> = sqlx::query_as("SELECT * FROM payments WHERE project_id = ?")
        .bind(project_id)
        .fetch_all(pool)
        .await?;

    // Find the earliest payment date to use as start of timeline
    // If no payments, use today
    let start_date = payments
        .iter()
        .filter_map(|p| parse_date(&p.payment_date))
        .min()
        .unwrap_or(today);

    // Identify recurring payments to pool accounts (for consolidation)
    let mut payments_to_consolidate: Vec<RecurringPaymentToConsolidate> = Vec::new();
    let mut payment_ids_to_exclude = std::collections::HashSet::new();

    if consolidate_mode {
        for payment in &payments {
            // Check if this is a recurring payment to a pool account
            if payment.is_recurring
                && payment.receiver_account_id.is_some()
                && pool_participants.contains(&payment.receiver_account_id.unwrap())
            {
                let payer_id = payment.payer_id.unwrap_or(0);
                let payer_name = participant_map.get(&payer_id).cloned().unwrap_or_default();

                payments_to_consolidate.push(RecurringPaymentToConsolidate {
                    payment_id: payment.id,
                    description: payment.description.clone(),
                    amount: payment.amount,
                    payer_id,
                    payer_name,
                    recurrence_type: payment.recurrence_type.clone().unwrap_or_default(),
                    recurrence_interval: payment.recurrence_interval.unwrap_or(1),
                });

                payment_ids_to_exclude.insert(payment.id);
            }
        }
    }

    // Get contributions
    let contributions: Vec<(i64, i64, f64)> = sqlx::query_as(
        "SELECT c.payment_id, c.participant_id, c.amount
         FROM contributions c
         JOIN payments p ON c.payment_id = p.id
         WHERE p.project_id = ?",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // Build contribution map
    let mut contribution_map: HashMap<i64, Vec<(i64, f64)>> = HashMap::new();
    for (payment_id, participant_id, amount) in contributions {
        contribution_map
            .entry(payment_id)
            .or_default()
            .push((participant_id, amount));
    }

    // Generate all payment occurrences up to end_date
    // In consolidate mode, exclude recurring pool contributions
    let mut all_occurrences: Vec<PaymentOccurrence> = Vec::new();
    for payment in &payments {
        if consolidate_mode && payment_ids_to_exclude.contains(&payment.id) {
            continue; // Skip this payment in consolidate mode
        }
        let occurrences = generate_payment_occurrences(payment, end_date);
        all_occurrences.extend(occurrences);
    }

    // Generate list of months
    let mut months = Vec::new();
    let mut current = NaiveDate::from_ymd_opt(start_date.year(), start_date.month(), 1).unwrap();
    let end_month = NaiveDate::from_ymd_opt(end_date.year(), end_date.month(), 1).unwrap();

    while current <= end_month {
        months.push(format!("{}-{:02}", current.year(), current.month()));
        current = current + Months::new(1);
    }

    // Calculate monthly balances
    let mut monthly_balances = Vec::new();

    // For each participant (excluding pools), track cumulative balance per month
    for (participant_id, participant_name, account_type) in &participants {
        if account_type == "pool" {
            continue; // Skip pool accounts
        }

        let mut cumulative_paid = 0.0;
        let mut cumulative_owed = 0.0;

        for month in &months {
            // Get all occurrences in this month
            let month_start = parse_date(&format!("{}-01", month)).unwrap();
            let month_end = month_start + Months::new(1);

            for occurrence in &all_occurrences {
                let occ_date = match parse_date(&occurrence.occurrence_date) {
                    Some(d) => d,
                    None => continue,
                };

                if occ_date < month_start || occ_date >= month_end {
                    continue; // Not in this month
                }

                // Check if this is a pool transfer (should be excluded from settlements)
                let payer_is_pool = occurrence
                    .payer_id
                    .map(|id| pool_participants.contains(&id))
                    .unwrap_or(false);
                let receiver_is_pool = occurrence
                    .receiver_account_id
                    .map(|id| pool_participants.contains(&id))
                    .unwrap_or(false);

                if payer_is_pool || receiver_is_pool {
                    // Pool transfer - skip for settlement calculations
                    continue;
                }

                // Handle user-to-user transfers
                if let Some(receiver_id) = occurrence.receiver_account_id {
                    // This is an internal transfer between users
                    if occurrence.payer_id == Some(*participant_id) {
                        cumulative_paid += occurrence.amount;
                    }
                    if receiver_id == *participant_id {
                        cumulative_owed += occurrence.amount;
                    }
                    continue;
                }

                // External expense
                if occurrence.payer_id == Some(*participant_id) {
                    cumulative_paid += occurrence.amount;
                }

                // Check if this participant contributed to this payment
                if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                    for (contrib_participant_id, contrib_amount) in contribs {
                        if contrib_participant_id == participant_id {
                            cumulative_owed += contrib_amount;
                        }
                    }
                }
            }

            let net_balance = cumulative_paid - cumulative_owed;
            monthly_balances.push(MonthlyBalance {
                month: month.clone(),
                participant_id: *participant_id,
                participant_name: participant_name.clone(),
                net_balance,
            });
        }
    }

    // Calculate recommendations
    let mut recommendations = Vec::new();

    for (participant_id, participant_name, account_type) in &participants {
        if account_type == "pool" {
            continue; // No recommendations for pool accounts
        }

        // Get this participant's monthly balances
        let participant_balances: Vec<f64> = monthly_balances
            .iter()
            .filter(|mb| mb.participant_id == *participant_id)
            .map(|mb| mb.net_balance)
            .collect();

        if participant_balances.is_empty() {
            continue;
        }

        let monthly_trend = if recommendation_mode == "linear_regression" {
            // Linear regression: calculate slope
            calculate_linear_regression_slope(&participant_balances)
        } else {
            // Simple average: (last - first) / num_months
            let first = participant_balances.first().copied().unwrap_or(0.0);
            let last = participant_balances.last().copied().unwrap_or(0.0);
            let num_months = participant_balances.len() as f64;
            if num_months > 0.0 {
                (last - first) / num_months
            } else {
                0.0
            }
        };

        // Convert monthly trend to desired frequency
        let recommended_amount =
            convert_monthly_to_frequency(-monthly_trend, frequency_type, frequency_interval);

        recommendations.push(RecurringContributionRecommendation {
            participant_id: *participant_id,
            participant_name: participant_name.clone(),
            recommended_amount,
            frequency_type: frequency_type.to_string(),
            frequency_interval,
            current_trend: monthly_trend,
            calculation_method: recommendation_mode.to_string(),
        });
    }

    // Calculate pool evolution
    let mut pool_evolutions = Vec::new();

    for (pool_id, pool_name, account_type) in &participants {
        if account_type != "pool" {
            continue;
        }

        let mut pool_monthly_balances = Vec::new();

        for month in &months {
            let month_start = parse_date(&format!("{}-01", month)).unwrap();
            let month_end = month_start + Months::new(1);

            // Calculate ownership for each participant at end of this month
            let mut ownership_map: HashMap<i64, (f64, f64)> = HashMap::new(); // (contributed, consumed)

            for occurrence in &all_occurrences {
                let occ_date = match parse_date(&occurrence.occurrence_date) {
                    Some(d) => d,
                    None => continue,
                };

                // Only include occurrences up to end of this month
                if occ_date >= month_end {
                    continue;
                }

                let payer_id = match occurrence.payer_id {
                    Some(id) => id,
                    None => continue,
                };

                // Pool transfer detection
                let payer_is_pool = payer_id == *pool_id;
                let receiver_is_pool = occurrence.receiver_account_id == Some(*pool_id);

                if receiver_is_pool {
                    // User deposited to pool
                    let entry = ownership_map.entry(payer_id).or_insert((0.0, 0.0));
                    entry.0 += occurrence.amount; // contributed
                } else if payer_is_pool {
                    // Pool paid (withdrawal or expense)
                    if let Some(receiver_id) = occurrence.receiver_account_id {
                        // Transfer from pool to user
                        let entry = ownership_map.entry(receiver_id).or_insert((0.0, 0.0));
                        entry.1 += occurrence.amount; // consumed
                    } else {
                        // Pool paid external expense - split among contributors
                        if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                            for (contrib_id, contrib_amount) in contribs {
                                let entry = ownership_map.entry(*contrib_id).or_insert((0.0, 0.0));
                                entry.1 += contrib_amount; // consumed
                            }
                        }
                    }
                }
            }

            // Build ownership snapshots
            let mut participant_ownerships: Vec<PoolOwnershipSnapshot> = participants
                .iter()
                .filter(|(id, _, acct_type)| *id != *pool_id && acct_type != "pool")
                .filter_map(|(id, name, _)| {
                    let (contributed, consumed) =
                        ownership_map.get(id).copied().unwrap_or((0.0, 0.0));
                    let ownership = contributed - consumed;
                    if ownership.abs() > 0.01 {
                        Some(PoolOwnershipSnapshot {
                            participant_id: *id,
                            participant_name: name.clone(),
                            ownership,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            participant_ownerships.sort_by(|a, b| {
                b.ownership
                    .partial_cmp(&a.ownership)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let total_balance: f64 = participant_ownerships.iter().map(|p| p.ownership).sum();

            pool_monthly_balances.push(PoolMonthlyBalance {
                month: month.clone(),
                total_balance,
                participant_ownerships,
            });
        }

        pool_evolutions.push(PoolEvolution {
            pool_id: *pool_id,
            pool_name: pool_name.clone(),
            monthly_balances: pool_monthly_balances,
        });
    }

    Ok(CashflowProjection {
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        months,
        monthly_balances,
        recommendations,
        pool_evolutions,
        consolidate_mode,
        payments_to_consolidate,
    })
}

/// Convert monthly amount to desired frequency
fn convert_monthly_to_frequency(monthly_amount: f64, freq_type: &str, freq_interval: i32) -> f64 {
    let base_amount = match freq_type {
        "weekly" => monthly_amount / 4.33,
        "monthly" => monthly_amount,
        "yearly" => monthly_amount * 12.0,
        "daily" => monthly_amount / 30.0,
        _ => monthly_amount,
    };
    base_amount * (freq_interval as f64)
}

/// Calculate slope using linear regression
fn calculate_linear_regression_slope(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    if n < 2.0 {
        return 0.0;
    }

    let x_mean = (n - 1.0) / 2.0; // Mean of indices 0, 1, 2, ..., n-1
    let y_mean = values.iter().sum::<f64>() / n;

    let mut numerator = 0.0;
    let mut denominator = 0.0;

    for (i, &y) in values.iter().enumerate() {
        let x = i as f64;
        numerator += (x - x_mean) * (y - y_mean);
        denominator += (x - x_mean).powi(2);
    }

    if denominator.abs() < 0.0001 {
        return 0.0;
    }

    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_settlement_two_participants() {
        // Carl (id=1) pays $100, David (id=2) is the sole contributor
        // Result: David owes Carl $100
        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 100.0,
                total_owed: 0.0,
                net_balance: 100.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
        ];

        let participant_map: HashMap<i64, String> =
            vec![(1, "Carl".to_string()), (2, "David".to_string())]
                .into_iter()
                .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].from_participant_id, 2); // David
        assert_eq!(settlements[0].to_participant_id, 1); // Carl
        assert_eq!(settlements[0].amount, 100.0);
    }

    #[test]
    fn test_settlement_with_prior_even_balance() {
        // Carl and David were even at $1000 each
        // Carl now pays additional $100 for David only
        // Result: David owes Carl $100
        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 1100.0,
                total_owed: 1000.0,
                net_balance: 100.0, // 1100 - 1000 = +100
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 1000.0,
                total_owed: 1100.0,
                net_balance: -100.0, // 1000 - 1100 = -100
            },
        ];

        let participant_map: HashMap<i64, String> =
            vec![(1, "Carl".to_string()), (2, "David".to_string())]
                .into_iter()
                .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].from_participant_id, 2); // David
        assert_eq!(settlements[0].to_participant_id, 1); // Carl
        assert_eq!(settlements[0].amount, 100.0);
    }

    #[test]
    fn test_settlement_three_participants() {
        // A paid 300 for A, B, C equally (each owes 100)
        // A: paid 300, owes 100, net = +200
        // B: paid 0, owes 100, net = -100
        // C: paid 0, owes 100, net = -100
        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "A".to_string(),
                total_paid: 300.0,
                total_owed: 100.0,
                net_balance: 200.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "B".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "C".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "A".to_string()),
            (2, "B".to_string()),
            (3, "C".to_string()),
        ]
        .into_iter()
        .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // Should have 2 settlements: B->A $100, C->A $100
        assert_eq!(settlements.len(), 2);

        let total: f64 = settlements.iter().map(|s| s.amount).sum();
        assert_eq!(total, 200.0);

        // All settlements should go to A (id=1)
        for s in &settlements {
            assert_eq!(s.to_participant_id, 1);
        }
    }

    #[test]
    fn test_direct_settlement_simple() {
        // Carl (id=1) pays $100 for David (id=2)
        // Pairwise: Carl paid $100 for David, David paid $0 for Carl
        // Net: Carl is owed $100 by David
        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 100.0,
                total_owed: 0.0,
                net_balance: 100.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
        ];

        let pairwise_balances = vec![
            PairwiseBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                other_participant_id: 2,
                other_participant_name: "David".to_string(),
                amount_paid_for: 100.0,
                amount_owed_by: 0.0,
                net: 100.0, // David owes Carl
                paid_for_breakdown: vec![],
                owed_by_breakdown: vec![],
            },
            PairwiseBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                other_participant_id: 1,
                other_participant_name: "Carl".to_string(),
                amount_paid_for: 0.0,
                amount_owed_by: 100.0,
                net: -100.0, // David owes Carl
                paid_for_breakdown: vec![],
                owed_by_breakdown: vec![],
            },
        ];

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements =
            calculate_direct_settlements(&pairwise_balances, &balances, &pool_participants);

        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].from_participant_id, 2); // David
        assert_eq!(settlements[0].to_participant_id, 1); // Carl
        assert_eq!(settlements[0].amount, 100.0);
    }

    // =====================================================
    // Internal Transfer Tests (receiver_account_id IS NOT NULL)
    // =====================================================

    #[test]
    fn test_pool_transfer_excluded_from_settlements() {
        // Scenario: Carl transfers $100 to pool (pool transfer)
        // This should NOT affect global settlements - only pool ownership
        //
        // When receiver_account_id is a pool, the occurrence is skipped
        // in the paid/owed calculations, so balances remain at 0

        // Simulate: After pool transfer is processed
        // Since pool transfers are skipped, balances are unaffected
        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 0.0, // Pool transfer doesn't count as "paid"
                total_owed: 0.0,
                net_balance: 0.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "Pool".to_string(),
                total_paid: 0.0,
                total_owed: 0.0,
                net_balance: 0.0,
            },
        ];

        let participant_map: HashMap<i64, String> =
            vec![(1, "Carl".to_string()), (2, "Pool".to_string())]
                .into_iter()
                .collect();

        let pool_participants: std::collections::HashSet<i64> = [2].into_iter().collect();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // No settlements because balances are all 0 (pool transfers don't affect settlements)
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn test_user_to_user_transfer_settles_debt() {
        // Full scenario:
        // 1. David pays $200 groceries, split between Carl and David ($100 each)
        //    - David: paid $200, owes $100, net = +$100
        //    - Carl: paid $0, owes $100, net = -$100
        //    Settlement: Carl owes David $100
        //
        // 2. Carl transfers $100 to David (pay back)
        //    - Carl's paid += $100, David's owed += $100
        //
        // Final totals:
        //    - David: paid $200, owes $200, net = 0
        //    - Carl: paid $100, owes $100, net = 0
        //    No settlements needed!

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 100.0, // Transfer of $100
                total_owed: 100.0, // His share of groceries
                net_balance: 0.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 200.0, // Groceries
                total_owed: 200.0, // His share ($100) + received transfer ($100)
                net_balance: 0.0,
            },
        ];

        let participant_map: HashMap<i64, String> =
            vec![(1, "Carl".to_string()), (2, "David".to_string())]
                .into_iter()
                .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // No settlements - debt is settled!
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn test_user_to_user_partial_transfer() {
        // Scenario:
        // 1. David pays $200 groceries, split between Carl and David ($100 each)
        //    - Carl owes David $100
        //
        // 2. Carl transfers $50 to David (partial pay back)
        //
        // Final totals:
        //    - David: paid $200, owes $150 ($100 share + $50 received), net = +$50
        //    - Carl: paid $50, owes $100, net = -$50
        //    Settlement: Carl owes David $50

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 50.0,  // Transfer of $50
                total_owed: 100.0, // His share of groceries
                net_balance: -50.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 200.0, // Groceries
                total_owed: 150.0, // His share ($100) + received transfer ($50)
                net_balance: 50.0,
            },
        ];

        let participant_map: HashMap<i64, String> =
            vec![(1, "Carl".to_string()), (2, "David".to_string())]
                .into_iter()
                .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // Carl still owes David $50
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].from_participant_id, 1); // Carl
        assert_eq!(settlements[0].to_participant_id, 2); // David
        assert_eq!(settlements[0].amount, 50.0);
    }

    #[test]
    fn test_external_expense_with_pool_payer_creates_correct_settlements() {
        // Scenario: Pool pays $300 for insurance, split equally among Carl, David, Lise
        // This is an EXTERNAL expense (receiver_account_id = NULL)
        //
        // Expected:
        // - Carl owes $100 (to pool ownership)
        // - David owes $100 (to pool ownership)
        // - Lise owes $100 (to pool ownership)
        // - Pool is excluded from settlements (it's a pool account)
        //
        // Since pool is excluded, settlements should be empty (no inter-user transfers needed)

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 4,
                participant_name: "Pool".to_string(),
                total_paid: 300.0,
                total_owed: 0.0,
                net_balance: 300.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "Carl".to_string()),
            (2, "David".to_string()),
            (3, "Lise".to_string()),
            (4, "Pool".to_string()),
        ]
        .into_iter()
        .collect();

        // Pool is participant 4
        let pool_participants: std::collections::HashSet<i64> = [4].into_iter().collect();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // No settlements among users because the creditor (Pool) is excluded
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn test_mixed_scenario_with_pool_and_users() {
        // Scenario:
        // 1. Pool pays $300 insurance (external), split among Carl, David, Lise ($100 each)
        // 2. Carl pays $150 groceries (external), split among Carl, David, Lise ($50 each)
        //
        // IMPORTANT: Pool-paid expenses should NOT affect user balances for settlements!
        // The debt to pool is tracked separately in pool ownership.
        //
        // Corrected Balances (excluding pool-paid contributions):
        // - Carl: paid $150, owes $50 (only from Carl's groceries), net = +100
        // - David: paid $0, owes $50 (only from Carl's groceries), net = -50
        // - Lise: paid $0, owes $50 (only from Carl's groceries), net = -50
        // - Pool: paid $300, owes $0, net = +300 (excluded from settlements)
        //
        // Settlements (Pool excluded):
        // - David owes Carl $50
        // - Lise owes Carl $50

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 150.0,
                total_owed: 50.0, // Only Carl's share from his own payment
                net_balance: 100.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 50.0, // Only David's share from Carl's payment
                net_balance: -50.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 0.0,
                total_owed: 50.0, // Only Lise's share from Carl's payment
                net_balance: -50.0,
            },
            ParticipantBalance {
                participant_id: 4,
                participant_name: "Pool".to_string(),
                total_paid: 300.0,
                total_owed: 0.0,
                net_balance: 300.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "Carl".to_string()),
            (2, "David".to_string()),
            (3, "Lise".to_string()),
            (4, "Pool".to_string()),
        ]
        .into_iter()
        .collect();

        let pool_participants: std::collections::HashSet<i64> = [4].into_iter().collect();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // Carl is creditor, David and Lise are debtors
        assert_eq!(settlements.len(), 2);

        let total: f64 = settlements.iter().map(|s| s.amount).sum();
        assert_eq!(total, 100.0);

        // All settlements should go to Carl
        for s in &settlements {
            assert_eq!(s.to_participant_id, 1);
        }
    }

    #[test]
    fn test_mixed_scenario_with_user_creditor() {
        // Scenario:
        // 1. Carl pays $300 groceries (external), split among Carl, David, Lise ($100 each)
        //
        // Balances:
        // - Carl: paid $300, owes $100, net = +200
        // - David: paid $0, owes $100, net = -100
        // - Lise: paid $0, owes $100, net = -100
        //
        // Settlements:
        // - David owes Carl $100
        // - Lise owes Carl $100

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 300.0,
                total_owed: 100.0,
                net_balance: 200.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 0.0,
                total_owed: 100.0,
                net_balance: -100.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "Carl".to_string()),
            (2, "David".to_string()),
            (3, "Lise".to_string()),
        ]
        .into_iter()
        .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        assert_eq!(settlements.len(), 2);
        let total: f64 = settlements.iter().map(|s| s.amount).sum();
        assert_eq!(total, 200.0);

        // All settlements should go to Carl
        for s in &settlements {
            assert_eq!(s.to_participant_id, 1);
        }
    }

    #[test]
    fn test_large_pool_transfer_no_unrelated_participants() {
        // Scenario: Carl transfers $100000 to Pool (pool transfer)
        // Other users (David, Lise) should not be affected AT ALL
        //
        // Since pool transfers are skipped in balance calculation,
        // everyone's balance remains at 0

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 0.0, // Pool transfers don't count
                total_owed: 0.0,
                net_balance: 0.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 0.0,
                net_balance: 0.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 0.0,
                total_owed: 0.0,
                net_balance: 0.0,
            },
            ParticipantBalance {
                participant_id: 4,
                participant_name: "Pool".to_string(),
                total_paid: 0.0,
                total_owed: 0.0,
                net_balance: 0.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "Carl".to_string()),
            (2, "David".to_string()),
            (3, "Lise".to_string()),
            (4, "Pool".to_string()),
        ]
        .into_iter()
        .collect();

        let pool_participants: std::collections::HashSet<i64> = [4].into_iter().collect();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // No settlements - everyone is at 0 (pool transfers don't affect user balances)
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn test_user_to_user_transfer_doesnt_affect_unrelated_users() {
        // Scenario: Carl transfers $100 to David
        // Lise should NOT be affected at all
        //
        // This tests that user-to-user transfers only affect the two parties involved

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 100.0, // Transfer
                total_owed: 0.0,
                net_balance: 100.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 0.0,
                total_owed: 100.0, // Received transfer
                net_balance: -100.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 0.0, // Not involved
                total_owed: 0.0,
                net_balance: 0.0,
            },
        ];

        let participant_map: HashMap<i64, String> = vec![
            (1, "Carl".to_string()),
            (2, "David".to_string()),
            (3, "Lise".to_string()),
        ]
        .into_iter()
        .collect();

        let pool_participants: std::collections::HashSet<i64> = std::collections::HashSet::new();

        let settlements = calculate_settlements(&balances, &participant_map, &pool_participants);

        // Only one settlement: David owes Carl
        // Lise is not involved at all
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].from_participant_id, 2); // David
        assert_eq!(settlements[0].to_participant_id, 1); // Carl

        // Verify Lise is not in any settlement
        for s in &settlements {
            assert_ne!(s.from_participant_id, 3);
            assert_ne!(s.to_participant_id, 3);
        }
    }

    #[test]
    fn test_occurrence_with_receiver_is_internal() {
        // Test that PaymentOccurrence with receiver_account_id is recognized as internal
        let occurrence = PaymentOccurrence {
            payment_id: 1,
            description: "Transfer to pool".to_string(),
            amount: 100.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: Some(1),
            is_recurring: false,
            receiver_account_id: Some(2), // Internal transfer to pool
        };

        assert!(occurrence.receiver_account_id.is_some());
    }

    #[test]
    fn test_occurrence_without_receiver_is_external() {
        // Test that PaymentOccurrence without receiver_account_id is external
        let occurrence = PaymentOccurrence {
            payment_id: 1,
            description: "Groceries".to_string(),
            amount: 100.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: Some(1),
            is_recurring: false,
            receiver_account_id: None, // External expense
        };

        assert!(occurrence.receiver_account_id.is_none());
    }
}
