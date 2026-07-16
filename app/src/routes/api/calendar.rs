//! Pure calendar-grid construction. No I/O — data in, grid out, unit-testable.
use crate::rrule_input::is_due_on;
use crate::schemas::Todo;
use std::collections::{HashMap, HashSet};
use time::{Date, Duration, Month};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Day,
    Month,
    Year,
}

impl View {
    pub fn as_str(&self) -> &'static str {
        match self {
            View::Day => "day",
            View::Month => "month",
            View::Year => "year",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "day" => View::Day,
            "year" => View::Year,
            _ => View::Month,
        }
    }
}

/// Shift the anchor date by one unit of the given view, forward or backward.
pub fn shift_anchor(anchor: Date, view: View, forward: bool) -> Date {
    let sign: i32 = if forward { 1 } else { -1 };
    match view {
        View::Day => anchor + Duration::days(sign as i64),
        View::Month => shift_month(anchor, sign),
        View::Year => shift_year(anchor, sign),
    }
}

fn shift_month(d: Date, delta: i32) -> Date {
    let total = (d.year() * 12 + (d.month() as i32 - 1)) + delta;
    let year = total.div_euclid(12);
    let month0 = total.rem_euclid(12);
    let month = Month::try_from((month0 + 1) as u8).unwrap();
    let day = d.day().min(days_in_month(year, month));
    Date::from_calendar_date(year, month, day).unwrap()
}

fn shift_year(d: Date, delta: i32) -> Date {
    let year = d.year() + delta;
    let day = d.day().min(days_in_month(year, d.month()));
    Date::from_calendar_date(year, d.month(), day).unwrap()
}

fn days_in_month(year: i32, month: Month) -> u8 {
    month.length(year)
}

// ---------------------------------------------------------------------------
// Shared Monday-first grid math. Used by month grid, year grid, and
// view_range so lead-day/week-count logic lives in exactly one place.
// ---------------------------------------------------------------------------

/// Returns (grid_start, number_of_weeks) for the calendar-month grid
/// containing `month`/`year`, padded to full Monday-first weeks.
fn month_grid_bounds(year: i32, month: Month) -> (Date, i64) {
    let first = Date::from_calendar_date(year, month, 1).unwrap();
    let lead_days = first.weekday().number_days_from_monday() as i64;
    let start = first - Duration::days(lead_days);
    let days = days_in_month(year, month) as i64;
    let total_cells = lead_days + days;
    let weeks = (total_cells + 6) / 7;
    (start, weeks)
}

fn month_name(m: Month) -> String {
    m.to_string()
}

fn month_abbrev(m: Month) -> String {
    m.to_string()[..3].to_string()
}

// ---------------------------------------------------------------------------
// Per-day cell: which todos are due, and their completion state.
// ---------------------------------------------------------------------------

pub struct TodoOccurrence {
    pub id: i64,
    pub title: String,
    pub completed: bool,
    pub label_id: Option<i64>,
    pub description: Option<String>,
    pub duration: Option<i64>,
    pub is_recurring: bool,
}

impl TodoOccurrence {
    pub fn has_label(&self, label_id: &i64) -> bool {
        self.label_id.as_ref() == Some(label_id)
    }
}

pub struct DayCell {
    pub date: Date,
    pub in_current_period: bool, // false for padding days from adjacent month
    pub is_today: bool,
    pub occurrences: Vec<TodoOccurrence>,
}

