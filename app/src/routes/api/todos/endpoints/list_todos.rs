use crate::{jwt_auth::Claims, routes::api::todos::repo, schemas::Todo};
use pavex::{Response, get, response::body::Json};
use sqlx::SqlitePool;

use super::create_todo::TodoError;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTodosResponse {
    pub todos: Vec<Todo>,
}

#[get(path = "/todos")]
pub async fn list_todos(claims: &Claims, pool: &SqlitePool) -> Result<Response, TodoError> {
    let user_id = claims.user_id().to_string();

    let todos = repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| TodoError::UnexpectedError(e.into()))?; //.map_err(|e| TodoError::UnexpectedError(anyhow::Error::new(e).context("Failed to create todo")))?

    let body = Json::new(ListTodosResponse { todos })
        .map_err(Into::into)
        .map_err(TodoError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}
