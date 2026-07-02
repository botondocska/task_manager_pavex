//! Refer to Pavex's [configuration guide](https://pavex.dev/docs/guide/configuration) for more details
//! on how to manage configuration values.
use pavex::config;
use pavex::server::IncomingStream;
use pavex_session::SessionStore;
use pavex_session_sqlx::SqliteSessionStore;
use jsonwebtoken::{DecodingKey, EncodingKey};
use secrecy::{ExposeSecret, Secret};
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr;


#[derive(serde::Deserialize, Debug, Clone)]
/// Configuration for the HTTP server used to expose our API
/// to users.
#[config(key = "server", include_if_unused)]
pub struct ServerConfig {
    /// The port that the server must listen on.
    ///
    /// Set the `PX_SERVER__PORT` environment variable to override its value.
    #[serde(deserialize_with = "serde_aux::field_attributes::deserialize_number_from_string")]
    pub port: u16,
    /// The network interface that the server must be bound to.
    ///
    /// E.g. `0.0.0.0` for listening to incoming requests from
    /// all sources.
    ///
    /// Set the `PX_SERVER__IP` environment variable to override its value.
    pub ip: std::net::IpAddr,
    /// The timeout for graceful shutdown of the server.
    ///
    /// E.g. `1 minute` for a 1 minute timeout.
    ///
    /// Set the `PX_SERVER__GRACEFUL_SHUTDOWN_TIMEOUT` environment variable to override its value.
    #[serde(deserialize_with = "deserialize_shutdown")]
    pub graceful_shutdown_timeout: std::time::Duration,
}

fn deserialize_shutdown<'de, D>(deserializer: D) -> Result<std::time::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;

    let duration = pavex::time::SignedDuration::deserialize(deserializer)?;
    if duration.is_negative() {
        Err(serde::de::Error::custom(
            "graceful shutdown timeout must be positive",
        ))
    } else {
        duration.try_into().map_err(serde::de::Error::custom)
    }
}

impl ServerConfig {
    /// Bind a TCP listener according to the specified parameters.
    pub async fn listener(&self) -> Result<IncomingStream, std::io::Error> {
        let addr = std::net::SocketAddr::new(self.ip, self.port);
        IncomingStream::bind(addr).await
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
#[config(key = "database")]
pub struct DatabaseConfig {
    /// Path to the SQLite database file. E.g. `./data/app.db`
    /// Set `PX_DATABASE__DATABASE_PATH` to override.
    pub database_url: String,
    /// Set via PX_DATABASE__CREATE_IF_MISSING.
    pub create_if_missing: bool
}

#[pavex::methods]
impl DatabaseConfig {
    pub fn connection_options(&self) -> Result<SqliteConnectOptions, sqlx::Error> {
        Ok(SqliteConnectOptions::from_str(&self.database_url)?
            .create_if_missing(self.create_if_missing))
    }
 
    /// Return a database connection pool.
    #[pavex::singleton(clone_if_necessary)]
    pub async fn get_pool(&self) -> Result<sqlx::SqlitePool, sqlx::Error> {
        let pool = sqlx::SqlitePool::connect_with(self.connection_options()?).await?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| sqlx::Error::Migrate(Box::new(e)))?;
        Ok(pool)
    }

    #[pavex::singleton]
    pub async fn session_store(pool: &sqlx::SqlitePool) -> Result<SessionStore, sqlx::Error> {
        let backend = SqliteSessionStore::new(pool.clone());
        backend.migrate().await?;
        Ok(SessionStore::new(backend))
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
#[config(key = "auth")]
/// Configuration for the authentication system.
pub struct AuthConfig {
    /// The private key used to sign JWTs.
    pub eddsa_private_key_pem: Secret<String>,
    /// The public key used to verify the signature of JWTs.
    pub eddsa_public_key_pem: String,
}
 
 #[pavex::methods]
impl AuthConfig {
    /// Return the private key to be used for JWT signing.
    #[pavex::singleton]
    pub fn encoding_key(&self) -> Result<EncodingKey, jsonwebtoken::errors::Error> {
        EncodingKey::from_ed_pem(self.eddsa_private_key_pem.expose_secret().as_bytes())
    }
    
    /// Return the public key to be used for verifying the signature of JWTs.
    #[pavex::singleton]
    pub fn decoding_key(&self) -> Result<DecodingKey, jsonwebtoken::errors::Error> {
        DecodingKey::from_ed_pem(self.eddsa_public_key_pem.as_bytes())
    }
}