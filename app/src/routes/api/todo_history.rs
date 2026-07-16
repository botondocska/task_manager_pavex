use sqlx::{QueryBuilder, SqlitePool};
use std::collections::HashSet;
use time::{Date, Duration as TimeDuration};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum Period {
    Day,
    Month,
    Year,
}

impl Period {
    fn sql_strftime_format(&self) -> &'static str {
        match self {
            Period::Day => "%Y-%m-%d",
            Period::Month => "%Y-%m",
            Period::Year => "%Y",
        }
    }
}

/// Selects either "no label" (label_id IS NULL after joining to todos,
/// including the case where the todo itself was deleted) or specific ids.
pub enum LabelFilter {
    All,
    Selected {
        label_ids: Vec<i64>,
        include_unlabeled: bool,
    },
}

pub struct PeriodCount {
    pub period_start: String,
    pub completed_count: i64,
    pub total_count: i64,
}

pub struct DurationPeriod {
    pub period_start: String,
    pub completed_minutes: i64,
    pub total_minutes: i64,
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

pub struct HistoryOccurrence {
    pub todo_id: Option<i64>,
    pub occurrence_date: String,
    pub completed: bool,
}

pub async fn occurrences_in_range(
    user_id: &str,
    start: &str,
    end: &str,
    pool: &SqlitePool,
) -> Result<Vec<HistoryOccurrence>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT todo_id, occurrence_date, completed as "completed: bool"
           FROM todo_history
           WHERE user_id = ? AND occurrence_date BETWEEN ? AND ?"#,
        user_id,
        start,
        end,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryOccurrence {
            todo_id: r.todo_id,
            occurrence_date: r.occurrence_date,
            completed: r.completed,
        })
        .collect())
}

pub async fn history_counts(
    user_id: &str,
    period: Period,
    num_periods: i64,
    label_filter: &LabelFilter,
    include_one_off: bool,
    pool: &SqlitePool,
) -> Result<Vec<PeriodCount>, sqlx::Error> {
    let fmt = period.sql_strftime_format();

    let mut qb = QueryBuilder::new(r#"SELECT strftime('"#);
    qb.push(fmt);
    qb.push(
        r#"', th.occurrence_date) AS period_start,
            SUM(th.completed) AS completed_count,
            COUNT(*) AS total_count
        FROM todo_history th
        LEFT JOIN todos t ON t.id = th.todo_id
        WHERE th.user_id = "#,
    );
    qb.push_bind(user_id);

    push_filters(&mut qb, label_filter, include_one_off);

    qb.push(" GROUP BY period_start ORDER BY period_start DESC LIMIT ");
    qb.push_bind(num_periods);

    let rows = qb
        .build_query_as::<(String, i64, i64)>()
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(period_start, completed_count, total_count)| PeriodCount {
            period_start,
            completed_count,
            total_count,
        })
        .collect())
}

pub async fn duration_counts(
    user_id: &str,
    period: Period,
    num_periods: i64,
    label_filter: &LabelFilter,
    include_one_off: bool,
    pool: &SqlitePool,
) -> Result<Vec<DurationPeriod>, sqlx::Error> {
    let fmt = period.sql_strftime_format();

    let mut qb = QueryBuilder::new(r#"SELECT strftime('"#);
    qb.push(fmt);
    qb.push(
        r#"', th.occurrence_date) AS period_start,
            COALESCE(SUM(CASE WHEN th.completed THEN COALESCE(t.duration, 0) ELSE 0 END), 0) AS completed_minutes,
            COALESCE(SUM(COALESCE(t.duration, 0)), 0) AS total_minutes
        FROM todo_history th
        LEFT JOIN todos t ON t.id = th.todo_id
        WHERE th.user_id = "#,
    );
    qb.push_bind(user_id);

    push_filters(&mut qb, label_filter, include_one_off);

    qb.push(" GROUP BY period_start ORDER BY period_start DESC LIMIT ");
    qb.push_bind(num_periods);

    let rows = qb
        .build_query_as::<(String, i64, i64)>()
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(
            |(period_start, completed_minutes, total_minutes)| DurationPeriod {
                period_start,
                completed_minutes,
                total_minutes,
            },
        )
        .collect())
}

/// Applies label filter and one-off filter to the WHERE clause.
/// `t.rrule IS NULL` marks a one-off todo (no recurrence rule).
/// When `include_one_off` is false, those rows are excluded.
fn push_filters(
    qb: &mut QueryBuilder<'_, sqlx::Sqlite>,
    label_filter: &LabelFilter,
    include_one_off: bool,
) {
    match label_filter {
        LabelFilter::All => {}
        LabelFilter::Selected {
            label_ids,
            include_unlabeled,
        } => {
            qb.push(" AND (");
            let mut first = true;

            if !label_ids.is_empty() {
                qb.push("t.label_id IN (");
                let mut sep = qb.separated(", ");
                for id in label_ids {
                    sep.push_bind(*id);
                }
                qb.push(")");
                first = false;
            }

            if *include_unlabeled {
                if !first {
                    qb.push(" OR ");
                }
                qb.push("t.label_id IS NULL");
            }

            qb.push(")");
        }
    }

    if !include_one_off {
        qb.push(" AND t.rrule IS NOT NULL");
    }
}

// ---------------------------------------------------------------------------
// Zero-fill
// ---------------------------------------------------------------------------

