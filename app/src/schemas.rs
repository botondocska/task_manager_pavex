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
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Todo {
    pub fn has_label(&self, label_id: &i64) -> bool {
        self.label_id.as_ref() == Some(label_id)
    }

    /// True only if this todo was completed on today's date. A completion
    /// from a previous day is stale and reads as "not complete" — no
    /// separate reset step needed, this is computed fresh on every read.
    pub fn is_completed_today(&self) -> bool {
        match self.completed_at {
            Some(ts) => ts.date() == OffsetDateTime::now_utc().date(),
            None => false,
        }
    }

    pub fn start_date_key(&self) -> Option<i64> {
        self.rrule.as_ref().map(|r| r.0.get_dt_start().timestamp())
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
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
        };
        let json = serde_json::to_string(&t).expect("should serialize");
        println!("{json}");
    }

    #[test]
    fn sort_by_start_datetime_time_matters_within_same_day() {
        use crate::rrule_input::RRuleField;
        use crate::rrule_input::{RRuleInput, build_rrule_set};

        fn todo_with_start(id: i64, dt_start: &str) -> Todo {
            let raw = RRuleInput {
                dt_start: dt_start.to_string(),
                freq: "daily".to_string(),
                interval: None,
                by_weekday: None,
                end_type: "never".to_string(),
                count: None,
                until: None,
            };
            let set = build_rrule_set(raw).unwrap();
            Todo {
                id,
                user_id: uuid::Uuid::new_v4(),
                label_id: None,
                duration: None,
                rrule: Some(RRuleField(set)),
                title: format!("todo-{id}"),
                description: None,
                completed_at: None,
                created_at: OffsetDateTime::now_utc(),
            }
        }

        // Same day, different times — 9am must sort before 5pm.
        let morning = todo_with_start(1, "2026-07-20T09:00");
        let evening = todo_with_start(2, "2026-07-20T17:00");
        let no_start = Todo {
            id: 3,
            user_id: uuid::Uuid::new_v4(),
            label_id: None,
            duration: None,
            rrule: None,
            title: "no-rrule".into(),
            description: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
        };

        let mut todos = vec![evening.clone(), no_start.clone(), morning.clone()];
        todos.sort_by_key(|t| t.start_date_key().unwrap_or(i64::MAX));

        assert_eq!(todos[0].id, 1); // morning first
        assert_eq!(todos[1].id, 2); // evening second
        assert_eq!(todos[2].id, 3); // no start date last
    }
}
