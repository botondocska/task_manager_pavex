use crate::{
    jwt_auth::encode_token, routes::users::password::compute_password_hash, schemas::User,
};
use jsonwebtoken::EncodingKey;
use pavex::{Response, methods, post, request::body::JsonBody, response::body::Json};
use secrecy::{ExposeSecret, Secret};
use sqlx::SqlitePool;

/// Create a new user.
#[post(path = "/users")]
pub async fn signup(
    body: JsonBody<Signup>,
    db_pool: &SqlitePool,
    jwt_key: &EncodingKey,
) -> Result<Response, SignupError> {
    let UserDetails {
        username,
        email,
        password,
    } = body.0.user;
    let password_hash = compute_password_hash(password).map_err(SignupError::UnexpectedError)?;
    let user_id = insert_user_record(&username, &email, &password_hash, db_pool).await?;
    let token = encode_token(user_id, jwt_key).map_err(SignupError::UnexpectedError)?;

    let body = SignupResponse {
        user: User {
            email,
            token: Some(token),
            username,
            bio: "".into(),
            image: "".into(),
        },
    };
    let body = Json::new(body)
        .map_err(Into::into)
        .map_err(SignupError::UnexpectedError)?;
    Ok(Response::created().set_typed_body(body))
}

#[derive(serde::Deserialize)]
pub struct Signup {
    pub user: UserDetails,
}

#[derive(serde::Deserialize)]
pub struct UserDetails {
    pub username: String,
    pub email: String,
    pub password: Secret<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignupResponse {
    pub user: User,
}

#[derive(Debug, thiserror::Error)]
pub enum SignupError {
    #[error("That username or email is already taken.")]
    Conflict(#[source] anyhow::Error),
    #[error("Something went wrong. Please retry later.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[methods]
impl SignupError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        match self {
            SignupError::Conflict(_) => Response::conflict(),
            SignupError::UnexpectedError(_) => Response::internal_server_error(),
        }
        .set_typed_body(format!("{self}"))
    }
}

/// Insert a new user record in the database.
///
/// If all goes well, it returns the ID of the newly created user.
async fn insert_user_record(
    username: &str,
    email: &str,
    password_hash: &Secret<String>,
    pool: &SqlitePool,
) -> Result<uuid::Uuid, SignupError> {
    let user_id = uuid::Uuid::new_v4();
    let user_id_str = user_id.to_string();
    let password_hash = password_hash.expose_secret();

    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, password_hash)
        VALUES (?, ?, ?, ?)
        "#,
        user_id_str,
        username,
        email,
        password_hash,
    )
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            SignupError::Conflict(e.into())
        }
        _ => SignupError::UnexpectedError(
            anyhow::Error::new(e).context("Failed to insert user record."),
        ),
    })?;

    Ok(user_id)
}
