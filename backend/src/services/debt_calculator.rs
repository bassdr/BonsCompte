use chrono::{NaiveDate, Months};
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
    pub amount_paid_for: f64,    // Amount this participant paid for other
    pub amount_owed_by: f64,     // Amount other paid for this participant
    pub net: f64,                // paid_for - owed_by (positive = they owe you)
    pub paid_for_breakdown: Vec<PairwisePaymentBreakdown>,  // Details of what we paid for them
    pub owed_by_breakdown: Vec<PairwisePaymentBreakdown>,   // Details of what they paid for us
}

#[derive(Debug, Serialize)]
pub struct PoolOwnershipEntry {
    pub participant_id: i64,
    pub participant_name: String,
    pub contributed: f64,     // Total deposited to pool
    pub consumed: f64,        // Total share of pool-paid expenses
    pub ownership: f64,       // contributed - consumed
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
}

#[derive(Debug, Serialize)]
pub struct DebtSummary {
    pub balances: Vec<ParticipantBalance>,
    pub settlements: Vec<Debt>,
    pub target_date: String,
    pub occurrences: Vec<PaymentOccurrence>,
    pub pairwise_balances: Vec<PairwiseBalance>,
    pub pool_ownership: Option<PoolOwnership>,
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
    let target = parse_date(target_date).unwrap_or_else(|| {
        chrono::Utc::now().date_naive()
    });

    // Get all participants for this project (including account_type)
    let participants: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, name, account_type FROM participants WHERE project_id = ?"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let participant_map: HashMap<i64, String> = participants.iter()
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();

    // Track which participants are pool accounts (excluded from settlements)
    let pool_participants: std::collections::HashSet<i64> = participants.iter()
        .filter(|(_, _, account_type)| account_type == "pool")
        .map(|(id, _, _)| *id)
        .collect();

    // Get all payments for this project
    let payments: Vec<Payment> = sqlx::query_as(
        "SELECT * FROM payments WHERE project_id = ?"
    )
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
         WHERE p.project_id = ?"
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
    let mut paid_map: HashMap<i64, f64> = HashMap::new();
    let mut owed_map: HashMap<i64, f64> = HashMap::new();
    let mut pairwise_map: HashMap<(i64, i64), (f64, Vec<PairwisePaymentBreakdown>)> = HashMap::new();

    for occurrence in &all_occurrences {
        // Add to paid total for payer
        if let Some(payer_id) = occurrence.payer_id {
            *paid_map.entry(payer_id).or_insert(0.0) += occurrence.amount;

            // Track pairwise amounts: how much payer paid for each contributor
            if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                for (contributor_id, amount) in contribs {
                    // payer paid this amount for contributor
                    let entry = pairwise_map.entry((payer_id, *contributor_id)).or_insert((0.0, Vec::new()));
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

        // Add to owed totals from contributions
        if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
            for (participant_id, amount) in contribs {
                *owed_map.entry(*participant_id).or_insert(0.0) += amount;
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

    // Calculate pool ownership (if a pool exists)
    let pool_ownership = if let Some((pool_id, pool_name)) = participants
        .iter()
        .find(|(_, _, account_type)| account_type == "pool")
        .map(|(id, name, _)| (*id, name.clone()))
    {
        // Track contributions to pool (deposited) and consumption from pool (expenses)
        // Contributions: Payments where pool is a contributor (someone paid, pool benefited)
        //   -> The PAYER's ownership increases by the pool's contribution amount
        // Consumption: Payments where pool is the payer
        //   -> Each contributor's ownership decreases by their share

        let mut ownership_map: HashMap<i64, (f64, f64)> = HashMap::new(); // (contributed, consumed)

        for occurrence in &all_occurrences {
            if let Some(contribs) = contribution_map.get(&occurrence.payment_id) {
                // Check if pool is involved in this payment
                let pool_contrib = contribs.iter().find(|(pid, _)| *pid == pool_id);

                if let Some(payer_id) = occurrence.payer_id {
                    if payer_id == pool_id {
                        // Pool is the payer: each contributor's ownership decreases
                        for (contributor_id, amount) in contribs {
                            if *contributor_id != pool_id {
                                let entry = ownership_map.entry(*contributor_id).or_insert((0.0, 0.0));
                                entry.1 += amount; // consumed
                            }
                        }
                    } else if let Some((_, pool_amount)) = pool_contrib {
                        // Pool is a contributor: the payer's ownership increases
                        let entry = ownership_map.entry(payer_id).or_insert((0.0, 0.0));
                        entry.0 += pool_amount; // contributed
                    }
                }
            }
        }

        // Build ownership entries for non-pool participants
        let mut entries: Vec<PoolOwnershipEntry> = participants
            .iter()
            .filter(|(id, _, account_type)| *id != pool_id && account_type != "pool")
            .filter_map(|(id, name, _)| {
                let (contributed, consumed) = ownership_map.get(id).copied().unwrap_or((0.0, 0.0));
                if contributed > 0.01 || consumed > 0.01 {
                    Some(PoolOwnershipEntry {
                        participant_id: *id,
                        participant_name: name.clone(),
                        contributed,
                        consumed,
                        ownership: contributed - consumed,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by ownership descending
        entries.sort_by(|a, b| b.ownership.partial_cmp(&a.ownership).unwrap_or(std::cmp::Ordering::Equal));

        let total_balance: f64 = entries.iter().map(|e| e.ownership).sum();

        Some(PoolOwnership {
            pool_id,
            pool_name,
            entries,
            total_balance,
        })
    } else {
        None
    };

    Ok(DebtSummary {
        balances,
        settlements,
        target_date: target_date.to_string(),
        occurrences: all_occurrences,
        pairwise_balances,
        pool_ownership,
    })
}

/// Generate all occurrences of a payment up to target_date
fn generate_payment_occurrences(payment: &Payment, target_date: NaiveDate) -> Vec<PaymentOccurrence> {
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
        });
        return occurrences;
    }

    // Recurring payment - generate occurrences
    let recurrence_type = payment.recurrence_type.as_deref().unwrap_or("monthly");
    let interval = payment.recurrence_interval.unwrap_or(1) as u32;
    let times_per = payment.recurrence_times_per;
    let end_date = payment.recurrence_end_date.as_ref()
        .and_then(|d| parse_date(d))
        .unwrap_or(target_date);

    let effective_end = end_date.min(target_date);
    let mut current = start_date;

    // Calculate the effective interval based on times_per
    // If times_per is set, we divide the period by times_per
    let (effective_interval, effective_type) = if let Some(times) = times_per {
        calculate_times_per_interval(recurrence_type, interval, times as u32)
    } else {
        (interval, recurrence_type.to_string())
    };

    while current <= effective_end {
        occurrences.push(PaymentOccurrence {
            payment_id: payment.id,
            description: payment.description.clone(),
            amount: payment.amount,
            occurrence_date: current.format("%Y-%m-%d").to_string(),
            payer_id: payment.payer_id,
            is_recurring: true,
        });

        current = match add_interval(current, &effective_type, effective_interval) {
            Some(d) => d,
            None => break,
        };
    }

    occurrences
}

/// Calculate effective interval for "X times per period"
fn calculate_times_per_interval(recurrence_type: &str, interval: u32, times_per: u32) -> (u32, String) {
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
        _ => (interval, recurrence_type.to_string())
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
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
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
                to_participant_name: participant_map.get(creditor_id).cloned().unwrap_or_default(),
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
