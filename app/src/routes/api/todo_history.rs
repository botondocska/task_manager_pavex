use sqlx::{QueryBuilder, SqlitePool};
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

pub async fn history_counts(
    user_id: &str,
    period: Period,
    num_periods: i64,
    label_filter: &LabelFilter,
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

    push_label_filter(&mut qb, label_filter);

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

    push_label_filter(&mut qb, label_filter);

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

fn push_label_filter(qb: &mut QueryBuilder<'_, sqlx::Sqlite>, label_filter: &LabelFilter) {
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
    pool: &SqlitePool,
) -> Result<Vec<PeriodCount>, sqlx::Error> {
    let today = time::OffsetDateTime::now_utc().date();
    let expected = expected_period_keys(period, num_periods, today);

    let found = history_counts(user_id, period, num_periods, label_filter, pool).await?;
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
    pool: &SqlitePool,
) -> Result<Vec<DurationPeriod>, sqlx::Error> {
    let today = time::OffsetDateTime::now_utc().date();
    let expected = expected_period_keys(period, num_periods, today);

    let found = duration_counts(user_id, period, num_periods, label_filter, pool).await?;
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