fn expected_period_keys(period: Period, num_periods: i64, today: Date) -> Vec<String> {
    let mut keys = Vec::with_capacity(num_periods as usize);
    match period {
        Period::Day => {
            for i in 0..num_periods {
                let d = today - TimeDuration::days(i);
                keys.push(d.to_string());
            }
        }
        Period::Month => {
            let mut year = today.year();
            let mut month = today.month() as i32;
            for _ in 0..num_periods {
                keys.push(format!("{year:04}-{month:02}"));
                month -= 1;
                if month == 0 {
                    month = 12;
                    year -= 1;
                }
            }
        }
        Period::Year => {
            for i in 0..num_periods {
                keys.push(format!("{}", today.year() - i as i32));
            }
        }
    }
    keys
}

pub async fn history_counts_filled(
    user_id: &str,
    period: Period,
    num_periods: i64,
    label_filter: &LabelFilter,
    include_one_off: bool,
    pool: &SqlitePool,
) -> Result<Vec<PeriodCount>, sqlx::Error> {
    let today = time::OffsetDateTime::now_utc().date();
    let expected = expected_period_keys(period, num_periods, today);

    let found = history_counts(
        user_id,
        period,
        num_periods,
        label_filter,
        include_one_off,
        pool,
    )
    .await?;
    let found_map: std::collections::HashMap<String, (i64, i64)> = found
        .into_iter()
        .map(|p| (p.period_start, (p.completed_count, p.total_count)))
        .collect();

    let mut result: Vec<PeriodCount> = expected
        .into_iter()
        .map(|key| {
            let (completed, total) = found_map.get(&key).copied().unwrap_or((0, 0));
            PeriodCount {
                period_start: key,
                completed_count: completed,
                total_count: total,
            }
        })
        .collect();

    result.reverse();
    Ok(result)
}

pub async fn duration_counts_filled(
    user_id: &str,
    period: Period,
    num_periods: i64,
    label_filter: &LabelFilter,
    include_one_off: bool,
    pool: &SqlitePool,
) -> Result<Vec<DurationPeriod>, sqlx::Error> {
    let today = time::OffsetDateTime::now_utc().date();
    let expected = expected_period_keys(period, num_periods, today);

    let found = duration_counts(
        user_id,
        period,
        num_periods,
        label_filter,
        include_one_off,
        pool,
    )
    .await?;
    let found_map: std::collections::HashMap<String, (i64, i64)> = found
        .into_iter()
        .map(|p| (p.period_start, (p.completed_minutes, p.total_minutes)))
        .collect();

    let mut result: Vec<DurationPeriod> = expected
        .into_iter()
        .map(|key| {
            let (completed_minutes, total_minutes) = found_map.get(&key).copied().unwrap_or((0, 0));
            DurationPeriod {
                period_start: key,
                completed_minutes,
                total_minutes,
            }
        })
        .collect();

    result.reverse();
    Ok(result)
}

/// Returns the set of (todo_id, occurrence_date) pairs marked completed,
/// for a user, within [start, end] inclusive. Date strings are "YYYY-MM-DD".
pub async fn completed_occurrences(
    user_id: &str,
    start: &str,
    end: &str,
    pool: &SqlitePool,
) -> Result<HashSet<(i64, String)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT todo_id as "todo_id!: i64", occurrence_date
        FROM todo_history
        WHERE user_id = ? AND completed = 1
          AND occurrence_date >= ? AND occurrence_date <= ?
        "#,
        user_id,
        start,
        end,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.todo_id, r.occurrence_date))
        .collect())
}

pub async fn toggle_history_completion(
    user_id: &str,
    todo_id: i64,
    occurrence_date: &str,
    pool: &SqlitePool,
) -> Result<Option<bool>, anyhow::Error> {
    let existing = sqlx::query!(
        r#"SELECT th.completed as "completed: bool"
           FROM todo_history th
           JOIN todos t ON t.id = th.todo_id
           WHERE th.todo_id = ? AND th.occurrence_date = ? AND t.user_id = ?"#,
        todo_id,
        occurrence_date,
        user_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = existing else { return Ok(None) };
    let new_val = !row.completed;

    // Always write the history row for the actual occurrence date.
    sqlx::query!(
        r#"UPDATE todo_history SET completed = ? WHERE todo_id = ? AND occurrence_date = ?"#,
        new_val,
        todo_id,
        occurrence_date,
    )
    .execute(pool)
    .await?;

    // If this is the most recent occurrence, keep todos.completed_at in
    // sync -- stamped with THIS occurrence's date, not "now".
    let most_recent = sqlx::query!(
        r#"SELECT occurrence_date FROM todo_history
           WHERE todo_id = ? ORDER BY occurrence_date DESC LIMIT 1"#,
        todo_id,
    )
    .fetch_one(pool)
    .await?;

    if most_recent.occurrence_date == occurrence_date {
        if new_val {
            let occurrence_ts = format!("{occurrence_date}T12:00:00.000Z");
            sqlx::query!(
                r#"UPDATE todos SET completed_at = ? WHERE id = ? AND user_id = ?"#,
                occurrence_ts,
                todo_id,
                user_id,
            )
            .execute(pool)
            .await?;
        } else {
            sqlx::query!(
                r#"UPDATE todos SET completed_at = NULL WHERE id = ? AND user_id = ?"#,
                todo_id,
                user_id,
            )
            .execute(pool)
            .await?;
        }
    }

    Ok(Some(new_val))
}
