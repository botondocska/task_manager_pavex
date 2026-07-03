use crate::{jwt_auth::Claims, routes::api::labels::repo, schemas::Label};
use pavex::{Response, methods, post, request::body::JsonBody, response::body::Json};
use sqlx::SqlitePool;

#[derive(serde::Deserialize)]
pub struct CreateLabel {
    pub name: String,
    pub color: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLabelResponse {
    pub label: Label,
}

#[post(path = "/labels")]
pub async fn create_label(
    body: JsonBody<CreateLabel>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, LabelError> {
    let CreateLabel { name, color } = body.0;
    let user_id = claims.user_id().to_string();

    let label = repo::create(&user_id, &name, &color, pool)
        .await
        .map_err(|e| LabelError::UnexpectedError(e.into()))?; //.map_err(|e| LabelError::UnexpectedError(anyhow::Error::new(e).context("Failed to create label")))?

    let body = Json::new(CreateLabelResponse { label })
        .map_err(Into::into)
        .map_err(LabelError::UnexpectedError)?;

    Ok(Response::created().set_typed_body(body))
}

#[derive(Debug, thiserror::Error)]
pub enum LabelError {
    #[error("Label not found.")]
    NotFound,
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[methods]
impl LabelError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        match self {
            LabelError::NotFound => Response::not_found(),
            LabelError::UnexpectedError(_) => Response::internal_server_error(),
        }
        .set_typed_body(format!("{self}"))
    }
}
