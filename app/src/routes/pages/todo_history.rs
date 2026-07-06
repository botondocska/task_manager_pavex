use crate::{
    routes::{
        api::{
            labels::repo as labels_repo,
            todo_history::{self, DurationPeriod, LabelFilter, Period, PeriodCount},
        },
        pages::nav::NAV_ITEMS,
    },
    schemas::Label,
    session_auth::SessionUserId,
};
use askama::Template;
use htmx_macro::hx_get;
use pavex::{Response, http::HeaderValue};
use sqlx::SqlitePool;

const CHART_HEIGHT: f64 = 200.0;
const BAR_WIDTH: f64 = 32.0;
const BAR_GAP: f64 = 12.0;

/// A single bar, pre-computed for the template — no math happens in Askama.
struct ChartBar {
    label: String, // e.g. "Jul 10", "2026-07", "2026"
    completed: i64,
    total: i64,
    percentage: i64,       // 0-100, precomputed
    bar_height: f64,       // pixels, scaled to CHART_HEIGHT
    completed_height: f64, // pixels, portion of bar_height that's "completed"
    x: f64,                // horizontal position
    count_label_y: f64,    // pre-clamped, always visible
}

fn build_chart_bars(counts: &[PeriodCount], period: &Period) -> Vec<ChartBar> {
    let max_total = counts
        .iter()
        .map(|c| c.total_count)
        .max()
        .unwrap_or(1)
        .max(1);

    counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let bar_height = if max_total == 0 {
                0.0
            } else {
                (c.total_count as f64 / max_total as f64) * CHART_HEIGHT
            };
            let completed_height = if c.total_count == 0 {
                0.0
            } else {
                (c.completed_count as f64 / c.total_count as f64) * bar_height
            };
            let count_label_y = (CHART_HEIGHT - bar_height - 4.0).max(10.0);
            let percentage = if c.total_count == 0 {
                0
            } else {
                ((c.completed_count as f64 / c.total_count as f64) * 100.0).round() as i64
            };
            ChartBar {
                label: format_period_label(&c.period_start, period),
                completed: c.completed_count,
                total: c.total_count,
                percentage,
                bar_height,
                completed_height,
                x: i as f64 * (BAR_WIDTH + BAR_GAP),
                count_label_y,
            }
        })
        .collect()
}

struct DurationBar {
    label: String,
    completed_minutes: i64,
    total_minutes: i64,
    percentage: i64,
    bar_height: f64,
    completed_height: f64,
    x: f64,
    minutes_label_y: f64,
}

fn build_duration_bars(counts: &[DurationPeriod], period: &Period) -> Vec<DurationBar> {
    let max_total = counts
        .iter()
        .map(|c| c.total_minutes)
        .max()
        .unwrap_or(1)
        .max(1);

    counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let bar_height = (c.total_minutes as f64 / max_total as f64) * CHART_HEIGHT;
            let completed_height = if c.total_minutes == 0 {
                0.0
            } else {
                (c.completed_minutes as f64 / c.total_minutes as f64) * bar_height
            };
            let minutes_label_y = (CHART_HEIGHT - bar_height - 4.0).max(10.0);
            let percentage = if c.total_minutes == 0 {
                0
            } else {
                ((c.completed_minutes as f64 / c.total_minutes as f64) * 100.0).round() as i64
            };
            DurationBar {
                label: format_period_label(&c.period_start, period),
                completed_minutes: c.completed_minutes,
                total_minutes: c.total_minutes,
                percentage,
                bar_height,
                completed_height,
                x: i as f64 * (BAR_WIDTH + BAR_GAP),
                minutes_label_y,
            }
        })
        .collect()
}

fn format_period_label(period_start: &str, period: &Period) -> String {
    match period {
        Period::Day => {
            // period_start is "YYYY-MM-DD" — show as "Jul 10"
            if let Ok(d) = time::Date::parse(
                period_start,
                &time::format_description::well_known::Iso8601::DATE,
            ) {
                format!("{} {}", month_abbrev(d.month()), d.day())
            } else {
                period_start.to_string()
            }
        }
        Period::Month => period_start.to_string(), // "YYYY-MM"
        Period::Year => period_start.to_string(),  // "YYYY"
    }
}

