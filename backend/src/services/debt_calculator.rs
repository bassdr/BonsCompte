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
    // Dual ledger: expected minimum from affects_payer/receiver_expectation flags
    pub expected_minimum: f64,
    pub is_below_expected: bool, // total_balance < expected_minimum
    pub shortfall: Option<f64>,  // expected_minimum - total_balance (if positive)
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
    // Payment finalization status: true = final, false = draft
    pub is_final: bool,
    // Dual ledger flags for pool expected minimum
    // affects_balance: Transaction affects actual pool balance (default: true)
    pub affects_balance: bool,
    // affects_payer_expectation: When payer is a pool and true, reduces payer's expected minimum
    pub affects_payer_expectation: bool,
    // affects_receiver_expectation: When receiver is a pool and true, increases receiver's expected minimum
    pub affects_receiver_expectation: bool,
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

/// Calculate debts as of today
pub async fn calculate_debts(
    pool: &SqlitePool,
    project_id: i64,
    include_drafts: bool,
) -> AppResult<DebtSummary> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    calculate_debts_at_date(pool, project_id, &today, include_drafts).await
}

/// Calculate debts as of a specific target date
pub async fn calculate_debts_at_date(
    pool: &SqlitePool,
    project_id: i64,
    target_date: &str,
    include_drafts: bool,
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

    // Get payments for this project (optionally filtering out drafts)
    let payments: Vec<Payment> = if include_drafts {
        sqlx::query_as("SELECT * FROM payments WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as("SELECT * FROM payments WHERE project_id = ? AND is_final = 1")
            .bind(project_id)
            .fetch_all(pool)
            .await?
    };

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
            } else if occurrence.payer_id.is_none() {
                // External inflow to user: receiver holds money for the group
                // Receiver is debited (holds/owes the full amount)
                *owed_map.entry(receiver_id).or_insert(0.0) += occurrence.amount;

                // Contributors are credited (owed their share from receiver)
                if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                    for (participant_id, amount) in contribs {
                        *paid_map.entry(*participant_id).or_insert(0.0) += amount;
                    }

                    // Track pairwise: each contributor is owed by receiver
                    for (contributor_id, amount) in contribs {
                        if *contributor_id != receiver_id {
                            let entry = pairwise_map
                                .entry((*contributor_id, receiver_id))
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
        //
        // Dual ledger tracking:
        // - affects_balance=true transactions affect actual pool balance (ownership_map)
        // - affects_payer_expectation=true: when payer is a pool, reduces payer pool's expected minimum
        // - affects_receiver_expectation=true: when receiver is a pool, increases receiver pool's expected minimum

        // Track ownership amounts and transaction breakdowns for ACTUAL balance
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

        // Track expected minimum separately
        let mut expected_minimum: f64 = 0.0;

        for occurrence in &all_occurrences {
            // Determine which ledger(s) this occurrence affects
            let affects_balance = occurrence.affects_balance;

            // Calculate expected minimum delta based on separate payer/receiver flags
            // For withdrawals from this pool (payer = pool_id): affects_payer_expectation reduces expected minimum
            // For deposits to this pool (receiver = pool_id): affects_receiver_expectation increases expected minimum
            if let Some(receiver_id) = occurrence.receiver_account_id {
                // Internal transfer
                if let Some(payer_id) = occurrence.payer_id {
                    if receiver_id == pool_id && payer_id != pool_id {
                        // Transfer TO this pool: receiver_expectation affects expected min
                        if occurrence.affects_receiver_expectation {
                            expected_minimum += occurrence.amount;
                        }
                    } else if payer_id == pool_id && receiver_id != pool_id {
                        // Transfer FROM this pool: payer_expectation affects expected min
                        if occurrence.affects_payer_expectation {
                            expected_minimum -= occurrence.amount;
                        }
                    }
                } else if occurrence.payer_id.is_none() && receiver_id == pool_id {
                    // External inflow to this pool: receiver_expectation affects expected min
                    if occurrence.affects_receiver_expectation {
                        expected_minimum += occurrence.amount;
                    }
                }
            } else {
                // External expense
                if let Some(payer_id) = occurrence.payer_id {
                    if payer_id == pool_id {
                        // Pool paying external expense: payer_expectation affects expected min
                        if occurrence.affects_payer_expectation {
                            expected_minimum -= occurrence.amount;
                        }
                    }
                }
            }

            // Skip actual balance tracking if doesn't affect balance
            if !affects_balance {
                continue;
            }

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
                } else if occurrence.payer_id.is_none() && receiver_id == pool_id {
                    // External inflow to pool: each contributor's ownership increases
                    if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                        for (contributor_id, amount) in contribs {
                            if *contributor_id != pool_id {
                                let entry = ownership_map.entry(*contributor_id).or_insert((
                                    0.0,
                                    0.0,
                                    Vec::new(),
                                    Vec::new(),
                                ));
                                entry.0 += amount; // contributed (ownership increases)
                                entry.2.push(PairwisePaymentBreakdown {
                                    payment_id: occurrence.payment_id,
                                    description: occurrence.description.clone(),
                                    occurrence_date: occurrence.occurrence_date.clone(),
                                    amount: *amount,
                                });
                            }
                        }
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
        let is_below_expected = total_balance < expected_minimum;
        let shortfall = if is_below_expected {
            Some(expected_minimum - total_balance)
        } else {
            None
        };

        pool_ownerships.push(PoolOwnership {
            pool_id,
            pool_name,
            entries,
            total_balance,
            expected_minimum,
            is_below_expected,
            shortfall,
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
            is_final: payment.is_final,
            affects_balance: payment.affects_balance,
            affects_payer_expectation: payment.affects_payer_expectation,
            affects_receiver_expectation: payment.affects_receiver_expectation,
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
                        payment.recurrence_monthdays.as_deref(),
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
            is_final: payment.is_final,
            affects_balance: payment.affects_balance,
            affects_payer_expectation: payment.affects_payer_expectation,
            affects_receiver_expectation: payment.affects_receiver_expectation,
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
                    is_final: payment.is_final,
                    affects_balance: payment.affects_balance,
                    affects_payer_expectation: payment.affects_payer_expectation,
                    affects_receiver_expectation: payment.affects_receiver_expectation,
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
                        is_final: payment.is_final,
                        affects_balance: payment.affects_balance,
                        affects_payer_expectation: payment.affects_payer_expectation,
                        affects_receiver_expectation: payment.affects_receiver_expectation,
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

/// Generate yearly occurrences in specific months (and optionally specific days)
/// months_json format: [1, 6, 12] - array of month numbers (1-12)
/// monthdays_json format: [1, 15] - array of day numbers (1-31), optional
fn generate_yearly_with_months(
    payment: &Payment,
    start_date: NaiveDate,
    end_date: NaiveDate,
    months_json: &str,
    monthdays_json: Option<&str>,
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

    // Parse monthdays JSON if provided, otherwise use start_date's day
    let monthdays: Vec<u32> = if let Some(json) = monthdays_json {
        match serde_json::from_str(json) {
            Ok(d) => d,
            Err(_) => vec![start_date.day()],
        }
    } else {
        vec![start_date.day()]
    };

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

            // Generate occurrences for each selected day
            for &day in &monthdays {
                let actual_day = day.min(days_in_month);

                if let Some(occurrence_date) =
                    NaiveDate::from_ymd_opt(current_year, month, actual_day)
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
                            is_final: payment.is_final,
                            affects_balance: payment.affects_balance,
                            affects_payer_expectation: payment.affects_payer_expectation,
                            affects_receiver_expectation: payment.affects_receiver_expectation,
                        });
                    }
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
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: false,
            affects_receiver_expectation: false,
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
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: false,
            affects_receiver_expectation: false,
        };

        assert!(occurrence.receiver_account_id.is_none());
    }

    #[test]
    fn test_occurrence_without_payer_is_external_inflow() {
        // Test that PaymentOccurrence without payer_id but with receiver_account_id
        // is recognized as an external inflow
        let occurrence = PaymentOccurrence {
            payment_id: 1,
            description: "Bank interest deposit".to_string(),
            amount: 300.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: None, // External inflow - no payer
            is_recurring: false,
            receiver_account_id: Some(4), // Goes to pool account
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: false,
            affects_receiver_expectation: false,
        };

        assert!(occurrence.payer_id.is_none());
        assert!(occurrence.receiver_account_id.is_some());
    }

    #[test]
    fn test_external_inflow_to_user_creates_settlements() {
        // Scenario: External inflow of $300 to Carl, split among Carl, David, Lise
        // Carl receives $300 from outside (e.g., landlord refund)
        // Split equally: Carl $100, David $100, Lise $100
        //
        // Result:
        // - Carl: holds $300 (owed), entitled to $100 (paid), net = -200
        // - David: entitled to $100 (paid), owes $0, net = +100
        // - Lise: entitled to $100 (paid), owes $0, net = +100
        //
        // Settlements:
        // - Carl owes David $100
        // - Carl owes Lise $100

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 100.0, // His share (credited)
                total_owed: 300.0, // Holds full amount (debited)
                net_balance: -200.0,
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "David".to_string(),
                total_paid: 100.0, // His share (credited)
                total_owed: 0.0,
                net_balance: 100.0,
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "Lise".to_string(),
                total_paid: 100.0, // Her share (credited)
                total_owed: 0.0,
                net_balance: 100.0,
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

        // Should have 2 settlements: Carl→David and Carl→Lise
        assert_eq!(settlements.len(), 2);

        // Total amount should be $200 (Carl pays $100 to David + $100 to Lise)
        let total: f64 = settlements.iter().map(|s| s.amount).sum();
        assert!((total - 200.0).abs() < 0.01);

        // All settlements should come from Carl (id=1)
        for s in &settlements {
            assert_eq!(s.from_participant_id, 1);
        }
    }

    #[test]
    fn test_external_inflow_to_pool_no_settlements() {
        // Scenario: External inflow of $300 to pool, split among Carl, David, Lise
        // (e.g., government grant for the cooperative)
        //
        // Result: Each person's pool ownership increases by $100
        // No effect on user-to-user settlements (everyone's balance stays 0)

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "Carl".to_string(),
                total_paid: 0.0,
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

        // No settlements - external inflow to pool doesn't affect user-to-user settlements
        assert_eq!(settlements.len(), 0);
    }

    #[test]
    fn test_external_inflow_bank_refund_split_three_ways() {
        // Scenario: Bank gives back $1000 to user A (external fund to A)
        // This $1000 must be split equally among 3 contributors: A, B, C
        // Each contributor has weight=1, so each gets $333.33
        //
        // Expected behavior:
        // - A receives $1000 from outside (external inflow)
        // - A holds this money for the group
        // - A is debited $1000 (owed_map: holds money that's not all theirs)
        // - Each contributor is credited their share (paid_map):
        //   - A: $333.33 (their share)
        //   - B: $333.33 (their share)
        //   - C: $333.33 (their share)
        //
        // Net balances:
        // - A: paid $333.33, owed $1000 → net = -$666.67 (owes money)
        // - B: paid $333.33, owed $0 → net = +$333.33 (is owed money)
        // - C: paid $333.33, owed $0 → net = +$333.33 (is owed money)
        //
        // Settlements:
        // - A owes B $333.33
        // - A owes C $333.33

        let amount_per_person = 1000.0 / 3.0; // 333.33...

        let balances = vec![
            ParticipantBalance {
                participant_id: 1,
                participant_name: "A".to_string(),
                total_paid: amount_per_person, // A's share (credited)
                total_owed: 1000.0,            // A holds full amount (debited)
                net_balance: amount_per_person - 1000.0, // -666.67
            },
            ParticipantBalance {
                participant_id: 2,
                participant_name: "B".to_string(),
                total_paid: amount_per_person, // B's share (credited)
                total_owed: 0.0,
                net_balance: amount_per_person, // +333.33
            },
            ParticipantBalance {
                participant_id: 3,
                participant_name: "C".to_string(),
                total_paid: amount_per_person, // C's share (credited)
                total_owed: 0.0,
                net_balance: amount_per_person, // +333.33
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

        // Should have 2 settlements: A→B and A→C
        assert_eq!(settlements.len(), 2);

        // Total amount should be ~$666.67 (A pays $333.33 to B + $333.33 to C)
        let total: f64 = settlements.iter().map(|s| s.amount).sum();
        assert!((total - 2.0 * amount_per_person).abs() < 0.01);

        // All settlements should come FROM A (id=1)
        for s in &settlements {
            assert_eq!(s.from_participant_id, 1, "A should be the one paying");
        }

        // Verify B and C each receive ~$333.33
        let b_settlement = settlements.iter().find(|s| s.to_participant_id == 2);
        let c_settlement = settlements.iter().find(|s| s.to_participant_id == 3);

        assert!(b_settlement.is_some(), "B should receive a settlement");
        assert!(c_settlement.is_some(), "C should receive a settlement");

        assert!(
            (b_settlement.unwrap().amount - amount_per_person).abs() < 0.01,
            "B should receive ~$333.33"
        );
        assert!(
            (c_settlement.unwrap().amount - amount_per_person).abs() < 0.01,
            "C should receive ~$333.33"
        );
    }

    #[test]
    fn test_pool_ownership_dual_ledger_fields() {
        // Test that PoolOwnership struct correctly tracks expected_minimum and related fields
        let entries = vec![
            PoolOwnershipEntry {
                participant_id: 1,
                participant_name: "Alice".to_string(),
                contributed: 3000.0,
                consumed: 500.0,
                ownership: 2500.0,
                contributed_breakdown: vec![],
                consumed_breakdown: vec![],
            },
            PoolOwnershipEntry {
                participant_id: 2,
                participant_name: "Bob".to_string(),
                contributed: 2000.0,
                consumed: 1000.0,
                ownership: 1000.0,
                contributed_breakdown: vec![],
                consumed_breakdown: vec![],
            },
        ];

        let total_balance = entries.iter().map(|e| e.ownership).sum::<f64>();
        let expected_minimum = 5000.0; // Set a $5000 reserve requirement

        let is_below_expected = total_balance < expected_minimum;
        let shortfall = if is_below_expected {
            Some(expected_minimum - total_balance)
        } else {
            None
        };

        let pool_ownership = PoolOwnership {
            pool_id: 1,
            pool_name: "Shared Account".to_string(),
            entries,
            total_balance,
            expected_minimum,
            is_below_expected,
            shortfall,
        };

        // total_balance = 2500 + 1000 = 3500
        // expected_minimum = 5000
        // shortfall = 5000 - 3500 = 1500
        assert_eq!(pool_ownership.total_balance, 3500.0);
        assert_eq!(pool_ownership.expected_minimum, 5000.0);
        assert!(pool_ownership.is_below_expected);
        assert_eq!(pool_ownership.shortfall, Some(1500.0));
    }

    #[test]
    fn test_pool_ownership_no_shortfall_when_above_expected() {
        // Test when balance is above expected minimum
        let entries = vec![PoolOwnershipEntry {
            participant_id: 1,
            participant_name: "Alice".to_string(),
            contributed: 6000.0,
            consumed: 500.0,
            ownership: 5500.0,
            contributed_breakdown: vec![],
            consumed_breakdown: vec![],
        }];

        let total_balance = 5500.0;
        let expected_minimum = 5000.0;

        let is_below_expected = total_balance < expected_minimum;
        let shortfall = if is_below_expected {
            Some(expected_minimum - total_balance)
        } else {
            None
        };

        let pool_ownership = PoolOwnership {
            pool_id: 1,
            pool_name: "Shared Account".to_string(),
            entries,
            total_balance,
            expected_minimum,
            is_below_expected,
            shortfall,
        };

        // Balance $5500 > expected $5000, no shortfall
        assert!(!pool_ownership.is_below_expected);
        assert!(pool_ownership.shortfall.is_none());
    }

    #[test]
    fn test_occurrence_affects_balance_only() {
        // Test occurrence that only affects balance (normal transaction)
        let occurrence = PaymentOccurrence {
            payment_id: 1,
            description: "Regular deposit".to_string(),
            amount: 1000.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: Some(1),
            is_recurring: false,
            receiver_account_id: Some(2),
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: false,
            affects_receiver_expectation: false,
        };

        assert!(occurrence.affects_balance);
        assert!(!occurrence.affects_payer_expectation);
        assert!(!occurrence.affects_receiver_expectation);
    }

    #[test]
    fn test_occurrence_affects_receiver_expectation_only() {
        // Test occurrence that only affects receiver expectation (rule-only transaction)
        // This is a "Rule" - sets expected minimum on pool without moving money
        let occurrence = PaymentOccurrence {
            payment_id: 2,
            description: "Set $5000 reserve".to_string(),
            amount: 5000.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: None,
            is_recurring: false,
            receiver_account_id: Some(2), // Pool receives the rule
            is_final: true,
            affects_balance: false,
            affects_payer_expectation: false,
            affects_receiver_expectation: true, // Increases pool's expected minimum
        };

        assert!(!occurrence.affects_balance);
        assert!(occurrence.affects_receiver_expectation);
    }

    #[test]
    fn test_occurrence_earmarked_deposit() {
        // Test earmarked deposit - affects both balance and receiver expectation
        let occurrence = PaymentOccurrence {
            payment_id: 3,
            description: "Earmarked deposit".to_string(),
            amount: 2000.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: Some(1),
            is_recurring: false,
            receiver_account_id: Some(2), // Pool receives the deposit
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: false,
            affects_receiver_expectation: true, // Earmarked: increases pool's expected minimum
        };

        assert!(occurrence.affects_balance);
        assert!(occurrence.affects_receiver_expectation);
    }

    #[test]
    fn test_occurrence_approved_withdrawal() {
        // Test approved withdrawal - affects both balance and payer expectation
        let occurrence = PaymentOccurrence {
            payment_id: 4,
            description: "Approved renovation expense".to_string(),
            amount: 3000.0,
            occurrence_date: "2024-01-01".to_string(),
            payer_id: Some(2), // Pool pays
            is_recurring: false,
            receiver_account_id: Some(1), // User receives (internal transfer)
            is_final: true,
            affects_balance: true,
            affects_payer_expectation: true, // Approved: reduces pool's expected minimum
            affects_receiver_expectation: false,
        };

        assert!(occurrence.affects_balance);
        assert!(occurrence.affects_payer_expectation);
        assert!(!occurrence.affects_receiver_expectation);
    }
}
