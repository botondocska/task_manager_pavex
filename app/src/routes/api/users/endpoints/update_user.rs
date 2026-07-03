use crate::{jwt_auth::Claims, routes::api::users::password::compute_password_hash, schemas::User};
use anyhow::Context;
use pavex::{Response, methods, put, request::body::JsonBody, response::body::Json};
use secrecy::Secret;
use sqlx::SqlitePool;

#[derive(serde::Deserialize)]
pub struct UpdateUser {
    pub user: UpdatedDetails,
}

#[derive(serde::Deserialize)]
pub struct UpdatedDetails {
    pub email: Option<String>,
    pub username: Option<String>,
    pub password: Option<Secret<String>>,
    pub bio: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserResponse {
    pub user: User,
}

#[put(path = "/user")]
pub async fn update_user(
    body: JsonBody<UpdateUser>,
    claims: &Claims,
    pool: &SqlitePool,
) -> Result<Response, UpdateUserError> {
    let user_id = claims.user_id().to_string();
    let details = body.0.user;

    // Hash new password if provided
    let password_hash_str = details
        .password
        .map(compute_password_hash)
        .transpose()
        .map_err(UpdateUserError::UnexpectedError)?
        .as_ref()
        .map(|s| secrecy::ExposeSecret::expose_secret(s).to_owned());

    // Build update query dynamically — only update provided fields
    sqlx::query!(
        r#"
        UPDATE users SET
            email         = COALESCE(?, email),
            username      = COALESCE(?, username),
            password_hash = COALESCE(?, password_hash),
            bio           = COALESCE(?, bio),
            image         = COALESCE(?, image),
            updated_at    = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?
        "#,
        details.email,
        details.username,
        password_hash_str,
        details.bio,
        details.image,
        user_id,
    )
    .execute(pool)
    .await
    .context("Failed to update user")
    .map_err(UpdateUserError::UnexpectedError)?;

    // Fetch updated row to return
    let row = sqlx::query!(
        r#"SELECT email, username, bio, image FROM users WHERE id = ?"#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to fetch updated user")
    .map_err(UpdateUserError::UnexpectedError)?;

    let body = Json::new(UpdateUserResponse {
        user: User {
            email: row.email,
            username: row.username,
            bio: row.bio.unwrap_or_default(),
            image: row.image.unwrap_or_default(),
            token: None,
        },
    })
    .map_err(Into::into)
    .map_err(UpdateUserError::UnexpectedError)?;

    Ok(Response::ok().set_typed_body(body))
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateUserError {
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[methods]
impl UpdateUserError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        Response::internal_server_error().set_typed_body(format!("{self}"))
    }
}