fn month_abbrev(m: time::Month) -> &'static str {
    match m {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    }
}

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryPage {
    bars: Vec<ChartBar>,
    duration_bars: Vec<DurationBar>,
    duration_avg_line_y: f64,
    avg_duration_percentage: f64,
    labels: Vec<Label>,
    selected_label_ids: Vec<i64>,
    include_unlabeled: bool,
    period: String, // "day" | "month" | "year", for form pre-selection
    num_periods: i64,
    avg_line_y: f64,
    avg_percentage: f64,
    chart_width: f64,
    chart_height: f64,
    active_page: &'static str,
    nav_items: &'static [crate::routes::pages::nav::NavItem],
}

#[derive(serde::Deserialize)]
pub struct HistoryQuery {
    pub period: Option<String>,
    pub count: Option<i64>,
    #[serde(default)]
    pub label_ids: Vec<i64>,
    pub include_unlabeled: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryPageError {
    #[error("Something went wrong.")]
    UnexpectedError(#[source] anyhow::Error),
}

#[pavex::methods]
impl HistoryPageError {
    #[error_handler]
    pub fn into_response(&self) -> Response {
        Response::internal_server_error().set_typed_body(format!("{self}"))
    }
}

#[hx_get(path = "/history", template = "history.html")]
pub async fn history_page(
    query: pavex::request::query::QueryParams<HistoryQuery>,
    user: &SessionUserId,
    pool: &SqlitePool,
) -> Result<Response, HistoryPageError> {
    let q = query.0;
    let user_id = user.0.to_string();

    let period_str = q.period.unwrap_or_else(|| "day".to_string());
    let period = match period_str.as_str() {
        "month" => Period::Month,
        "year" => Period::Year,
        _ => Period::Day,
    };
    let num_periods = q.count.unwrap_or(14).clamp(2, 60);

    let include_unlabeled = q.include_unlabeled.is_some();
    let label_filter = if q.label_ids.is_empty() && !include_unlabeled {
        LabelFilter::All
    } else {
        LabelFilter::Selected {
            label_ids: q.label_ids.clone(),
            include_unlabeled,
        }
    };

    let counts =
        todo_history::history_counts_filled(&user_id, period, num_periods, &label_filter, pool)
            .await
            .map_err(|e| HistoryPageError::UnexpectedError(e.into()))?;

    let avg_percentage: f64 = {
        let completed_sum: i64 = counts.iter().map(|c| c.completed_count).sum();
        let total_sum: i64 = counts.iter().map(|c| c.total_count).sum();
        if total_sum == 0 {
            0.0
        } else {
            (completed_sum as f64 / total_sum as f64 * 100.0 * 100.0).round() / 100.0
        }
    };

    let avg_line_y = CHART_HEIGHT - (avg_percentage / 100.0 * CHART_HEIGHT);

    let bars = build_chart_bars(&counts, &period);

    let duration_counts =
        todo_history::duration_counts_filled(&user_id, period, num_periods, &label_filter, pool)
            .await
            .map_err(|e| HistoryPageError::UnexpectedError(e.into()))?;

    let avg_duration_percentage: f64 = {
        let completed_sum: i64 = duration_counts.iter().map(|c| c.completed_minutes).sum();
        let total_sum: i64 = duration_counts.iter().map(|c| c.total_minutes).sum();
        if total_sum == 0 {
            0.0
        } else {
            (completed_sum as f64 / total_sum as f64 * 100.0 * 100.0).round() / 100.0
        }
    };
    let duration_avg_line_y = CHART_HEIGHT - (avg_duration_percentage / 100.0 * CHART_HEIGHT);
    let duration_bars = build_duration_bars(&duration_counts, &period);

    let chart_width = bars.len() as f64 * (BAR_WIDTH + BAR_GAP);

    let labels = labels_repo::list_for_user(&user_id, pool)
        .await
        .map_err(|e| HistoryPageError::UnexpectedError(e.into()))?;

    let html = HistoryPage {
        bars,
        duration_bars,
        duration_avg_line_y,
        avg_duration_percentage,
        labels,
        selected_label_ids: q.label_ids,
        include_unlabeled,
        period: period_str,
        avg_percentage,
        avg_line_y,
        num_periods,
        chart_width,
        chart_height: CHART_HEIGHT,
        active_page: "history",
        nav_items: NAV_ITEMS,
    }
    .render()
    .expect("render failed");

    Ok(Response::ok().set_typed_body(html).insert_header(
        pavex::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    ))
}
