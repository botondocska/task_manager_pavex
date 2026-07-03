use crate::{jwt_auth::Claims, routes::api::todos::repo};
use pavex::{Response, delete, request::path::PathParams};
use sqlx::SqlitePool;

use super::create_todo::TodoError;
use super::update_todo::TodoId;

#[delete(path = "/todos/{id}")]
pub async fn delete_todo(
    params: PathParams<TodoId>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, TodoError> {
    let TodoId { id } = params.0;
    let user_id = claims.user_id().to_string();

    let deleted = repo::delete(&user_id, id, pool)
        .await
        .map_err(|e| TodoError::UnexpectedError(e.into()))?; //.map_err(|e| TodoError::UnexpectedError(anyhow::Error::new(e).context("Failed to create todo")))?

    if !deleted {
        return Err(TodoError::NotFound);
    }

    Ok(Response::no_content())
}
