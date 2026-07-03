use crate::schemas::Todo;
use sqlx::SqlitePool;
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn list_for_user(user_id: &str, pool: &SqlitePool) -> Result<Vec<Todo>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, user_id as "user_id: Uuid", duration, rrule, title, description, label_id, created_at as "created_at: OffsetDateTime" 
        FROM todos 
        WHERE user_id = ?"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Todo {
            user_id: r.user_id,
            id: r.id,
            duration: r.duration,
            rrule: r.rrule,
            title: r.title,
            description: r.description,
            created_at: r.created_at,
            label_id: r.label_id,
        })
        .collect())
}

pub async fn create(
    user_id: &str,
    title: &str,
    description: Option<&str>,
    duration: Option<i64>,
    rrule: Option<&str>,
    label_id: Option<i64>,
    pool: &SqlitePool,
) -> Result<Todo, sqlx::Error> {
    let id = sqlx::query!(
        r#"INSERT INTO todos (user_id, title, description, duration, rrule, label_id) VALUES (?, ?, ?, ?, ?, ?)"#,
        user_id,
        title,
        description,
        duration,
        rrule,
        label_id,
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    Ok(Todo {
        id,
        user_id: user_id.parse().unwrap(),
        title: title.to_owned(),
        description: description.map(str::to_owned),
        duration,
        rrule: rrule.map(str::to_owned),
        label_id,
        created_at: OffsetDateTime::now_utc(),
    })
}

pub async fn update(
    user_id: &str,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    duration: Option<i64>,
    rrule: Option<&str>,
    label_id: Option<i64>,
    pool: &SqlitePool,
) -> Result<Option<Todo>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE todos SET
            title = COALESCE(?, title),
            description = COALESCE(?, description),
            duration = COALESCE(?, duration),
            rrule = COALESCE(?, rrule),
            label_id = COALESCE(?, label_id)
        WHERE id = ? AND user_id = ?
        "#,
        title,
        description,
        duration,
        rrule,
        label_id,
        id,
        user_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query!(
        r#"SELECT id, title, description, duration, rrule, label_id FROM todos WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(Some(Todo {
        id: row.id,
        user_id: user_id.parse().unwrap(),
        title: row.title,
        description: row.description,
        duration: row.duration,
        rrule: row.rrule,
        label_id: row.label_id,
        created_at: OffsetDateTime::now_utc(),
    }))
}

pub async fn delete(user_id: &str, id: i64, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"DELETE FROM todos WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