/// `history_by_date` maps "YYYY-MM-DD" -> [(todo_id, completed)], sourced
/// from `todo_history` for dates in the requested range. Past days read
/// exclusively from this ledger — rrule is never consulted for the past,
/// since re-projecting it after an edit can silently disagree with what
/// actually happened (see todo_history_job for the write side).
fn due_todos_for_day(
    todos: &[Todo],
    day: Date,
    today: Date,
    history_by_date: &HashMap<String, Vec<(i64, bool)>>,
    completed: &HashSet<(i64, String)>,
) -> Vec<TodoOccurrence> {
    let day_str = day.to_string();

    if day < today {
        // Past: ledger only. Todo deleted since -> drop silently.
        return history_by_date
            .get(&day_str)
            .into_iter()
            .flatten()
            .filter_map(|(todo_id, completed_flag)| {
                let t = todos.iter().find(|t| t.id == *todo_id)?;
                Some(TodoOccurrence {
                    id: t.id,
                    title: t.title.clone(),
                    completed: *completed_flag,
                    label_id: t.label_id,
                    description: t.description.clone(),
                    duration: t.duration,
                    is_recurring: t.rrule.is_some(),
                })
            })
            .collect();
    }

    // Today/future: rrule projection, unchanged.
    todos
        .iter()
        .filter_map(|t| match is_due_on(t.rrule.as_ref(), t.created_at, day) {
            Ok(true) => Some(TodoOccurrence {
                id: t.id,
                title: t.title.clone(),
                completed: completed.contains(&(t.id, day_str.clone())),
                label_id: t.label_id,
                description: t.description.clone(),
                duration: t.duration,
                is_recurring: t.rrule.is_some(),
            }),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Month view: weeks x days grid, Monday-first, padded to full weeks.
// ---------------------------------------------------------------------------

pub struct WeekRow {
    pub days: Vec<DayCell>,
}

pub struct MonthGrid {
    pub weeks: Vec<WeekRow>,
    pub month_label: String, // "July 2026"
}

pub fn build_month_grid(
    anchor: Date,
    today: Date,
    todos: &[Todo],
    history_by_date: &HashMap<String, Vec<(i64, bool)>>,
    completed: &HashSet<(i64, String)>,
) -> MonthGrid {
    let (grid_start, num_weeks) = month_grid_bounds(anchor.year(), anchor.month());

    let mut weeks = Vec::with_capacity(num_weeks as usize);
    for w in 0..num_weeks {
        let mut days = Vec::with_capacity(7);
        for d in 0..7 {
            let date = grid_start + Duration::days(w * 7 + d);
            days.push(DayCell {
                date,
                in_current_period: date.month() == anchor.month() && date.year() == anchor.year(),
                is_today: date == today,
                occurrences: due_todos_for_day(todos, date, today, history_by_date, completed),
            });
        }
        weeks.push(WeekRow { days });
    }

    MonthGrid {
        weeks,
        month_label: format!("{} {}", month_name(anchor.month()), anchor.year()),
    }
}

// ---------------------------------------------------------------------------
// Day view: single day cell (with full occurrence list, reuse DayCell).
// ---------------------------------------------------------------------------

pub fn build_day_view(
    anchor: Date,
    today: Date,
    todos: &[Todo],
    history_by_date: &HashMap<String, Vec<(i64, bool)>>,
    completed: &HashSet<(i64, String)>,
) -> DayCell {
    DayCell {
        date: anchor,
        in_current_period: true,
        is_today: anchor == today,
        occurrences: due_todos_for_day(todos, anchor, today, history_by_date, completed),
    }
}

// ---------------------------------------------------------------------------
// Year view: 12 mini month-grids, each cell just shows due/completed counts.
// ---------------------------------------------------------------------------

pub struct MiniDayCell {
    pub day_of_month: u8,
    pub in_current_period: bool,
    pub is_today: bool,
    pub total: usize,
    pub completed: usize,
}

pub struct MiniMonthGrid {
    pub month_label: String, // "Jan"
    pub month: Date,         // first of month, for linking into month view
    pub weeks: Vec<Vec<MiniDayCell>>,
}

pub struct YearGrid {
    pub year: i32,
    pub months: Vec<MiniMonthGrid>,
}

pub fn build_year_grid(
    anchor: Date,
    today: Date,
    todos: &[Todo],
    history_by_date: &HashMap<String, Vec<(i64, bool)>>,
    completed: &HashSet<(i64, String)>,
) -> YearGrid {
    let year = anchor.year();
    let mut months = Vec::with_capacity(12);

    for m in 1..=12u8 {
        let month = Month::try_from(m).unwrap();
        let (grid_start, num_weeks) = month_grid_bounds(year, month);

        let mut weeks = Vec::with_capacity(num_weeks as usize);
        for w in 0..num_weeks {
            let mut days = Vec::with_capacity(7);
            for d in 0..7 {
                let date = grid_start + Duration::days(w * 7 + d);
                let occs = due_todos_for_day(todos, date, today, history_by_date, completed);
                days.push(MiniDayCell {
                    day_of_month: date.day(),
                    in_current_period: date.month() == month && date.year() == year,
                    is_today: date == today,
                    total: occs.len(),
                    completed: occs.iter().filter(|o| o.completed).count(),
                });
            }
            weeks.push(days);
        }

        months.push(MiniMonthGrid {
            month_label: month_abbrev(month),
            month: Date::from_calendar_date(year, month, 1).unwrap(),
            weeks,
        });
    }

    YearGrid { year, months }
}

/// Compute the [start, end] date range (inclusive, "YYYY-MM-DD") that a view
/// touches — used to bound the todo_history query. Month/year views pad to
/// full weeks, so range extends slightly past calendar boundaries.
pub fn view_range(anchor: Date, view: View) -> (String, String) {
    match view {
        View::Day => (anchor.to_string(), anchor.to_string()),
        View::Month => {
            let (start, weeks) = month_grid_bounds(anchor.year(), anchor.month());
            let end = start + Duration::days(weeks * 7 - 1);
            (start.to_string(), end.to_string())
        }
        View::Year => {
            let (start, _) = month_grid_bounds(anchor.year(), Month::January);
            let (dec_start, dec_weeks) = month_grid_bounds(anchor.year(), Month::December);
            let end = dec_start + Duration::days(dec_weeks * 7 - 1);
            (start.to_string(), end.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u8, day: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
    }

    #[test]
    fn shift_day_forward_and_back() {
        let a = d(2026, 7, 15);
        assert_eq!(shift_anchor(a, View::Day, true), d(2026, 7, 16));
        assert_eq!(shift_anchor(a, View::Day, false), d(2026, 7, 14));
    }

    #[test]
    fn shift_month_wraps_year() {
        let a = d(2026, 12, 15);
        assert_eq!(shift_anchor(a, View::Month, true), d(2027, 1, 15));
        let a2 = d(2026, 1, 15);
        assert_eq!(shift_anchor(a2, View::Month, false), d(2025, 12, 15));
    }

    #[test]
    fn shift_month_clamps_day() {
        // Jan 31 -> Feb has no 31st, clamp to 28 (2026 not leap).
        let a = d(2026, 1, 31);
        assert_eq!(shift_anchor(a, View::Month, true), d(2026, 2, 28));
    }

    #[test]
    fn shift_year_clamps_leap_day() {
        let a = d(2024, 2, 29);
        assert_eq!(shift_anchor(a, View::Year, true), d(2025, 2, 28));
    }

    #[test]
    fn month_grid_starts_on_monday() {
        // July 2026: 1st is a Wednesday. Use a future "today" so no day in
        // this grid is treated as past (keeps this test on the rrule path).
        let today = d(2026, 6, 1);
        let grid = build_month_grid(d(2026, 7, 1), today, &[], &HashMap::new(), &HashSet::new());
        assert_eq!(grid.weeks[0].days[0].date.weekday(), time::Weekday::Monday);
        // First week should contain June padding days.
        assert!(!grid.weeks[0].days[0].in_current_period);
        assert!(grid.weeks[0].days[2].in_current_period); // July 1st = index 2 (Mon,Tue,Wed)
    }

    #[test]
    fn month_grid_covers_full_month() {
        let today = d(2026, 6, 1);
        let grid = build_month_grid(d(2026, 7, 1), today, &[], &HashMap::new(), &HashSet::new());
        let in_period_count = grid
            .weeks
            .iter()
            .flat_map(|w| &w.days)
            .filter(|d| d.in_current_period)
            .count();
        assert_eq!(in_period_count, 31);
    }

    #[test]
    fn year_grid_has_12_months() {
        let today = d(2025, 1, 1);
        let grid = build_year_grid(d(2026, 1, 1), today, &[], &HashMap::new(), &HashSet::new());
        assert_eq!(grid.months.len(), 12);
        assert_eq!(grid.months[0].month_label, "Jan");
        assert_eq!(grid.months[11].month_label, "Dec");
    }

    #[test]
    fn view_range_month_covers_padded_grid() {
        let (start, end) = view_range(d(2026, 7, 1), View::Month);
        // July 2026 grid starts Mon Jun 29, ends Sun Aug 2 (5 weeks).
        assert_eq!(start, "2026-06-29");
        assert_eq!(end, "2026-08-02");
    }

    #[test]
    fn past_day_reads_from_history_not_rrule() {
        // Rrule says due only on Mondays from Jul 13. History says a
        // different (deleted-since) completion happened on Jul 6, a
        // Monday before dt_start — rrule would say false, history wins
        // for past days.
        let todo = Todo {
            id: 1,
            user_id: uuid::Uuid::new_v4(),
            label_id: None,
            duration: None,
            rrule: None, // one-off, always "due" per rrule path — irrelevant here
            title: "Standup".into(),
            description: None,
            completed_at: None,
            created_at: time::OffsetDateTime::now_utc(),
        };
        let today = d(2026, 7, 15);
        let mut history: HashMap<String, Vec<(i64, bool)>> = HashMap::new();
        history.insert("2026-07-06".to_string(), vec![(1, true)]);

        let day = build_day_view(d(2026, 7, 6), today, &[todo], &history, &HashSet::new());
        assert_eq!(day.occurrences.len(), 1);
        assert!(day.occurrences[0].completed);
    }

    #[test]
    fn past_day_drops_occurrence_for_deleted_todo() {
        let today = d(2026, 7, 15);
        let mut history: HashMap<String, Vec<(i64, bool)>> = HashMap::new();
        history.insert("2026-07-06".to_string(), vec![(999, true)]); // no matching todo

        let day = build_day_view(d(2026, 7, 6), today, &[], &history, &HashSet::new());
        assert!(day.occurrences.is_empty());
    }
}
