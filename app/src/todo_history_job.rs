use chrono::{DateTime, Datelike, TimeZone};
use rrule::{RRuleSet, Tz};
use sqlx::SqlitePool;
use time::{Date, OffsetDateTime};

pub async fn record_occurrences_for_user_range(
    user_id: &str,
    from: Date,
    to: Date,
    pool: &SqlitePool,
) -> Result<(), anyhow::Error> {
    let todos = sqlx::query!(
        r#"SELECT id, rrule, completed_at as "completed_at: OffsetDateTime",
           created_at as "created_at: OffsetDateTime"
           FROM todos WHERE user_id = ?"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    let mut rows: Vec<(i64, String, bool)> = Vec::new();

    for todo in &todos {
        let completed_that_day =
            |d: Date| todo.completed_at.map(|ts| ts.date() == d).unwrap_or(false);

        match &todo.rrule {
            None => {
                let created_date = todo.created_at.date();
                if created_date >= from && created_date <= to {
                    rows.push((todo.id, created_date.to_string(), false));
                }
            }
            Some(rrule_str) => {
                let set: RRuleSet = rrule_str.parse()?;
                let start = day_start_tz(from)?;
                let end = day_end_tz(to)?;
                let occurrences = set.after(start).before(end).all(u16::MAX).dates;
                for occ in occurrences {
                    let d = time_date_from_chrono(occ);
                    rows.push((todo.id, d.to_string(), completed_that_day(d)));
                }
            }
        }
    }

    if rows.is_empty() {
        return Ok(());
    }

    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO todo_history (user_id, todo_id, occurrence_date, completed) ",
    );
    qb.push_values(rows, |mut b, (todo_id, date, completed)| {
        b.push_bind(user_id)
            .push_bind(todo_id)
            .push_bind(date)
            .push_bind(completed);
    });
    qb.push(" ON CONFLICT(todo_id, occurrence_date) DO UPDATE SET completed = excluded.completed");
    qb.build().execute(pool).await?;

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

fn day_start_tz(day: Date) -> Result<DateTime<Tz>, anyhow::Error> {
    Tz::UTC
        .with_ymd_and_hms(day.year(), day.month() as u32, day.day() as u32, 0, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid date"))
}

fn day_end_tz(day: Date) -> Result<DateTime<Tz>, anyhow::Error> {
    Tz::UTC
        .with_ymd_and_hms(day.year(), day.month() as u32, day.day() as u32, 23, 59, 59)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid date"))
}

fn time_date_from_chrono(dt: DateTime<Tz>) -> Date {
    Date::from_calendar_date(
        dt.year(),
        time::Month::try_from(dt.month() as u8).expect("valid month"),
        dt.day() as u8,
    )
    .expect("valid date from chrono")
}
