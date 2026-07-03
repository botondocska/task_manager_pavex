use crate::{jwt_auth::Claims, routes::api::labels::repo};
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

    let deleted = repo::delete(&user_id, id, pool)
        .await
        .map_err(|e| LabelError::UnexpectedError(e.into()))?; //.map_err(|e| LabelError::UnexpectedError(anyhow::Error::new(e).context("Failed to create label")))?

    if !deleted {
        return Err(LabelError::NotFound);
    }

    Ok(Response::no_content())
}
