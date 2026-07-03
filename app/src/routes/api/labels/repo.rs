use crate::schemas::Label;
use sqlx::SqlitePool;

pub async fn list_for_user(user_id: &str, pool: &SqlitePool) -> Result<Vec<Label>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, name, color FROM labels WHERE user_id = ?"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Label {
            id: r.id,
            name: r.name,
            color: r.color,
        })
        .collect())
}

pub async fn create(
    user_id: &str,
    name: &str,
    color: &str,
    pool: &SqlitePool,
) -> Result<Label, sqlx::Error> {
    let id = sqlx::query!(
        r#"INSERT INTO labels (user_id, name, color) VALUES (?, ?, ?)"#,
        user_id,
        name,
        color,
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    Ok(Label {
        id,
        name: name.to_string(),
        color: color.to_string(),
    })
}

pub async fn update(
    user_id: &str,
    id: i64,
    name: Option<&str>,
    color: Option<&str>,
    pool: &SqlitePool,
) -> Result<Option<Label>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        UPDATE labels SET
            name  = COALESCE(?, name),
            color = COALESCE(?, color)
        WHERE id = ? AND user_id = ?
        "#,
        name,
        color,
        id,
        user_id,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query!(
        r#"SELECT id, name, color FROM labels WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(Some(Label {
        id: row.id,
        name: row.name,
        color: row.color,
    }))
}

pub async fn delete(user_id: &str, id: i64, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"DELETE FROM labels WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
