use crate::session_theme::Theme;
use crate::{
    routes::{
        api::{
            calendar::{self, DayCell, MonthGrid, TodoOccurrence, View, YearGrid},
            todo_history,
            todos::repo as todos_repo,
            toggle_history_completion,
        },
        pages::{html_response, nav::NAV_ITEMS},
    },
    session_auth::CheckedInUser,
};
use askama::Template;
use pavex::{
    Response, get,
    http::HeaderValue,
    put,
    request::{path::PathParams, query::QueryParams},
};
use sqlx::SqlitePool;
use time::Date;

#[derive(serde::Deserialize)]
pub struct CalendarQuery {
    pub view: Option<String>,
    pub anchor: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CalendarPageError {
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
    #[error("Not found.")]
    NotFound,
}

#[pavex::methods]
impl CalendarPageError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        Response::internal_server_error().set_typed_body(format!("{self}"))
    }
}

#[derive(Template)]
#[template(path = "calendar.html")]
struct CalendarPage {
    view: String,
    anchor: String, // YYYY-MM-DD, for prev/next links
    prev_anchor: String,
    next_anchor: String,
    today_anchor: String,
    day: Option<DayCell>,
    month: Option<MonthGrid>,
    year: Option<YearGrid>,
    active_page: &'static str,
    nav_items: &'static [crate::routes::pages::nav::NavItem],
    theme: Theme,
}

#[derive(Template)]
#[template(path = "day_occurrence_row.html")]
struct DayOccurrenceRow {
    day: DaySingle,
    occ: TodoOccurrence,
}

// Minimal wrapper so template's `day.date` path resolves without needing
// the full DayCell (is_today, occurrences, etc. aren't used by this row).
struct DaySingle {
    date: String, // Display impl needed, or store as pre-formatted string
}

//#[hx_get(path = "/calendar", template = "calendar.html")]
#[get(path = "/calendar")]
pub async fn calendar_page(
    query: QueryParams<CalendarQuery>,
    user: &CheckedInUser,
    pool: &SqlitePool,
    theme: &Theme,
) -> Result<Response, CalendarPageError> {
    let q = query.0;
    let user_id = user.0.to_string();
    let today = time::OffsetDateTime::now_utc().date();

    let view = View::parse(q.view.as_deref().unwrap_or("month"));
    let anchor = q
        .anchor
        .as_deref()
        .and_then(|s| Date::parse(s, &time::format_description::well_known::Iso8601::DATE).ok())
        .unwrap_or(today);

    let todos = todos_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?;

    let (range_start, range_end) = calendar::view_range(anchor, view);
    let completed = todo_history::completed_occurrences(&user_id, &range_start, &range_end, pool)
        .await
        .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?;

    let (day, month, year) = match view {
        View::Day => (
            Some(calendar::build_day_view(anchor, today, &todos, &completed)),
            None,
            None,
        ),
        View::Month => (
            None,
            Some(calendar::build_month_grid(
                anchor, today, &todos, &completed,
            )),
            None,
        ),
        View::Year => (
            None,
            None,
            Some(calendar::build_year_grid(anchor, today, &todos, &completed)),
        ),
    };

    let prev_anchor = calendar::shift_anchor(anchor, view, false).to_string();
    let next_anchor = calendar::shift_anchor(anchor, view, true).to_string();

    let html = CalendarPage {
        view: view.as_str().to_string(),
        anchor: anchor.to_string(),
        prev_anchor,
        next_anchor,
        today_anchor: today.to_string(),
        day,
        month,
        year,
        active_page: "calendar",
        nav_items: NAV_ITEMS,
        theme: *theme,
    }
    .render()
    .expect("render failed");

    Ok(Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    ))
}

#[PathParams]
pub struct DayTodoToggleParams {
    pub date: String, // "YYYY-MM-DD"
    pub todo_id: i64,
}

//#[hx_put(path = "/calendar/{date}/todos/{todo_id}/toggle")]
#[put(path = "/calendar/{date}/todos/{todo_id}/toggle")]
pub async fn toggle_day_occurrence(
    params: PathParams<DayTodoToggleParams>,
    user: &CheckedInUser,
    pool: &SqlitePool,
) -> Result<Response, CalendarPageError> {
    let DayTodoToggleParams { date, todo_id } = params.0;
    let user_id = user.0.to_string();

    // Check if this is a one-off todo currently NOT completed for this
    // occurrence. If so, completing it deletes the todo entirely instead
    // of just flipping todo_history.
    let todo_info = sqlx::query!(
        r#"SELECT rrule FROM todos WHERE id = ? AND user_id = ?"#,
        todo_id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?
    .ok_or(CalendarPageError::NotFound)?;

    let is_one_off = todo_info.rrule.is_none();

    if is_one_off {
        let currently_completed = sqlx::query!(
            r#"SELECT completed as "completed: bool" FROM todo_history
               WHERE todo_id = ? AND occurrence_date = ?"#,
            todo_id,
            date,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?
        .map(|r| r.completed)
        .unwrap_or(false);

        if !currently_completed {
            // Marking complete -> delete the todo (cascades todo_history
            // via ON DELETE SET NULL / CASCADE per schema).
            todos_repo::delete(&user_id, todo_id, pool)
                .await
                .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?;

            // Empty response -> htmx removes the row (hx-swap="outerHTML"
            // with empty body collapses the <li>).
            return Ok(html_response(String::new()));
        }
        // If somehow already completed (edge case, shouldn't normally
        // happen since it'd already be deleted), fall through to
        // ordinary toggle so the checkbox can un-stick.
    }

    let new_completed = toggle_history_completion(&user_id, todo_id, &date, pool)
        .await
        .map_err(CalendarPageError::UnexpectedError)?
        .ok_or(CalendarPageError::NotFound)?;

    let row = sqlx::query!(r#"SELECT title, label_id FROM todos WHERE id = ?"#, todo_id)
        .fetch_one(pool)
        .await
        .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?;

    let parsed_date = Date::parse(&date, &time::format_description::well_known::Iso8601::DATE)
        .map_err(|e| CalendarPageError::UnexpectedError(e.into()))?;

    let html = DayOccurrenceRow {
        day: DaySingle {
            date: parsed_date.to_string(),
        },
        occ: TodoOccurrence {
            id: todo_id,
            title: row.title,
            completed: new_completed,
            label_id: row.label_id,
        },
    }
    .render()
    .expect("render failed");

    Ok(html_response(html))
}
