/// Utility to recalculate all contribution amounts with 4-decimal precision
/// This fixes rounding errors from the old 2-decimal calculation
///
/// Usage: cargo run --bin recalculate_contributions
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "data/bonscompte.db".to_string());

    println!("Connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", database_url)).await?;

    // Get all payments
    let payments: Vec<(i64, f64)> = sqlx::query_as("SELECT id, amount FROM payments")
        .fetch_all(&pool)
        .await?;

    println!("Found {} payments to process", payments.len());

    let mut total_updated = 0;
    let mut total_errors = 0;

    for (payment_id, payment_amount) in payments {
        // Get all contributions for this payment
        let contributions: Vec<(i64, i64, f64, f64)> = sqlx::query_as(
            "SELECT id, participant_id, amount, weight FROM contributions WHERE payment_id = ?",
        )
        .bind(payment_id)
        .fetch_all(&pool)
        .await?;

        if contributions.is_empty() {
            continue;
        }

        // Calculate total weight
        let total_weight: f64 = contributions.iter().map(|(_, _, _, w)| w).sum();

        if total_weight == 0.0 {
            eprintln!(
                "Warning: Payment {} has zero total weight, skipping",
                payment_id
            );
            total_errors += 1;
            continue;
        }

        // Track old and new amounts for debugging
        let mut old_total = 0.0;
        let mut new_total = 0.0;

        // Recalculate each contribution
        for (contrib_id, _participant_id, old_amount, weight) in contributions {
            old_total += old_amount;

            // New calculation with 4 decimals
            let new_amount = (payment_amount * weight / total_weight * 10000.0).round() / 10000.0;
            new_total += new_amount;

            // Update the contribution
            sqlx::query("UPDATE contributions SET amount = ? WHERE id = ?")
                .bind(new_amount)
                .bind(contrib_id)
                .execute(&pool)
                .await?;

            total_updated += 1;
        }

        // Check if there's still a rounding error
        let diff = (new_total - payment_amount).abs();
        if diff > 0.001 {
            println!(
                "Payment {}: amount={:.2}, old_total={:.4}, new_total={:.4}, diff={:.4}",
                payment_id, payment_amount, old_total, new_total, diff
            );
        }
    }

    println!("\nRecalculation complete!");
    println!("  Updated contributions: {}", total_updated);
    println!("  Errors/skipped: {}", total_errors);

    Ok(())
}
