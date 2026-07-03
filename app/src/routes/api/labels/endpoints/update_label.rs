use crate::{
    jwt_auth::Claims,
    routes::api::labels::repo,
    schemas::{Label, UpdateLabelBody},
};
use pavex::{
    Response, put,
    request::{body::JsonBody, path::PathParams},
    response::body::Json,
};
use sqlx::SqlitePool;

use super::create_label::LabelError;

#[PathParams]
pub struct LabelId {
    pub id: i64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLabelResponse {
    pub label: Label,
}

#[put(path = "/labels/{id}")]
pub async fn update_label(
    params: PathParams<LabelId>,
    body: JsonBody<UpdateLabelBody>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, LabelError> {
    let LabelId { id } = params.0;
    let user_id = claims.user_id().to_string();

    let label = repo::update(&user_id, id, &body.0, pool)
        .await
        .map_err(|e| LabelError::UnexpectedError(e.into()))? //.map_err(|e| LabelError::UnexpectedError(anyhow::Error::new(e).context("Failed to create label")))?
        .ok_or(LabelError::NotFound)?;

    let body = Json::new(UpdateLabelResponse { label })
        .map_err(Into::into)
        .map_err(LabelError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}
