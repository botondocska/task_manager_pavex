use crate::{
    jwt_auth::Claims,
    routes::api::todos::repo,
    schemas::{CreateTodoBody, Todo},
};
use pavex::{Response, methods, post, request::body::JsonBody, response::body::Json};
use sqlx::SqlitePool;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTodoResponse {
    pub todo: Todo,
}

#[post(path = "/todos")]
pub async fn create_todo(
    body: JsonBody<CreateTodoBody>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, TodoError> {
    let user_id = claims.user_id().to_string();

    let todo = repo::create(&user_id, &body.0, pool)
        .await
        .map_err(|e| TodoError::UnexpectedError(e.into()))?;

    let body = Json::new(CreateTodoResponse { todo })
        .map_err(Into::into)
        .map_err(TodoError::UnexpectedError)?;

    Ok(Response::created().set_typed_body(body))
}

#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    #[error("Todo not found.")]
    NotFound,
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[methods]
impl TodoError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        match self {
            TodoError::NotFound => Response::not_found(),
            TodoError::UnexpectedError(_) => Response::internal_server_error(),
        }
        .set_typed_body(format!("{self}"))
    }
}
