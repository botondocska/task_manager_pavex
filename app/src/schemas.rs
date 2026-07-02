//! Typed schemas shared across routes.
use secrecy::{ExposeSecret, Secret};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub email: String,
    #[serde(serialize_with = "serialize_secret_opt", skip_serializing_if = "Option::is_none")]
    pub token: Option<Secret<String>>,
    pub username: String,
    pub bio: String,
    pub image: String,
}

/// By default, `Secret<String>` cannot be serialized to prevent accidental
/// exfiltration of sensitive data.
/// This function (and the `serialize_with` attribute) allow us to
/// be explicit when we want to override this behaviour and serialize
/// a sensitive value with `serde`.
fn serialize_secret_opt<S>(secret: &Option<Secret<String>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match secret {
        Some(s) => serializer.serialize_str(s.expose_secret()),
        None => serializer.serialize_none(),
    }
}