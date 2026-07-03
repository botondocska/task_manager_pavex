use crate::{jwt_auth::Claims, routes::api::todos::repo, schemas::Todo};
use pavex::{Response, methods, post, request::body::JsonBody, response::body::Json};
use sqlx::SqlitePool;

#[derive(serde::Deserialize)]
pub struct CreateTodo {
    pub title: String,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub rrule: Option<String>,
    pub label_id: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTodoResponse {
    pub todo: Todo,
}

#[post(path = "/todos")]
pub async fn create_todo(
    body: JsonBody<CreateTodo>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, TodoError> {
    let CreateTodo {
        title,
        description,
        duration,
        rrule,
        label_id,
    } = body.0;
    let user_id = claims.user_id().to_string();

    let todo = repo::create(
        &user_id,
        &title,
        description.as_deref(),
        duration,
        rrule.as_deref(),
        label_id,
        pool,
    )
    .await
    .map_err(|e| TodoError::UnexpectedError(e.into()))?; //.map_err(|e| TodoError::UnexpectedError(anyhow::Error::new(e).context("Failed to create todo")))?

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
