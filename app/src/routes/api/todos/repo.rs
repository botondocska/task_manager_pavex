use crate::rrule_input::RRuleField;
use crate::schemas::{CreateTodoBody, Todo, UpdateTodoBody};
use sqlx::SqlitePool;
use std::str::FromStr;
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn list_for_user(user_id: &str, pool: &SqlitePool) -> Result<Vec<Todo>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, user_id, duration, rrule, title, description, label_id, completed_at as "completed_at: OffsetDateTime", created_at as "created_at: OffsetDateTime" 
        FROM todos 
        WHERE user_id = ?"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    let mut todos = Vec::with_capacity(rows.len());
    for r in rows {
        let user_id = Uuid::parse_str(&r.user_id)
            .map_err(|e| sqlx::Error::Decode(format!("invalid stored user_id: {e}").into()))?;
        let rrule = r
            .rrule
            .as_deref()
            .map(RRuleField::from_str)
            .transpose()
            .map_err(|e| sqlx::Error::Decode(format!("invalid stored rrule: {e}").into()))?;

        todos.push(Todo {
            user_id,
            id: r.id,
            duration: r.duration,
            rrule,
            title: r.title,
            description: r.description,
            completed_at: r.completed_at,
            created_at: r.created_at,
            label_id: r.label_id,
        });
    }

    todos.sort_by_key(|t| t.start_date_key().unwrap_or(i64::MAX));
    Ok(todos)
}

pub async fn create(
    user_id_str: &str,
    fields: &CreateTodoBody,
    pool: &SqlitePool,
) -> Result<Todo, sqlx::Error> {
    let rrule_str = fields.rrule.as_ref().map(|r| r.0.to_string());
    let completed_at = fields
        .completed
        .unwrap_or(false)
        .then(OffsetDateTime::now_utc);

    let id = sqlx::query!(
        r#"INSERT INTO todos (user_id, title, description, duration, rrule, label_id, completed_at) VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        user_id_str,
        fields.title,
        fields.description,
        fields.duration,
        rrule_str,
        fields.label_id,
        completed_at,
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    let row = sqlx::query!(
        r#"SELECT id, user_id, title, description, duration, rrule, label_id, 
        completed_at as "completed_at: OffsetDateTime", 
        created_at as "created_at: OffsetDateTime"
        FROM todos WHERE id = ?"#,
        id,
    )
    .fetch_one(pool)
    .await?;
    let user_id = Uuid::parse_str(&row.user_id)
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored user_id: {e}").into()))?;
    let rrule = row
        .rrule
        .as_deref()
        .map(RRuleField::from_str)
        .transpose()
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored rrule: {e}").into()))?;

    let todo = Todo {
        id: row.id,
        user_id,
        title: row.title,
        description: row.description,
        duration: row.duration,
        rrule,
        label_id: row.label_id,
        completed_at: row.completed_at,
        created_at: row.created_at,
    };

    record_today_if_due(user_id_str, &todo, pool).await;

    Ok(todo)
}

pub async fn update(
    user_id_str: &str,
    id: i64,
    fields: &UpdateTodoBody,
    pool: &SqlitePool,
) -> Result<Option<Todo>, sqlx::Error> {
    let rrule_str = fields.rrule.as_ref().map(|r| r.0.to_string());
    let completed_at = fields
        .completed
        .unwrap_or(false)
        .then(OffsetDateTime::now_utc);
    let result = sqlx::query!(
        r#"
        UPDATE todos SET
            title = COALESCE(?, title),
            description = COALESCE(?, description),
            duration = COALESCE(?, duration),
            rrule = ?,
            label_id = COALESCE(?, label_id),
            completed_at = ?
        WHERE id = ? AND user_id = ?
        "#,
        fields.title,
        fields.description,
        fields.duration,
        rrule_str,
        fields.label_id,
        completed_at,
        id,
        user_id_str,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query!(
        r#"SELECT id, user_id, title, description, duration, rrule, label_id, 
        completed_at as "completed_at: OffsetDateTime", 
        created_at as "created_at: OffsetDateTime"
        FROM todos WHERE id = ? AND user_id = ?"#,
        id,
        user_id_str,
    )
    .fetch_one(pool)
    .await?;
    let user_id = Uuid::parse_str(&row.user_id)
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored user_id: {e}").into()))?;
    let rrule = row
        .rrule
        .as_deref()
        .map(RRuleField::from_str)
        .transpose()
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored rrule: {e}").into()))?;

    let todo = Todo {
        id: row.id,
        user_id,
        title: row.title,
        description: row.description,
        duration: row.duration,
        completed_at: row.completed_at,
        rrule,
        label_id: row.label_id,
        created_at: row.created_at,
    };
    record_today_if_due(user_id_str, &todo, pool).await;
    Ok(Some(todo))
}

pub async fn toggle_completed(
    user_id: &str,
    id: i64,
    pool: &SqlitePool,
) -> Result<Option<Todo>, sqlx::Error> {
    let current = sqlx::query!(
        r#"SELECT rrule, completed_at as "completed_at: OffsetDateTime" FROM todos
           WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(current) = current else {
        return Ok(None);
    };

    // One-off, currently incomplete, being marked complete -> delete instead
    // of flipping completed_at. No future occurrences to track.
    if current.rrule.is_none() && current.completed_at.is_none() {
        delete(user_id, id, pool).await?;
        return Ok(None);
    }

    let now = OffsetDateTime::now_utc();
    let result = sqlx::query!(
        r#"UPDATE todos 
        SET completed_at = CASE WHEN completed_at IS NULL THEN ? ELSE NULL END 
        WHERE id = ? AND user_id = ?"#,
        now,
        id,
        user_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query!(
        r#"SELECT id, user_id, title, description, duration, rrule, label_id, 
        completed_at as "completed_at: OffsetDateTime", 
        created_at as "created_at: OffsetDateTime"
        FROM todos WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .fetch_one(pool)
    .await?;

    let user_id_parsed = Uuid::parse_str(&row.user_id)
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored user_id: {e}").into()))?;
    let rrule = row
        .rrule
        .as_deref()
        .map(RRuleField::from_str)
        .transpose()
        .map_err(|e| sqlx::Error::Decode(format!("invalid stored rrule: {e}").into()))?;

    let todo = Todo {
        id: row.id,
        user_id: user_id_parsed,
        title: row.title,
        description: row.description,
        duration: row.duration,
        completed_at: row.completed_at,
        rrule,
        label_id: row.label_id,
        created_at: row.created_at,
    };

    record_today_if_due(user_id, &todo, pool).await;

    Ok(Some(todo))
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

async fn record_today_if_due(user_id_str: &str, todo: &Todo, pool: &SqlitePool) {
    let today_date = time::OffsetDateTime::now_utc().date();

    let is_due_today = match crate::rrule_input::is_due_on(
        todo.rrule.as_ref(),
        todo.created_at,
        today_date,
    ) {
        Ok(due) => due,
        Err(e) => {
            tracing::error!(error = %e, todo_id = todo.id, "failed to evaluate rrule due-check");
            return;
        }
    };

    if !is_due_today {
        return;
    }

    let today = today_date.to_string();
    let completed = todo.is_completed_today();
    if let Err(e) = sqlx::query!(
        r#"
        INSERT INTO todo_history (user_id, todo_id, occurrence_date, completed)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(todo_id, occurrence_date) DO UPDATE SET completed = excluded.completed
        "#,
        user_id_str,
        todo.id,
        today,
        completed,
    )
    .execute(pool)
    .await
    {
        tracing::error!(error = %e, todo_id = todo.id, "failed to record today's history row");
    }
}
