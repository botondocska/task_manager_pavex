use crate::{jwt_auth::Claims, routes::api::labels::repo, schemas::Label};
use pavex::{Response, get, response::body::Json};
use sqlx::SqlitePool;

use super::create_label::LabelError;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLabelsResponse {
    pub labels: Vec<Label>,
}

#[get(path = "/labels")]
pub async fn list_labels(claims: &Claims, pool: &SqlitePool) -> Result<Response, LabelError> {
    let user_id = claims.user_id().to_string();

    let labels = repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| LabelError::UnexpectedError(e.into()))?; //.map_err(|e| LabelError::UnexpectedError(anyhow::Error::new(e).context("Failed to create label")))?

    let body = Json::new(ListLabelsResponse { labels })
        .map_err(Into::into)
        .map_err(LabelError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}
