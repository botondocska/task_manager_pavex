use chrono::TimeZone;
use rrule::{RRuleSet, Tz};
use sqlx::SqlitePool;
use std::str::FromStr;
use time::OffsetDateTime;

/// Runs once immediately (catch-up for "today"), then loops forever,
/// waiting until the next UTC midnight and running again each cycle.
pub async fn run_daily_loop(pool: SqlitePool) {
    if let Err(e) = record_todays_occurrences(&pool).await {
        tracing::error!(error = %e, "daily history job failed (startup run)");
    }

    loop {
        let now = OffsetDateTime::now_utc();
        let tomorrow_midnight = (now.date() + time::Duration::days(1))
            .midnight()
            .assume_utc();
        let sleep_for = tomorrow_midnight - now;

        tokio::time::sleep(std::time::Duration::from_secs(
            sleep_for.whole_seconds().max(0) as u64,
        ))
        .await;

        if let Err(e) = record_todays_occurrences(&pool).await {
            tracing::error!(error = %e, "daily history job failed");
        }
    }
}

/// Runs once per day (see wiring below). For every todo across every user,
/// determines whether today is a due occurrence (per its rrule, or — for
/// one-off todos — whether today is its creation day) and inserts a
/// `todo_history` row for it if one doesn't already exist for today.
pub async fn record_todays_occurrences(pool: &SqlitePool) -> Result<(), anyhow::Error> {
    let today = OffsetDateTime::now_utc().date();
    let today_str = today.to_string();

    let todos = sqlx::query!(r#"SELECT id, user_id, rrule, completed FROM todos"#)
        .fetch_all(pool)
        .await?;

    for todo in todos {
        let is_due_today = match &todo.rrule {
            Some(rrule_str) => {
                let set = RRuleSet::from_str(rrule_str)?;
                occurs_on(&set, today)?
            }
            None => true,
        };

        if !is_due_today {
            continue;
        }

        sqlx::query!(
            r#"
            INSERT INTO todo_history (user_id, todo_id, occurrence_date, completed)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(todo_id, occurrence_date) DO UPDATE SET completed = excluded.completed
            "#,
            todo.user_id,
            todo.id,
            today_str,
            todo.completed,
        )
        .execute(pool)
        .await?;
    }

    // Fresh day — clear completion state for the next 24 hours.
    sqlx::query!(r#"UPDATE todos SET completed = 0"#)
        .execute(pool)
        .await?;

    Ok(())
}

pub fn occurs_on(set: &RRuleSet, day: time::Date) -> Result<bool, anyhow::Error> {
    let year = day.year();
    let month = day.month() as u32;
    let d = day.day() as u32;

    let start = Tz::UTC
        .with_ymd_and_hms(year, month, d, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid date"))?;
    let end = Tz::UTC
        .with_ymd_and_hms(year, month, d, 23, 59, 59)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid date"))?;

    let bounded = set.clone().after(start).before(end);
    Ok(!bounded.all(1).dates.is_empty())
}
