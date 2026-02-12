use chrono::{Months, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::error::AppResult;
use crate::models::Payment;
use crate::routes::budget::BudgetOverride;

/// Source of a budget line item
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum BudgetSource {
    RecurringPayment { payment_id: i64 },
    Override { override_id: i64 },
    Observed { payment_ids: Vec<i64> },
}

/// A single budget line item
#[derive(Debug, Clone, Serialize)]
pub struct BudgetLineItem {
    pub tag: Option<String>,
    pub name: String,
    pub yearly_amount: f64,
    pub is_income: bool,
    pub source: BudgetSource,
}

/// Summary of budget items
#[derive(Debug, Clone, Serialize)]
pub struct BudgetSummary {
    pub items: Vec<BudgetLineItem>,
    pub total_income_yearly: f64,
    pub total_expenses_yearly: f64,
    pub net_yearly: f64,
}

impl BudgetSummary {
    pub fn new(items: Vec<BudgetLineItem>) -> Self {
        let total_income_yearly: f64 = items
            .iter()
            .filter(|i| i.is_income)
            .map(|i| i.yearly_amount)
            .sum();
        let total_expenses_yearly: f64 = items
            .iter()
            .filter(|i| !i.is_income)
            .map(|i| i.yearly_amount)
            .sum();
        let net_yearly = total_income_yearly - total_expenses_yearly;

        Self {
            items,
            total_income_yearly,
            total_expenses_yearly,
            net_yearly,
        }
    }
}

/// Full budget response with budgeted, observed, and available tags
#[derive(Debug, Clone, Serialize)]
pub struct BudgetResponse {
    pub budgeted: BudgetSummary,
    pub observed: BudgetSummary,
    pub available_tags: Vec<String>,
}

/// Calculate yearly occurrences for a recurring payment
fn calculate_yearly_occurrences(payment: &Payment) -> f64 {
    if !payment.is_recurring {
        return 0.0;
    }

    let recurrence_type = payment.recurrence_type.as_deref().unwrap_or("monthly");
    let interval = payment.recurrence_interval.unwrap_or(1) as f64;

    // Calculate how many times per year this payment occurs
    let base_occurrences = match recurrence_type {
        "daily" => 365.0,
        "weekly" => 52.0,
        "monthly" => 12.0,
        "yearly" => 1.0,
        _ => 12.0, // Default to monthly
    };

    // Apply interval (e.g., every 2 weeks = 52/2 = 26 occurrences)
    let occurrences_per_year = base_occurrences / interval;

    // Handle multiple times per period
    let times_multiplier = if let Some(weekdays_json) = &payment.recurrence_weekdays {
        // Count total weekdays selected across all week patterns
        if let Ok(patterns) = serde_json::from_str::<Vec<Vec<i32>>>(weekdays_json) {
            // Sum up all weekdays selected
            patterns.iter().map(|week| week.len() as f64).sum::<f64>()
                / patterns.len() as f64 // Average per interval
        } else {
            1.0
        }
    } else if let Some(monthdays_json) = &payment.recurrence_monthdays {
        // Count how many days per month
        if let Ok(days) = serde_json::from_str::<Vec<i32>>(monthdays_json) {
            days.len() as f64
        } else {
            1.0
        }
    } else if let Some(months_json) = &payment.recurrence_months {
        // For yearly: count how many months
        if let Ok(months) = serde_json::from_str::<Vec<i32>>(months_json) {
            months.len() as f64
        } else {
            1.0
        }
    } else {
        1.0
    };

    occurrences_per_year * times_multiplier
}

/// Calculate yearly amount for a recurring payment
fn calculate_yearly_amount(payment: &Payment) -> f64 {
    let occurrences = calculate_yearly_occurrences(payment);
    payment.amount * occurrences
}

/// Parse tags from JSON string
fn parse_tags(tags_json: &Option<String>) -> Vec<String> {
    tags_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
        .unwrap_or_default()
}

/// Get the first tag from a payment, or None
fn get_primary_tag(payment: &Payment) -> Option<String> {
    parse_tags(&payment.tags).into_iter().next()
}

/// Determine if a payment represents income
/// Income: receiver_account_id is set AND payer_id is NULL (external funds coming in)
/// OR: receiver_account_id is a user account and it's an incoming transfer
fn is_income_payment(payment: &Payment) -> bool {
    // External funds to user/pool: payer is NULL, receiver is set
    payment.payer_id.is_none() && payment.receiver_account_id.is_some()
}

/// Calculate budget from recurring payments and overrides
pub async fn calculate_budget(
    pool: &SqlitePool,
    project_id: i64,
    participant_id: Option<i64>,
) -> AppResult<BudgetResponse> {
    // Get all recurring payments for the project
    let recurring_payments: Vec<Payment> = if let Some(pid) = participant_id {
        sqlx::query_as(
            "SELECT * FROM payments WHERE project_id = ? AND is_recurring = 1 AND (payer_id = ? OR receiver_account_id = ?)"
        )
        .bind(project_id)
        .bind(pid)
        .bind(pid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as("SELECT * FROM payments WHERE project_id = ? AND is_recurring = 1")
            .bind(project_id)
            .fetch_all(pool)
            .await?
    };

    // Get budget overrides
    let overrides: Vec<BudgetOverride> = if let Some(pid) = participant_id {
        sqlx::query_as(
            "SELECT * FROM budget_overrides WHERE project_id = ? AND (participant_id IS NULL OR participant_id = ?)"
        )
        .bind(project_id)
        .bind(pid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as("SELECT * FROM budget_overrides WHERE project_id = ?")
            .bind(project_id)
            .fetch_all(pool)
            .await?
    };

    // Collect payment IDs that should be excluded due to overrides
    let excluded_payment_ids: std::collections::HashSet<i64> = overrides
        .iter()
        .filter(|o| o.override_type == "exclude")
        .filter_map(|o| o.linked_payment_id)
        .collect();

    // Build budgeted items from recurring payments
    let mut budgeted_items: Vec<BudgetLineItem> = recurring_payments
        .iter()
        .filter(|p| !excluded_payment_ids.contains(&p.id))
        .map(|p| BudgetLineItem {
            tag: get_primary_tag(p),
            name: p.description.clone(),
            yearly_amount: calculate_yearly_amount(p),
            is_income: is_income_payment(p),
            source: BudgetSource::RecurringPayment { payment_id: p.id },
        })
        .collect();

    // Apply adjustments from overrides
    for override_entry in &overrides {
        match override_entry.override_type.as_str() {
            "add" => {
                // Add a new budget item
                budgeted_items.push(BudgetLineItem {
                    tag: override_entry.tag.clone(),
                    name: override_entry.name.clone(),
                    yearly_amount: override_entry.yearly_amount.abs(),
                    is_income: override_entry.yearly_amount >= 0.0,
                    source: BudgetSource::Override {
                        override_id: override_entry.id,
                    },
                });
            }
            "adjust" => {
                // If linked to a payment, modify the existing item
                if let Some(payment_id) = override_entry.linked_payment_id {
                    if let Some(item) = budgeted_items.iter_mut().find(|i| {
                        matches!(&i.source, BudgetSource::RecurringPayment { payment_id: pid } if *pid == payment_id)
                    }) {
                        item.yearly_amount = override_entry.yearly_amount.abs();
                        item.is_income = override_entry.yearly_amount >= 0.0;
                        if override_entry.tag.is_some() {
                            item.tag = override_entry.tag.clone();
                        }
                    }
                }
            }
            "exclude" => {
                // Already handled by filtering above
            }
            _ => {}
        }
    }

    let budgeted = BudgetSummary::new(budgeted_items);

    // Calculate observed from last 12 months of payments
    let today = chrono::Utc::now().date_naive();
    let one_year_ago = today
        .checked_sub_months(Months::new(12))
        .unwrap_or(today);

    // Get all payments in the last 12 months
    let recent_payments: Vec<Payment> = if let Some(pid) = participant_id {
        sqlx::query_as(
            "SELECT * FROM payments WHERE project_id = ? AND payment_date >= ? AND (payer_id = ? OR receiver_account_id = ?)"
        )
        .bind(project_id)
        .bind(one_year_ago.format("%Y-%m-%d").to_string())
        .bind(pid)
        .bind(pid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT * FROM payments WHERE project_id = ? AND payment_date >= ?"
        )
        .bind(project_id)
        .bind(one_year_ago.format("%Y-%m-%d").to_string())
        .fetch_all(pool)
        .await?
    };

    // Group by tag and calculate yearly amounts
    let mut tag_totals: HashMap<Option<String>, (f64, f64, Vec<i64>)> = HashMap::new(); // (income, expense, payment_ids)

    for payment in &recent_payments {
        let tag = get_primary_tag(payment);
        let entry = tag_totals.entry(tag).or_insert((0.0, 0.0, Vec::new()));

        if is_income_payment(payment) {
            entry.0 += payment.amount;
        } else {
            entry.1 += payment.amount;
        }
        entry.2.push(payment.id);
    }

    // Calculate how many months of data we have
    let earliest_payment_date = recent_payments
        .iter()
        .filter_map(|p| NaiveDate::parse_from_str(&p.payment_date, "%Y-%m-%d").ok())
        .min();

    let months_of_data = if let Some(earliest) = earliest_payment_date {
        let days_diff = (today - earliest).num_days();
        (days_diff as f64 / 30.44).max(1.0).min(12.0)
    } else {
        12.0
    };

    // Convert to yearly amounts
    let annualization_factor = 12.0 / months_of_data;

    let observed_items: Vec<BudgetLineItem> = tag_totals
        .into_iter()
        .flat_map(|(tag, (income, expense, payment_ids))| {
            let mut items = Vec::new();

            if income > 0.0 {
                items.push(BudgetLineItem {
                    tag: tag.clone(),
                    name: tag.clone().unwrap_or_else(|| "Uncategorized Income".to_string()),
                    yearly_amount: income * annualization_factor,
                    is_income: true,
                    source: BudgetSource::Observed {
                        payment_ids: payment_ids.clone(),
                    },
                });
            }

            if expense > 0.0 {
                items.push(BudgetLineItem {
                    tag: tag.clone(),
                    name: tag.unwrap_or_else(|| "Uncategorized Expenses".to_string()),
                    yearly_amount: expense * annualization_factor,
                    is_income: false,
                    source: BudgetSource::Observed { payment_ids },
                });
            }

            items
        })
        .collect();

    let observed = BudgetSummary::new(observed_items);

    // Get all unique tags used in the project
    let tags_rows: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT tags FROM payments WHERE project_id = ? AND tags IS NOT NULL")
            .bind(project_id)
            .fetch_all(pool)
            .await?;

    let mut all_tags: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (tags_json,) in tags_rows {
        if let Some(json_str) = tags_json {
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&json_str) {
                for tag in tags {
                    all_tags.insert(tag);
                }
            }
        }
    }

    // Also add tags from overrides
    for override_entry in &overrides {
        if let Some(ref tag) = override_entry.tag {
            all_tags.insert(tag.clone());
        }
    }

    let mut available_tags: Vec<String> = all_tags.into_iter().collect();
    available_tags.sort();

    Ok(BudgetResponse {
        budgeted,
        observed,
        available_tags,
    })
}
