use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::error::AppResult;

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

#[derive(Debug, Serialize)]
pub struct DebtSummary {
    pub balances: Vec<ParticipantBalance>,
    pub settlements: Vec<Debt>,
}

pub async fn calculate_debts(pool: &SqlitePool, project_id: i64) -> AppResult<DebtSummary> {
    // Get all participants for this project
    let participants: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM participants WHERE project_id = ?"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let participant_map: HashMap<i64, String> = participants.iter().cloned().collect();

    // Calculate total paid by each participant (as payer)
    let paid: Vec<(i64, f64)> = sqlx::query_as(
        "SELECT payer_id, SUM(amount) FROM payments
         WHERE project_id = ? AND payer_id IS NOT NULL
         GROUP BY payer_id"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let paid_map: HashMap<i64, f64> = paid.into_iter().collect();

    // Calculate total owed by each participant (from contributions)
    let owed: Vec<(i64, f64)> = sqlx::query_as(
        "SELECT c.participant_id, SUM(c.amount)
         FROM contributions c
         JOIN payments p ON c.payment_id = p.id
         WHERE p.project_id = ?
         GROUP BY c.participant_id"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let owed_map: HashMap<i64, f64> = owed.into_iter().collect();

    // Calculate balances
    let mut balances: Vec<ParticipantBalance> = participants
        .iter()
        .map(|(id, name)| {
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
    let settlements = calculate_settlements(&balances, &participant_map);

    Ok(DebtSummary {
        balances,
        settlements,
    })
}

fn calculate_settlements(balances: &[ParticipantBalance], participant_map: &HashMap<i64, String>) -> Vec<Debt> {
    let mut settlements = Vec::new();

    // Separate into debtors (negative balance) and creditors (positive balance)
    let mut debtors: Vec<(i64, f64)> = balances
        .iter()
        .filter(|b| b.net_balance < -0.01) // Small threshold for floating point
        .map(|b| (b.participant_id, -b.net_balance)) // Convert to positive amount owed
        .collect();

    let mut creditors: Vec<(i64, f64)> = balances
        .iter()
        .filter(|b| b.net_balance > 0.01)
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
