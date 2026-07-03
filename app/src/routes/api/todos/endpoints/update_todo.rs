use crate::{
    jwt_auth::Claims,
    routes::api::todos::repo,
    schemas::{Todo, UpdateTodoBody},
};
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

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTodoResponse {
    pub todo: Todo,
}

#[put(path = "/todos/{id}")]
pub async fn update_todo(
    params: PathParams<TodoId>,
    body: JsonBody<UpdateTodoBody>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, TodoError> {
    let TodoId { id } = params.0;
    let user_id = claims.user_id().to_string();

    let todo = repo::update(&user_id, id, &body.0, pool)
        .await
        .map_err(|e| TodoError::UnexpectedError(e.into()))?
        .ok_or(TodoError::NotFound)?;

    let body = Json::new(UpdateTodoResponse { todo })
        .map_err(Into::into)
        .map_err(TodoError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}
