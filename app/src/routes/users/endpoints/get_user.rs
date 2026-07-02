use crate::{jwt_auth::Claims, schemas::User};
use anyhow::Context;
use pavex::{Response, get, response::body::Json};
use sqlx::SqlitePool;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUserResponse {
    pub user: User,
}

#[get(path = "/user")]
pub async fn get_user(claims: &Claims, pool: &SqlitePool) -> Result<Response, GetUserError> {
    let user_id = claims.user_id().to_string();

    let row = sqlx::query!(
        r#"SELECT email, username, bio, image FROM users WHERE id = ?"#,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to query user")
    .map_err(GetUserError::UnexpectedError)?
    .ok_or_else(|| GetUserError::UnexpectedError(anyhow::anyhow!("User not found")))?;

    let body = Json::new(GetUserResponse {
        user: User {
            email: row.email,
            username: row.username,
            bio: row.bio.unwrap_or_default(),
            image: row.image.unwrap_or_default(),
            token: None, // token not returned on GET
        },
    })
    .map_err(Into::into)
    .map_err(GetUserError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}

#[derive(Debug, thiserror::Error)]
pub enum GetUserError {
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[pavex::methods]
impl GetUserError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        Response::internal_server_error().set_typed_body(format!("{self}"))
    }
}