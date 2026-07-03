//! This module contains the tools required to validate user-provided passwords
//! and store them securely in the database as PHC-encoded password hashes.
use crate::telemetry::spawn_blocking_with_tracing;
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use secrecy::{ExposeSecret, Secret};
use sqlx::SqlitePool;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error("Something went wrong when validating credentials")]
    UnexpectedError(#[source] anyhow::Error),
}

/// Compute the password hash using the Argon2id algorithm.
///
/// The returned hash is in the PHC string format, which includes the salt and
/// the algorithm parameters.
pub fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)
    .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?
    .to_string();
    Ok(Secret::new(password_hash))
}

#[tracing::instrument(name = "Validate credentials", skip_all)]
/// Retrieve the stored password hash for the given email address and compare
/// it with the given password.
///
/// # Timing attacks
///
/// We want this function to take a constant time to avoid user enumeration
/// attacks—i.e. an attacker should not be able to guess whether a given
/// email address is registered in our database or not by measuring the
/// response time of the login endpoint.
///
/// To achieve this, we always perform a credential comparison using
/// a dummy password hash, even if the email address is not registered.
///
/// If you want to learn more about this topic, check out
/// <https://www.lpalmieri.com/posts/password-authentication-in-rust/>
pub async fn validate_credentials(
    email: &str,
    password: Secret<String>,
    pool: &SqlitePool,
) -> Result<uuid::Uuid, AuthError> {
    let mut user_id = None;
    let mut expected_password_hash = Secret::new(
        "$argon2id$v=19$m=15000,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
            .to_string(),
    );

    if let Some((stored_user_id, stored_password_hash)) = get_stored_credentials(email, pool)
        .await
        .map_err(AuthError::UnexpectedError)?
    {
        user_id = Some(stored_user_id);
        expected_password_hash = stored_password_hash;
    }

    spawn_blocking_with_tracing(move || verify_password_hash(expected_password_hash, password))
        .await
        .context("Failed to spawn blocking task.")
        .map_err(AuthError::UnexpectedError)??;

    user_id
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Validate credentials", skip_all)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .map_err(|e| anyhow::anyhow!("Failed to parse hash in PHC string format: {e}"))
        .map_err(AuthError::UnexpectedError)?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .map_err(|e| anyhow::anyhow!("Invalid password: {e}"))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Get stored credentials", skip_all)]
async fn get_stored_credentials(
    email: &str,
    pool: &SqlitePool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, password_hash
        FROM users
        WHERE email = ?
        "#,
        email,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")?;

    let Some(row) = row else {
        return Ok(None);
    };
    let id = uuid::Uuid::parse_str(&row.id).context("Stored user id is not a valid UUID.")?;
    Ok(Some((id, Secret::new(row.password_hash))))
}
