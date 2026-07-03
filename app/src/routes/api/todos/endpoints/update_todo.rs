use crate::{jwt_auth::Claims, routes::api::todos::repo, schemas::Todo};
use pavex::{
    Response, put,
    request::{body::JsonBody, path::PathParams},
    response::body::Json,
};
use sqlx::SqlitePool;

use super::create_todo::TodoError;

#[PathParams]
pub struct TodoId {
    pub id: i64,
}

#[derive(serde::Deserialize)]
pub struct UpdateTodo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub rrule: Option<String>,
    pub label_id: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTodoResponse {
    pub todo: Todo,
}

#[put(path = "/todos/{id}")]
pub async fn update_todo(
    params: PathParams<TodoId>,
    body: JsonBody<UpdateTodo>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, TodoError> {
    let TodoId { id } = params.0;
    let UpdateTodo {
        title,
        description,
        duration,
        rrule,
        label_id,
    } = body.0;
    let user_id = claims.user_id().to_string();

    let todo = repo::update(
        &user_id,
        id,
        title.as_deref(),
        description.as_deref(),
        duration,
        rrule.as_deref(),
        label_id,
        pool,
    )
    .await
    .map_err(|e| TodoError::UnexpectedError(e.into()))? //.map_err(|e| TodoError::UnexpectedError(anyhow::Error::new(e).context("Failed to create todo")))?
    .ok_or(TodoError::NotFound)?;

    let body = Json::new(UpdateTodoResponse { todo })
        .map_err(Into::into)
        .map_err(TodoError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}
