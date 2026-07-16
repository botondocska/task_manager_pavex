use chrono::{DateTime, Weekday};
use rrule::{Frequency, NWeekday, RRule, RRuleSet, Tz};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Pure decision: does `rrule` (if any) produce an occurrence on `day`?
/// `None` (a one-off todo) is always due. No I/O, no logging — just data in,
/// data out, fully unit-testable.
pub fn is_due_on(
    rrule: Option<&RRuleField>,
    created_at: time::OffsetDateTime,
    day: time::Date,
) -> Result<bool, anyhow::Error> {
    match rrule {
        Some(r) => crate::todo_history_job::occurs_on(&r.0, day),
        None => Ok(created_at.date() == day),
    }
}

fn parse_dt(s: &str) -> Result<DateTime<Tz>, anyhow::Error> {
    // datetime-local inputs come as "YYYY-MM-DDTHH:MM" (no seconds, no offset).
    // Normalize before parsing.
    let normalized = if s.len() == 16 {
        format!("{s}:00Z")
    } else if s.len() == 19 {
        format!("{s}Z")
    } else {
        s.to_string()
    };
    let parsed = DateTime::parse_from_rfc3339(&normalized)?;
    Ok(parsed.with_timezone(&Tz::UTC))
}

fn map_weekday(s: &str) -> Result<Weekday, anyhow::Error> {
    Ok(match s {
        "mon" => Weekday::Mon,
        "tue" => Weekday::Tue,
        "wed" => Weekday::Wed,
        "thu" => Weekday::Thu,
        "fri" => Weekday::Fri,
        "sat" => Weekday::Sat,
        "sun" => Weekday::Sun,
        _ => anyhow::bail!("invalid weekday: {s}"),
    })
}

/// Raw shape submitted by the create/update todo form.
#[derive(Debug, Deserialize)]
pub struct RRuleInput {
    pub dt_start: String,
    pub freq: String,
    pub interval: Option<u16>,
    pub by_weekday: Option<Vec<String>>,
    pub end_type: String,
    pub count: Option<u32>,
    pub until: Option<String>,
}

pub fn build_rrule_set(raw: RRuleInput) -> Result<RRuleSet, anyhow::Error> {
    let dt_start = parse_dt(&raw.dt_start)?;

    let freq = match raw.freq.as_str() {
        "daily" => Frequency::Daily,
        "weekly" => Frequency::Weekly,
        "monthly" => Frequency::Monthly,
        "yearly" => Frequency::Yearly,
        _ => anyhow::bail!("unknown frequency: {}", raw.freq),
    };

    let mut rule = RRule::new(freq);
    rule = rule.interval(raw.interval.unwrap_or(1));

    if let Some(days) = raw.by_weekday {
        if freq != Frequency::Weekly {
            anyhow::bail!("by_weekday only valid for weekly frequency");
        }
        let weekdays: Vec<NWeekday> = days
            .iter()
            .map(|d| map_weekday(d).map(NWeekday::Every))
            .collect::<Result<_, _>>()?;
        rule = rule.by_weekday(weekdays);
    }

    match raw.end_type.as_str() {
        "count" => {
            let c = raw
                .count
                .ok_or_else(|| anyhow::anyhow!("count required when end_type=count"))?;
            rule = rule.count(c);
        }
        "until" => {
            let u = raw
                .until
                .ok_or_else(|| anyhow::anyhow!("until required when end_type=until"))?;
            rule = rule.until(parse_dt(&u)?);
        }
        "never" => {}
        other => anyhow::bail!("unknown end_type: {other}"),
    }

    let validated = rule.validate(dt_start)?;
    let set = RRuleSet::new(dt_start).rrule(validated);
    Ok(set)
}

pub fn parse_rrule_string(s: &str) -> Result<RRuleInput, anyhow::Error> {
    let set: RRuleSet = s.parse()?;
    let rule = set
        .get_rrule()
        .first()
        .ok_or_else(|| anyhow::anyhow!("no rrule in set"))?;

    let freq = match rule.get_freq() {
        Frequency::Daily => "daily",
        Frequency::Weekly => "weekly",
        Frequency::Monthly => "monthly",
        Frequency::Yearly => "yearly",
        _ => anyhow::bail!("unsupported frequency for display"),
    }
    .to_string();

    let by_weekday = {
        let days = rule.get_by_weekday();
        if days.is_empty() {
            None
        } else {
            Some(
                days.iter()
                    .filter_map(|nwd| match nwd {
                        NWeekday::Every(wd) => Some(weekday_to_str(*wd)),
                        NWeekday::Nth(_, _) => None, // not supported on the way in, skip on the way out
                    })
                    .collect(),
            )
        }
    };

    let (end_type, count, until) = match (rule.get_count(), rule.get_until()) {
        (Some(c), _) => ("count".to_string(), Some(c), None),
        (None, Some(u)) => (
            "until".to_string(),
            None,
            Some(u.format("%Y-%m-%dT%H:%M").to_string()),
        ),
        (None, None) => ("never".to_string(), None, None),
    };

    Ok(RRuleInput {
        dt_start: set.get_dt_start().format("%Y-%m-%dT%H:%M").to_string(),
        freq,
        interval: Some(rule.get_interval()),
        by_weekday,
        end_type,
        count,
        until,
    })
}

