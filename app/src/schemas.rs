//! Typed schemas shared across routes.
use crate::rrule_input::RRuleField;
use secrecy::{ExposeSecret, Secret};
use time::OffsetDateTime;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub email: String,
    #[serde(
        serialize_with = "serialize_secret_opt",
        skip_serializing_if = "Option::is_none"
    )]
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
fn serialize_secret_opt<S>(
    secret: &Option<Secret<String>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match secret {
        Some(s) => serializer.serialize_str(s.expose_secret()),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(serde::Deserialize)]
pub struct CreateLabelBody {
    pub name: String,
    pub color: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateLabelBody {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: i64,
    pub user_id: uuid::Uuid,
    pub label_id: Option<i64>,
    pub duration: Option<i64>,
    pub rrule: Option<RRuleField>,
    pub title: String,
    pub description: Option<String>,
    pub completed: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Todo {
    pub fn has_label(&self, label_id: &i64) -> bool {
        self.label_id.as_ref() == Some(label_id)
    }
}

#[derive(serde::Deserialize)]
pub struct CreateTodoBody {
    pub title: String,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub rrule: Option<RRuleField>,
    pub label_id: Option<i64>,
    pub completed: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTodoBody {
    pub title: Option<String>,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub rrule: Option<RRuleField>,
    pub label_id: Option<i64>,
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoHistory {
    pub id: i64,
    pub user_id: uuid::Uuid,
    pub todo_id: Option<i64>,
    pub occurrence_date: String,
    pub completed: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_serializes() {
        let t = Todo {
            id: 1,
            user_id: uuid::Uuid::new_v4(),
            label_id: None,
            duration: None,
            rrule: None,
            title: "test".into(),
            description: None,
            completed: false,
            created_at: OffsetDateTime::now_utc(),
        };
        let json = serde_json::to_string(&t).expect("should serialize");
        println!("{json}");
    }
}
