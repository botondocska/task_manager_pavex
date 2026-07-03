use crate::{jwt_auth::Claims, schemas::Label};
use anyhow::Context;
use pavex::{
    Response, put,
    request::{body::JsonBody, path::PathParams},
    response::body::Json,
};
use sqlx::SqlitePool;

use super::create_label::LabelError;

#[PathParams]
pub struct LabelId {
    pub id: i64,
}

#[derive(serde::Deserialize)]
pub struct UpdateLabel {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLabelResponse {
    pub label: Label,
}

#[put(path = "/labels/{id}")]
pub async fn update_label(
    params: PathParams<LabelId>,
    body: JsonBody<UpdateLabel>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, LabelError> {
    let LabelId { id } = params.0;
    let UpdateLabel { name, color } = body.0;
    let user_id = claims.user_id().to_string();

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
    .await
    .context("Failed to update label")
    .map_err(LabelError::UnexpectedError)?;

    if result.rows_affected() == 0 {
        return Err(LabelError::NotFound);
    }

    let row = sqlx::query!(
        r#"SELECT id, name, color FROM labels WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to fetch updated label")
    .map_err(LabelError::UnexpectedError)?;

    let body = Json::new(UpdateLabelResponse {
        label: Label {
            id: row.id,
            name: row.name,
            color: row.color,
        },
    })
    .map_err(Into::into)
    .map_err(LabelError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}