fn weekday_to_str(wd: chrono::Weekday) -> String {
    match wd {
        chrono::Weekday::Mon => "mon",
        chrono::Weekday::Tue => "tue",
        chrono::Weekday::Wed => "wed",
        chrono::Weekday::Thu => "thu",
        chrono::Weekday::Fri => "fri",
        chrono::Weekday::Sat => "sat",
        chrono::Weekday::Sun => "sun",
    }
    .to_string()
}

/// Newtype wrapping a validated RRuleSet for use in request bodies.
/// Deserializes from the raw form shape (`RawRRuleInput`), validating
/// on the way in. Serializes back out as the canonical iCalendar string.
#[derive(Debug, Clone)]
pub struct RRuleField(pub RRuleSet);

impl std::str::FromStr for RRuleField {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RRuleField(s.parse::<RRuleSet>()?))
    }
}

impl<'de> Deserialize<'de> for RRuleField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RRuleInput::deserialize(deserializer)?;
        let set = build_rrule_set(raw).map_err(serde::de::Error::custom)?;
        Ok(RRuleField(set))
    }
}

impl Serialize for RRuleField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u8, d: u8) -> time::Date {
        time::Date::from_calendar_date(y, time::Month::try_from(m).unwrap(), d).unwrap()
    }

    #[test]
    fn one_off_todo_always_due() {
        let created = time::OffsetDateTime::now_utc().replace_date(date(2026, 7, 10));
        assert!(is_due_on(None, created, date(2026, 7, 10)).unwrap());
        assert!(is_due_on(None, created, date(2026, 1, 1)).unwrap());
    }

    #[test]
    fn weekly_monday_is_due_on_monday_not_tuesday() {
        let raw = RRuleInput {
            dt_start: "2026-07-06T09:00".to_string(), // a Monday
            freq: "weekly".to_string(),
            interval: None,
            by_weekday: Some(vec!["mon".to_string()]),
            end_type: "never".to_string(),
            count: None,
            until: None,
        };
        let set = build_rrule_set(raw).unwrap();
        let field = RRuleField(set);

        // 2026-07-06 is a Monday, 2026-07-07 is a Tuesday.
        let dt_start = time::OffsetDateTime::now_utc().replace_date(date(2026, 7, 6));
        assert!(is_due_on(Some(&field), dt_start, date(2026, 7, 6)).unwrap());
        assert!(!is_due_on(Some(&field), dt_start, date(2026, 7, 7)).unwrap());
    }

    #[test]
    fn daily_is_due_every_day() {
        let raw = RRuleInput {
            dt_start: "2026-07-01T09:00".to_string(),
            freq: "daily".to_string(),
            interval: None,
            by_weekday: None,
            end_type: "never".to_string(),
            count: None,
            until: None,
        };
        let set = build_rrule_set(raw).unwrap();
        let field = RRuleField(set);

        let dt_start = time::OffsetDateTime::now_utc().replace_date(date(2026, 7, 1));
        assert!(is_due_on(Some(&field), dt_start, date(2026, 7, 1)).unwrap());
        let dt_start = time::OffsetDateTime::now_utc().replace_date(date(2026, 7, 15));
        assert!(is_due_on(Some(&field), dt_start, date(2026, 7, 15)).unwrap());
    }

    #[test]
    fn rrule_round_trips_through_build_and_parse() {
        let raw = RRuleInput {
            dt_start: "2026-07-10T09:00".to_string(),
            freq: "weekly".to_string(),
            interval: Some(2),
            by_weekday: Some(vec!["mon".to_string(), "wed".to_string()]),
            end_type: "count".to_string(),
            count: Some(5),
            until: None,
        };
        let set = build_rrule_set(raw).unwrap();
        let parsed_back = parse_rrule_string(&set.to_string()).unwrap();

        assert_eq!(parsed_back.freq, "weekly");
        assert_eq!(parsed_back.interval, Some(2));
        assert_eq!(parsed_back.end_type, "count");
        assert_eq!(parsed_back.count, Some(5));
    }
}
