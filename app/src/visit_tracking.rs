use crate::todo_history_job::record_occurrences_for_user_range;
use sqlx::SqlitePool;
use time::{Date, Duration, OffsetDateTime};

const MAX_BACKFILL_DAYS: i64 = 366;

/// Backfill todo_history for every day between the user's last visit and
/// today (exclusive of last_visit, inclusive of today), in one bulk pass.
/// Updates last_visit. No-op if already caught up today.
pub async fn catch_up_user(user_id: &str, pool: &SqlitePool) -> Result<Date, anyhow::Error> {
    let today = OffsetDateTime::now_utc().date();

    let row = sqlx::query!(r#"SELECT last_visit FROM users WHERE id = ?"#, user_id)
        .fetch_one(pool)
        .await?;

    let last_visit: Date = match row.last_visit.as_deref() {
        Some(s) => match Date::parse(s, &time::format_description::well_known::Iso8601::DATE) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, user_id, "corrupt last_visit, resetting to today");
                today
            }
        },
        None => today,
    };

    if last_visit >= today {
        return set_last_visit(user_id, today, pool).await;
    }

    let earliest_allowed = today - Duration::days(MAX_BACKFILL_DAYS);
    let start = last_visit.max(earliest_allowed);

    if start > last_visit {
        tracing::warn!(
            user_id,
            skipped_days = (start - last_visit).whole_days(),
            "last_visit older than backfill window, some history will be missing"
        );
    }

    if start < today {
        record_occurrences_for_user_range(user_id, start + Duration::days(1), today, pool).await?;
    }

    set_last_visit(user_id, today, pool).await
}

async fn set_last_visit(
    user_id: &str,
    today: Date,
    pool: &SqlitePool,
) -> Result<Date, anyhow::Error> {
    let today_str = today.to_string();
    sqlx::query!(
        r#"UPDATE users SET last_visit = ? WHERE id = ?"#,
        today_str,
        user_id,
    )
    .execute(pool)
    .await?;
    Ok(today)
}
