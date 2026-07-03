use crate::{jwt_auth::Claims, schemas::Label};
use anyhow::Context;
use pavex::{Response, get, response::body::Json};
use sqlx::SqlitePool;

use super::create_label::LabelError;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLabelsResponse {
    pub labels: Vec<Label>,
}

#[get(path = "/labels")]
pub async fn list_labels(claims: &Claims, pool: &SqlitePool) -> Result<Response, LabelError> {
    let user_id = claims.user_id().to_string();

    let rows = sqlx::query!(
        r#"SELECT id, name, color FROM labels WHERE user_id = ?"#,
        user_id,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list labels")
    .map_err(LabelError::UnexpectedError)?;

    let labels = rows
        .into_iter()
        .map(|r| Label {
            id: r.id,
            name: r.name,
            color: r.color,
        })
        .collect();

    let body = Json::new(ListLabelsResponse { labels })
        .map_err(Into::into)
        .map_err(LabelError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}