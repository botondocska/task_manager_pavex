use crate::jwt_auth::Claims;
use anyhow::Context;
use pavex::{Response, delete, request::path::PathParams};
use sqlx::SqlitePool;

use super::create_label::LabelError;
use super::update_label::LabelId;

#[delete(path = "/labels/{id}")]
pub async fn delete_label(
    params: PathParams<LabelId>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, LabelError> {
    let LabelId { id } = params.0;
    let user_id = claims.user_id().to_string();

    let result = sqlx::query!(
        r#"DELETE FROM labels WHERE id = ? AND user_id = ?"#,
        id,
        user_id,
    )
    .execute(pool)
    .await
    .context("Failed to delete label")
    .map_err(LabelError::UnexpectedError)?;

    if result.rows_affected() == 0 {
        return Err(LabelError::NotFound);
    }

    Ok(Response::no_content())
}